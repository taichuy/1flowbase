//! Strict three-table archive import into the single recovery ledger.
//! Only private scratch files are used; no target rows change during normalization.
use super::{
    archive::{self, Column, Record, Scratch, Table},
    schema,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};
use tokio::io::AsyncWriteExt;

pub(super) const LEDGER: &str = "legacy_event_recovery_ledger";
const PENDING: &str = "legacy_event_recovery_pending";
const RECEIPTS: &str = "legacy_credit_handoff_receipts";
const STATE: &str = "legacy_event_recovery_state";
fn legacy(name: &str) -> bool {
    matches!(name, PENDING | RECEIPTS | STATE)
}
pub(super) fn columns(name: &str) -> Vec<Column> {
    let fields: &[(&str, &str, bool)] = match name {
        PENDING => &[
            ("sequence", "bigint", false),
            ("topic", "bytea", false),
            ("payload", "text", false),
            ("credit_event_id", "uuid", true),
            ("value_size_bytes", "bigint", false),
        ],
        RECEIPTS => &[
            ("event_id", "uuid", false),
            ("sequence", "bigint", false),
            ("cutover_id", "uuid", false),
            ("origin", "text", false),
            ("handed_off_at", "timestamp with time zone", false),
        ],
        STATE => &[
            ("singleton", "boolean", false),
            ("last_sequence", "bigint", false),
            ("cutover_id", "uuid", false),
            ("created_at", "timestamp with time zone", false),
        ],
        _ => unreachable!(),
    };
    fields
        .iter()
        .map(|(name, sql_type, nullable)| Column {
            name: (*name).into(),
            sql_type: (*sql_type).into(),
            nullable: *nullable,
            generated: false,
            generated_expression: None,
        })
        .collect()
}
fn sequence(row: &Value) -> Result<i64> {
    let seq = row["sequence"]
        .as_i64()
        .context("invalid recovery archive sequence")?;
    ensure!(seq > 0, "invalid recovery archive sequence");
    Ok(seq)
}
fn blank(sequence: i64, disposition: &str) -> Value {
    json!({"sequence":sequence,"disposition":disposition,"topic":null,"payload":null,"payload_digest":null,"value_size_bytes":null,"credit_event_id":null,"cutover_id":null,"origin":null,"handed_off_at":null,"last_sequence":null,"created_at":null})
}
fn receipt(target: &mut Value, row: &Value) {
    target["credit_event_id"] = row["event_id"].clone();
    for field in ["cutover_id", "origin", "handed_off_at"] {
        target[field] = row[field].clone();
    }
}
fn pending(row: &Value, handoff: Option<&Value>) -> Result<Value> {
    let mut target = blank(sequence(row)?, "pending");
    let payload = row["payload"]
        .as_str()
        .context("invalid recovery payload")?;
    for field in ["topic", "payload", "value_size_bytes"] {
        target[field] = row[field].clone();
    }
    target["payload_digest"] = json!(format!("\\x{:x}", Sha256::digest(payload.as_bytes())));
    match handoff {
        Some(handoff) => {
            ensure!(
                row["credit_event_id"] == handoff["event_id"],
                "inconsistent recovery receipt identity"
            );
            receipt(&mut target, handoff);
        }
        None => ensure!(
            row["credit_event_id"].is_null(),
            "missing recovery credit receipt"
        ),
    }
    Ok(target)
}
// Fixed buckets bound open handles and read memory. A lookup streams only one
// bucket, including oversized buckets, and never accumulates a body backlog.
struct Rows {
    path: PathBuf,
}
impl Rows {
    fn new() -> Result<Self> {
        let path =
            std::env::temp_dir().join(format!("1flowbase-recovery-{}", uuid::Uuid::now_v7()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self { path })
    }
    fn path(&self, seq: i64) -> PathBuf {
        self.path.join((seq % 256).to_string())
    }
    fn insert(&self, record: &Record) -> Result<()> {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(self.path(sequence(&record.row)?))?;
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
        Ok(())
    }
    fn find(&self, table: &str, seq: i64) -> Result<Option<Value>> {
        let file = match File::open(self.path(seq)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut reader = BufReader::new(file);
        loop {
            let mut bytes = Vec::new();
            let count = std::io::Read::take(&mut reader, archive::MAX_LINE + 1)
                .read_until(b'\n', &mut bytes)?;
            if count == 0 {
                return Ok(None);
            }
            ensure!(
                bytes.len() as u64 <= archive::MAX_LINE && bytes.last() == Some(&b'\n'),
                "invalid recovery scratch row"
            );
            let record: Record = serde_json::from_slice(&bytes)?;
            if record.table == table && sequence(&record.row)? == seq {
                return Ok(Some(record.row));
            }
        }
    }
}
impl Drop for Rows {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
async fn write(file: &mut tokio::fs::File, record: &Record) -> Result<()> {
    let mut bytes = serde_json::to_vec(record)?;
    bytes.push(b'\n');
    file.write_all(&bytes).await?;
    Ok(())
}
pub(super) async fn normalize(scratch: Scratch, connection: &mut PgConnection) -> Result<Scratch> {
    let mut reader = scratch.reader().await?;
    let mut header = archive::header(&mut reader).await?;
    let count = header.tables.iter().filter(|t| legacy(&t.name)).count();
    if count == 0 {
        return Ok(scratch);
    }
    ensure!(
        count == 3 && !header.tables.iter().any(|t| t.name == LEDGER),
        "partial or duplicate recovery archive schema"
    );
    ensure!(
        !header
            .schema_dependencies
            .iter()
            .any(|t| legacy(&t.name) || t.name == LEDGER),
        "duplicate recovery schema dependency or alias"
    );
    for table in header.tables.iter().filter(|t| legacy(&t.name)) {
        ensure!(
            table.columns == columns(&table.name),
            "incompatible historical recovery fields"
        );
        let expected = if table.name == STATE {
            "singleton"
        } else if table.name == RECEIPTS {
            "event_id"
        } else {
            "sequence"
        };
        ensure!(
            table.primary_key == [expected]
                && table.feature_id == "system.system-runtime"
                && table.data
                && !table.prerequisite
                && table.plugin_owners.is_empty(),
            "invalid historical recovery table owner or key"
        );
        let expected_prerequisites: Vec<String> = if table.name == PENDING {
            vec![RECEIPTS.into()]
        } else {
            vec![]
        };
        ensure!(
            table.prerequisites == expected_prerequisites,
            "invalid historical recovery prerequisites"
        );
    }
    let rows = Rows::new()?;
    let mut keys = super::key_index::KeyIndex::new()?;
    let mut state = None;
    while let Some(bytes) = archive::line(&mut reader).await? {
        let mut record: Record = serde_json::from_slice(&bytes)?;
        archive::normalize_record(&mut record, &header)?;
        if !legacy(&record.table) {
            continue;
        }
        ensure!(
            record.notification_receipts.is_none(),
            "invalid recovery notification receipt manifest"
        );
        let expected = columns(&record.table);
        let object = record
            .row
            .as_object()
            .context("invalid historical recovery row")?;
        ensure!(
            object.len() == expected.len()
                && expected
                    .iter()
                    .all(|c| object.contains_key(&c.name)
                        && (c.nullable || !object[&c.name].is_null())),
            "historical recovery row field mismatch"
        );
        if record.table == STATE {
            ensure!(
                state.is_none()
                    && record.row["singleton"] == true
                    && record.row["last_sequence"].as_i64().is_some_and(|s| s >= 0),
                "invalid or duplicate recovery state"
            );
            state = Some(record.row);
            continue;
        }
        sequence(&record.row)?;
        keys.insert(&record.table, &["sequence".into()], &record.row)?;
        if record.table == RECEIPTS {
            ensure!(
                matches!(
                    record.row["origin"].as_str(),
                    Some("live" | "legacy_snapshot")
                ),
                "invalid recovery receipt origin"
            );
            keys.insert(
                "recovery_credit_identity",
                &["event_id".into()],
                &record.row,
            )?;
        } else {
            ensure!(
                record.row["topic"].as_str().is_some()
                    && record.row["payload"].as_str().is_some()
                    && record.row["value_size_bytes"]
                        .as_i64()
                        .is_some_and(|s| s >= 0),
                "invalid recovery pending fields"
            );
        }
        rows.insert(&record)?;
    }
    keys.finish_unique()?;
    let state = state.context("missing recovery archive state")?;
    let mut state_row = blank(0, "state");
    for field in ["last_sequence", "cutover_id", "created_at"] {
        state_row[field] = state[field].clone();
    }
    header.tables.retain(|t| !legacy(&t.name));
    header.tables.push(Table {
        name: LEDGER.into(),
        feature_id: "system.system-runtime".into(),
        data: true,
        prerequisite: false,
        columns: schema::columns(connection, LEDGER).await?,
        primary_key: vec!["sequence".into()],
        plugin_owners: vec![],
        prerequisites: vec![],
    });
    // All old declaration names have already been checked; emit canonical names only.
    header.declared_tables.clear();
    let (output, mut file) = Scratch::new()?;
    let mut bytes = serde_json::to_vec(&header)?;
    bytes.push(b'\n');
    file.write_all(&bytes).await?;
    write(
        &mut file,
        &Record {
            table: LEDGER.into(),
            row: state_row,
            notification_receipts: None,
        },
    )
    .await?;
    let mut reader = scratch.reader().await?;
    let source_header = archive::header(&mut reader).await?;
    while let Some(bytes) = archive::line(&mut reader).await? {
        let mut record: Record = serde_json::from_slice(&bytes)?;
        archive::normalize_record(&mut record, &source_header)?;
        if record.table == STATE {
            continue;
        }
        if record.table == PENDING {
            let handoff = rows.find(RECEIPTS, sequence(&record.row)?)?;
            record.row = pending(&record.row, handoff.as_ref())?;
            record.table = LEDGER.into();
        } else if record.table == RECEIPTS {
            let seq = sequence(&record.row)?;
            if rows.find(PENDING, seq)?.is_some() {
                continue;
            }
            let mut target = blank(seq, "taken");
            receipt(&mut target, &record.row);
            record.row = target;
            record.table = LEDGER.into();
        }
        // Preserve additive lifecycle normalization flags when rewriting the header.
        archive::normalize_publication(
            &mut record,
            source_header.legacy_lifecycle_protocol,
            source_header.legacy_notification_digest,
        )?;
        write(&mut file, &record).await?;
    }
    file.flush().await?;
    Ok(output)
}

/// Identity is immutable; taken dominates pending. Unknown pre-ledger bodies are
/// never reconstructed. Missing entries within an existing high-water range are
/// ambiguous pre-ledger takes and must fail closed rather than resurrect work.
pub(super) async fn validate(connection: &mut PgConnection, row: &Value) -> Result<()> {
    let valid: bool = sqlx::query_scalar("select coalesce((select (case when r.sequence=0 then r.disposition='state' and r.last_sequence>=0 and r.cutover_id is not null and r.created_at is not null and r.topic is null and r.payload is null and r.payload_digest is null and r.value_size_bytes is null and r.credit_event_id is null and r.origin is null and r.handed_off_at is null else r.sequence>0 and r.last_sequence is null and r.created_at is null and ((r.credit_event_id is null and r.cutover_id is null and r.origin is null and r.handed_off_at is null) or (r.credit_event_id is not null and r.cutover_id is not null and r.origin in ('live','legacy_snapshot') and r.handed_off_at is not null)) and ((r.disposition='pending' and r.topic is not null and r.payload is not null and r.payload_digest=sha256(convert_to(r.payload,'UTF8')) and octet_length(r.payload_digest)=32 and r.value_size_bytes>=0) or (r.disposition='taken' and r.payload is null and ((r.topic is not null and octet_length(r.payload_digest)=32 and r.value_size_bytes>=0) or (r.topic is null and r.payload_digest is null and r.value_size_bytes is null and r.credit_event_id is not null)))) end) from jsonb_populate_record(null::legacy_event_recovery_ledger,$1) r),false)")
        .bind(row).fetch_one(&mut *connection).await?;
    ensure!(valid, "invalid recovery ledger row or payload digest");
    let conflict: bool = sqlx::query_scalar("select exists(select 1 from legacy_event_recovery_ledger t cross join jsonb_populate_record(null::legacy_event_recovery_ledger,$1) r where r.sequence>0 and t.sequence=r.sequence and ((t.credit_event_id,t.cutover_id,t.origin,t.handed_off_at) is distinct from (r.credit_event_id,r.cutover_id,r.origin,r.handed_off_at) or (t.topic is not null and r.topic is not null and t.topic<>r.topic) or (t.payload_digest is not null and r.payload_digest is not null and t.payload_digest<>r.payload_digest) or (t.value_size_bytes is not null and r.value_size_bytes is not null and t.value_size_bytes<>r.value_size_bytes) or (t.payload is not null and r.payload is not null and t.payload<>r.payload)))")
        .bind(row).fetch_one(&mut *connection).await?;
    ensure!(!conflict, "immutable recovery ledger identity conflict");
    let ambiguous: bool = sqlx::query_scalar("select exists(select 1 from legacy_event_recovery_ledger s cross join jsonb_populate_record(null::legacy_event_recovery_ledger,$1) r where s.sequence=0 and r.sequence>0 and r.disposition='pending' and r.sequence<=greatest(s.last_sequence,(select max(sequence) from legacy_event_recovery_ledger)) and not exists(select 1 from legacy_event_recovery_ledger t where t.sequence=r.sequence))")
        .bind(row).fetch_one(&mut *connection).await?;
    ensure!(
        !ambiguous,
        "recovery entry within target high-water range has unknown historical disposition"
    );
    Ok(())
}
