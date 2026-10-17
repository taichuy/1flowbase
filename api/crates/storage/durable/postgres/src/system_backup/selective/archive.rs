use anyhow::{Context, Result, ensure};
use control_plane_contracts::{
    ports::{BackupComponentReader, BackupComponentWriter},
    system_backup::selective::SelectiveBackupSelection,
};
use flate2::{
    Compression,
    write::{GzEncoder, MultiGzDecoder},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::PathBuf,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) const FORMAT: &str = "1flowbase.settings-rows/v1";
pub(super) const MAX_LINE: u64 = 64 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Column {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub generated: bool,
    pub generated_expression: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Table {
    pub name: String,
    pub feature_id: String,
    pub data: bool,
    pub prerequisite: bool,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
    pub plugin_owners: Vec<String>,
    pub prerequisites: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SchemaDependency {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    #[serde(skip)]
    pub declared_tables: BTreeMap<String, String>,
    #[serde(skip)]
    pub legacy_lifecycle_protocol: bool,
    #[serde(skip)]
    pub legacy_notification_digest: bool,
    pub format: String,
    pub selection: Vec<SelectiveBackupSelection>,
    pub tables: Vec<Table>,
    pub plugins: Vec<String>,
    pub schema_dependencies: Vec<SchemaDependency>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_receipts: Option<NotificationReceiptManifest>,
    pub table: String,
    pub row: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct NotificationReceiptManifest {
    pub target_count: u64,
    pub target_identity_digest: String,
}
pub(super) fn hash_receipt_identity(
    hash: &mut Sha256,
    subscriber: &str,
    handler: &str,
    version: &str,
) {
    for value in [subscriber, handler, version] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
}
pub(super) fn finish_receipt_manifest(
    target_count: u64,
    hash: Sha256,
) -> NotificationReceiptManifest {
    NotificationReceiptManifest {
        target_count,
        target_identity_digest: hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}
pub(super) fn receipt_manifest(
    targets: &[(String, String, String)],
) -> NotificationReceiptManifest {
    let mut hash = Sha256::new();
    for (subscriber, handler, version) in targets {
        hash_receipt_identity(&mut hash, subscriber, handler, version);
    }
    finish_receipt_manifest(targets.len() as u64, hash)
}

// Only archive relation identifiers are translated; row fields and PostgreSQL SQL are canonical.
fn canonical_table(name: &str) -> &str {
    match name {
        // @field-contract-compat source=1flowbase.settings-rows/v1:host_event_bus_pending alias=legacy_event_recovery_pending remove_by=2027-04-17
        "host_event_bus_pending" => "legacy_event_recovery_pending",
        // @field-contract-compat source=1flowbase.settings-rows/v1:credit_event_handoffs alias=legacy_credit_handoff_receipts remove_by=2027-04-17
        "credit_event_handoffs" => "legacy_credit_handoff_receipts",
        // @field-contract-compat source=1flowbase.settings-rows/v1:host_event_bus_counter alias=legacy_event_recovery_state remove_by=2027-04-17
        "host_event_bus_counter" => "legacy_event_recovery_state",
        _ => name,
    }
}

pub(super) fn normalize_record(record: &mut Record, header: &Header) -> Result<()> {
    record.table = header
        .declared_tables
        .get(&record.table)
        .context("row references undeclared archive table or alias")?
        .clone();
    Ok(())
}

/// The scratch file is private and removed on every exit, including transaction failure.
pub(super) struct Scratch {
    path: PathBuf,
}
impl Scratch {
    pub fn new() -> Result<(Self, tokio::fs::File)> {
        let path =
            std::env::temp_dir().join(format!("1flowbase-settings-{}.jsonl", uuid::Uuid::now_v7()));
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        Ok((Self { path }, tokio::fs::File::from_std(file)))
    }
    pub async fn reader(&self) -> Result<BufReader<tokio::fs::File>> {
        Ok(BufReader::new(tokio::fs::File::open(&self.path).await?))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
pub(super) async fn unpack(mut reader: BackupComponentReader) -> Result<Scratch> {
    let (scratch, mut file) = Scratch::new()?;
    let mut decoder = MultiGzDecoder::new(Vec::new());
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        decoder
            .write_all(&buffer[..count])
            .context("invalid compressed settings backup")?;
        let decoded = std::mem::take(decoder.get_mut());
        file.write_all(&decoded).await?;
    }
    let decoded = decoder
        .finish()
        .context("truncated compressed settings backup")?;
    file.write_all(&decoded).await?;
    file.flush().await?;
    Ok(scratch)
}
pub(super) async fn line(reader: &mut BufReader<tokio::fs::File>) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    let count = reader
        .take(MAX_LINE + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    if count == 0 {
        return Ok(None);
    }
    ensure!(
        bytes.len() as u64 <= MAX_LINE && bytes.last() == Some(&b'\n'),
        "settings backup row exceeds limit or is truncated"
    );
    Ok(Some(bytes))
}
pub(super) async fn header(reader: &mut BufReader<tokio::fs::File>) -> Result<Header> {
    let bytes = line(reader).await?.context("empty settings backup")?;
    let mut header: Header = serde_json::from_slice(&bytes)?;
    ensure!(header.format == FORMAT, "unsupported settings rows format");
    ensure!(
        header.tables.len() + header.schema_dependencies.len() <= 10000,
        "too many settings tables"
    );
    let mut names = BTreeSet::new();
    for table in &mut header.tables {
        let original = table.name.clone();
        table.name = canonical_table(&original).to_owned();
        ensure!(
            names.insert(table.name.clone()),
            "duplicate archive table or alias"
        );
        header.declared_tables.insert(original, table.name.clone());
        let mut prerequisites = BTreeSet::new();
        for prerequisite in &mut table.prerequisites {
            *prerequisite = canonical_table(prerequisite).to_owned();
            ensure!(
                prerequisites.insert(prerequisite.clone()),
                "duplicate archive prerequisite or alias"
            );
        }
    }
    let mut dependencies = BTreeSet::new();
    for dependency in &mut header.schema_dependencies {
        dependency.name = canonical_table(&dependency.name).to_owned();
        ensure!(
            dependencies.insert(dependency.name.clone()),
            "duplicate schema dependency or alias"
        );
        ensure!(
            !names.contains(&dependency.name),
            "schema dependency duplicates archive table or alias"
        );
    }
    for table in &mut header.tables {
        if table.name == "lifecycle_outbox" {
            let version = table
                .columns
                .iter()
                .any(|c| c.name == "native_publication_version");
            let receipt = table
                .columns
                .iter()
                .any(|c| c.name == "legacy_handoff_sequence");
            ensure!(version == receipt, "partial lifecycle publication schema");
            let digest = table
                .columns
                .iter()
                .any(|c| c.name == "notification_payload_digest");
            ensure!(
                version || !digest,
                "invalid historical lifecycle publication schema"
            );
            if !version {
                header.legacy_lifecycle_protocol = true;
                // Only this exact additive historical schema is supported. The normal
                // catalog equality check still rejects every other schema difference.
                for (name, sql_type, nullable) in [
                    ("native_publication_version", "smallint", false),
                    ("legacy_handoff_sequence", "bigint", true),
                ] {
                    table.columns.push(Column {
                        name: name.into(),
                        sql_type: sql_type.into(),
                        nullable,
                        generated: false,
                        generated_expression: None,
                    });
                }
            }
            if !digest {
                header.legacy_notification_digest = true;
                table.columns.push(Column {
                    name: "notification_payload_digest".into(),
                    sql_type: "bytea".into(),
                    nullable: true,
                    generated: false,
                    generated_expression: None,
                });
            }
        }
    }
    Ok(header)
}

pub(super) fn normalize_publication(
    record: &mut Record,
    historical: bool,
    historical_digest: bool,
) -> Result<()> {
    if record.table != "lifecycle_outbox" {
        return Ok(());
    }
    let row = record
        .row
        .as_object_mut()
        .context("archive row is not an object")?;
    if !row.contains_key("notification_payload_digest") {
        ensure!(
            historical_digest,
            "notification digest missing from current archive"
        );
        row.insert("notification_payload_digest".into(), Value::Null);
    }
    let version = row.get("native_publication_version");
    let receipt = row.get("legacy_handoff_sequence");
    if version.is_none() && receipt.is_none() {
        ensure!(
            historical,
            "lifecycle publication metadata missing from current archive"
        );
        row.insert("native_publication_version".into(), Value::from(1));
        row.insert("legacy_handoff_sequence".into(), Value::Null);
        return Ok(());
    }
    ensure!(
        matches!(version.and_then(Value::as_i64), Some(1 | 2)),
        "invalid lifecycle native publication version"
    );
    ensure!(
        receipt.is_some_and(|r| r.is_null() || r.as_i64().is_some_and(|s| s > 0)),
        "invalid lifecycle handoff sequence"
    );
    Ok(())
}
pub(super) struct Encoder {
    gzip: GzEncoder<Vec<u8>>,
    writer: BackupComponentWriter,
}
impl Encoder {
    pub fn new(writer: BackupComponentWriter) -> Self {
        Self {
            gzip: GzEncoder::new(Vec::new(), Compression::default()),
            writer,
        }
    }
    pub async fn write<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let mut row = serde_json::to_vec(value)?;
        row.push(b'\n');
        ensure!(
            row.len() as u64 <= MAX_LINE,
            "settings backup row exceeds limit"
        );
        self.gzip.write_all(&row)?;
        if self.gzip.get_ref().len() >= 64 * 1024 {
            self.writer
                .write_all(&std::mem::take(self.gzip.get_mut()))
                .await?;
        }
        Ok(())
    }
    pub async fn finish(mut self) -> Result<()> {
        self.gzip.try_finish()?;
        self.writer.write_all(self.gzip.get_ref()).await?;
        self.writer.flush().await?;
        Ok(())
    }
}
