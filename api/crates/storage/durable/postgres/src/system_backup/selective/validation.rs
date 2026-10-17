use super::{
    archive::{self, Header, Record, Scratch, Table},
    inventory, schema,
};
use crate::secret_crypto::{
    decrypt_secret_json, decrypt_secret_json_with_aad, encrypt_secret_json,
    encrypt_secret_json_with_aad,
};
use anyhow::{Context, Result, ensure};
use control_plane_contracts::system_backup::selective::SelectiveBackupPreview;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn rekey(table: &str, row: &mut Value, source: &str, target: &str) -> Result<()> {
    if matches!(
        table,
        "model_provider_instance_secrets"
            | "data_source_secrets"
            | "network_egress_provider_secrets"
            | "mcp_upstream_connection_secrets"
            | "mcp_client_credentials"
    ) {
        let value = row
            .get("encrypted_secret_json")
            .context("secret row missing ciphertext")?;
        let plain = decrypt_secret_json(value, source)
            .context("source secret key cannot decrypt selected row")?;
        if source != target {
            row["encrypted_secret_json"] = encrypt_secret_json(&plain, target)?;
        }
    }
    if table == "notification_instances" {
        for column in ["encrypted_secret_json", "encrypted_managed_secret_json"] {
            // The new managed column is absent in historical archives and null for legacy
            // instances. Old editable-secret columns retain their existing required shape.
            let value = match row.get(column) {
                Some(value) => value,
                None if column == "encrypted_managed_secret_json" => continue,
                None => anyhow::bail!("notification instance missing secret column"),
            };
            if value.is_null() {
                continue;
            }
            let identity = |name| -> Result<uuid::Uuid> {
                let value = row
                    .get(name)
                    .and_then(Value::as_str)
                    .context("notification instance missing identity")?;
                Ok(uuid::Uuid::parse_str(value)?)
            };
            let workspace = identity("workspace_id")?;
            let instance = identity("id")?;
            let aad = if column == "encrypted_managed_secret_json" {
                crate::notification_repository::managed_aad(workspace, instance)
            } else {
                crate::notification_repository::aad(workspace, instance)
            };
            let plain = decrypt_secret_json_with_aad(value, source, &aad)
                .context("source secret key cannot decrypt notification instance")?;
            if source != target {
                row[column] = encrypt_secret_json_with_aad(&plain, target, &aad)?;
            }
        }
    }
    if table == "plugin_credentials" {
        let field = |name| {
            row.get(name)
                .and_then(Value::as_str)
                .context("invalid plugin credential identity")
        };
        let aad = crate::plugin_credential_repository::credential_associated_data(
            field("publisher_namespace")?,
            field("plugin_code")?,
            field("scope_id")?,
            field("credential_id")?,
        )?;
        let plain = decrypt_secret_json_with_aad(
            row.get("encrypted_value")
                .context("missing plugin credential ciphertext")?,
            source,
            &aad,
        )?;
        if source != target {
            row["encrypted_value"] = encrypt_secret_json_with_aad(&plain, target, &aad)?;
        }
    }
    if table == "provider_protocol_capsules" {
        let field = |name| {
            row.get(name)
                .and_then(Value::as_str)
                .context("invalid capsule identity")
        };
        let aad = format!(
            "provider_protocol_capsule:v1\0{}\0{}\0{}",
            field("flow_run_id")?,
            field("capsule_kind")?,
            field("slot_key")?
        );
        let plain = decrypt_secret_json_with_aad(
            row.get("encrypted_payload")
                .context("missing capsule payload")?,
            source,
            aad.as_bytes(),
        )?;
        if source != target {
            row["encrypted_payload"] =
                encrypt_secret_json_with_aad(&plain, target, aad.as_bytes())?;
        }
    }
    Ok(())
}
struct ForeignKey {
    parent: String,
    columns: Vec<String>,
    parent_columns: Vec<String>,
}
async fn foreign_keys(connection: &mut PgConnection, table: &str) -> Result<Vec<ForeignKey>> {
    let rows=sqlx::query("select p.relname::text as parent, array(select a.attname::text from unnest(f.conkey) with ordinality k(num,ord) join pg_attribute a on a.attrelid=f.conrelid and a.attnum=k.num order by ord) as columns, array(select a.attname::text from unnest(f.confkey) with ordinality k(num,ord) join pg_attribute a on a.attrelid=f.confrelid and a.attnum=k.num order by ord) as parent_columns from pg_constraint f join pg_class c on c.oid=f.conrelid join pg_class p on p.oid=f.confrelid join pg_namespace n on n.oid=c.relnamespace where n.nspname=current_schema() and c.relname=$1 and f.contype='f'").bind(table).fetch_all(connection).await?;
    rows.into_iter()
        .map(|r| {
            Ok(ForeignKey {
                parent: r.try_get("parent")?,
                columns: r.try_get("columns")?,
                parent_columns: r.try_get("parent_columns")?,
            })
        })
        .collect()
}
async fn missing_parent(
    connection: &mut PgConnection,
    keys: &mut super::key_index::KeyIndex,
    table: &Table,
    row: &Value,
    fk: &ForeignKey,
    selected: &BTreeSet<String>,
) -> Result<bool> {
    if fk
        .columns
        .iter()
        .any(|c| row.get(c).is_none_or(Value::is_null))
    {
        return Ok(false);
    }
    let conditions = fk
        .columns
        .iter()
        .zip(&fk.parent_columns)
        .map(|(child, parent)| {
            Ok(format!(
                "p.{}=s.{}",
                schema::quote(parent)?,
                schema::quote(child)?
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(" and ");
    let sql = format!(
        "select exists(select 1 from {} p cross join jsonb_populate_record(null::{},$1) s where {conditions})",
        schema::quote(&fk.parent)?,
        schema::quote(&table.name)?
    );
    // SQL safety: Names are schema::quote output; index expressions/predicates come from the target PostgreSQL catalog.
    if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(row)
        .fetch_one(connection)
        .await?
    {
        return Ok(false);
    }
    Ok(!selected.contains(&fk.parent)
        || !keys.contains(&fk.parent, &fk.parent_columns, &fk.columns, row)?)
}
async fn unique_queries(
    connection: &mut PgConnection,
    table: &Table,
) -> Result<Vec<(String, String)>> {
    let rows=sqlx::query("select i.indexrelid::bigint as oid, ci.relname::text as name, i.indnkeyatts::int as count, pg_get_expr(i.indpred,i.indrelid) as predicate from pg_index i join pg_class c on c.oid=i.indrelid join pg_namespace n on n.oid=c.relnamespace join pg_class ci on ci.oid=i.indexrelid where n.nspname=current_schema() and c.relname=$1 and i.indisunique and not i.indisprimary and i.indisvalid").bind(&table.name).fetch_all(&mut *connection).await?;
    let mut result = Vec::new();
    for index in rows {
        let oid: i64 = index.try_get("oid")?;
        let count: i32 = index.try_get("count")?;
        let expressions = sqlx::query_scalar::<_, String>(
            "select pg_get_indexdef($1::bigint::oid,k,true) from generate_series(1,$2) k",
        )
        .bind(oid)
        .bind(count)
        .fetch_all(&mut *connection)
        .await?;
        let projection = expressions
            .iter()
            .enumerate()
            .map(|(i, e)| format!("{e} as key{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let equality = (0..count)
            .map(|i| format!("old.key{i}=new.key{i}"))
            .collect::<Vec<_>>()
            .join(" and ");
        let same_pk = table
            .primary_key
            .iter()
            .map(|p| {
                Ok(format!(
                    "old.record -> '{}' = $1 -> '{}'",
                    p.replace('\'', "''"),
                    p.replace('\'', "''")
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .join(" and ");
        let predicate = index
            .try_get::<Option<String>, _>("predicate")?
            .unwrap_or_else(|| "true".into());
        let name = schema::quote(&table.name)?;
        let sql = format!(
            "select exists(select 1 from (select to_jsonb(t) as record,{projection} from {name} t where {predicate}) old cross join (select {projection} from jsonb_populate_record(null::{name},$1) t where {predicate}) new where {equality} and not ({same_pk}))"
        );
        result.push((index.try_get("name")?, sql));
    }
    Ok(result)
}
fn failure(preview: &mut SelectiveBackupPreview, message: String) {
    if preview.failures.len() < 100 && !preview.failures.contains(&message) {
        preview.failures.push(message);
    }
}

fn typed_notification(row: &Value) -> bool {
    matches!(
        row["contract_id"].as_str(),
        Some("credit.integration_notification" | "model_definition.integration_notification")
    ) && row["contract_version"] == "v1"
}

fn bytea(value: &Value) -> Result<Vec<u8>> {
    let hex = value
        .as_str()
        .and_then(|s| s.strip_prefix("\\x"))
        .context("invalid notification bytea")?;
    ensure!(hex.len() % 2 == 0, "invalid notification bytea length");
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |b: u8| {
                (b as char)
                    .to_digit(16)
                    .context("invalid notification bytea digit")
            };
            Ok(((digit(pair[0])? << 4) | digit(pair[1])?) as u8)
        })
        .collect()
}

fn compact(row: &Value) -> bool {
    row["canonical_payload"] == "\\x"
}

fn notification_digest(row: &Value) -> Result<[u8; 32]> {
    let body = bytea(&row["canonical_payload"])?;
    let supplied = if row["notification_payload_digest"].is_null() {
        None
    } else {
        let digest = bytea(&row["notification_payload_digest"])?;
        Some(
            <[u8; 32]>::try_from(digest)
                .map_err(|_| anyhow::anyhow!("notification digest must be 32 bytes"))?,
        )
    };
    if body.is_empty() {
        ensure!(
            typed_notification(row) && row["status"] == "delivered",
            "only completed typed notifications may compact"
        );
        return supplied.context("compact notification missing digest");
    }
    let computed: [u8; 32] = Sha256::digest(&body).into();
    ensure!(
        supplied.is_none_or(|digest| digest == computed),
        "notification payload digest mismatch"
    );
    Ok(computed)
}

/// Full and compact representations share identity only after checksum validation.
pub(super) async fn publication_conflict(
    connection: &mut PgConnection,
    row: &Value,
) -> Result<bool> {
    let incoming: Value = sqlx::query_scalar(
        "select to_jsonb(r) from jsonb_populate_record(null::lifecycle_outbox,$1) r",
    )
    .bind(row)
    .fetch_one(&mut *connection)
    .await?;
    let digest = notification_digest(&incoming)?;
    let existing: Option<Value> = sqlx::query_scalar(
        "select to_jsonb(t) from lifecycle_outbox t where event_id=($1->>'event_id')::uuid",
    )
    .bind(row)
    .fetch_optional(connection)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };
    let existing_digest = notification_digest(&existing)?;
    for field in [
        "transaction_id",
        "contract_id",
        "contract_version",
        "occurred_at",
        "created_at",
        "graph_fingerprint",
    ] {
        if existing[field] != incoming[field] {
            return Ok(true);
        }
    }
    if digest != existing_digest {
        return Ok(true);
    }
    if existing["canonical_payload"] == incoming["canonical_payload"] {
        return Ok(false);
    }
    Ok(!typed_notification(&incoming)
        || digest != existing_digest
        || !(compact(&existing) || compact(&incoming)))
}

pub(super) async fn delivery_conflict(connection: &mut PgConnection, row: &Value) -> Result<bool> {
    Ok(sqlx::query_scalar("select exists(select 1 from lifecycle_outbox_deliveries t cross join jsonb_populate_record(null::lifecycle_outbox_deliveries,$1) r where t.event_id=r.event_id and t.subscriber_id=r.subscriber_id and (t.handler_id,t.handler_version) is distinct from (r.handler_id,r.handler_version))")
        .bind(row).fetch_one(connection).await?)
}

fn index_receipt_manifest(
    keys: &mut super::key_index::KeyIndex,
    event: &str,
    count: u64,
    hash: Sha256,
) -> Result<()> {
    let manifest = archive::finish_receipt_manifest(count, hash);
    let row = serde_json::json!({"event_id":event,"target_count":manifest.target_count,"target_identity_digest":manifest.target_identity_digest});
    keys.insert(
        "notification_receipt_manifest",
        &[
            "event_id".into(),
            "target_count".into(),
            "target_identity_digest".into(),
        ],
        &row,
    )
}

pub(super) async fn validate(
    connection: &mut PgConnection,
    scratch: &Scratch,
    source: &str,
    target: &str,
) -> Result<(Header, SelectiveBackupPreview)> {
    let mut reader = scratch.reader().await?;
    let header = archive::header(&mut reader).await?;
    let mut preview = SelectiveBackupPreview::default();
    let mut names = BTreeSet::new();
    let mut valid = BTreeMap::new();
    let selections: BTreeMap<_, _> = header
        .selection
        .iter()
        .map(|s| (s.feature_id.as_str(), s))
        .collect();
    ensure!(
        selections.len() == header.selection.len(),
        "duplicate archive feature selection"
    );
    let mut dependency_names = BTreeSet::new();
    for dependency in &header.schema_dependencies {
        ensure!(
            dependency_names.insert(&dependency.name),
            "duplicate schema dependency"
        );
        let columns = schema::columns(connection, &dependency.name).await?;
        if columns.is_empty() {
            failure(
                &mut preview,
                format!("missing physical table dependency: {}", dependency.name),
            );
            continue;
        }
        if columns != dependency.columns {
            failure(
                &mut preview,
                format!("incompatible physical fields: {}", dependency.name),
            );
        }
        if schema::primary_key(connection, &dependency.name).await? != dependency.primary_key {
            failure(
                &mut preview,
                format!("incompatible physical primary key: {}", dependency.name),
            );
        }
    }
    for table in &header.tables {
        ensure!(names.insert(table.name.clone()), "duplicate archive table");
        ensure!(
            !inventory::excluded(&table.name),
            "backup contains excluded installation table"
        );
        if let Some((owner, data)) = inventory::static_owner(&table.name) {
            ensure!(
                owner == table.feature_id && data == table.data,
                "invalid archive table owner"
            );
        } else {
            ensure!(
                table.data
                    && (matches!(
                        table.feature_id.as_str(),
                        "system.data-models" | "system.files"
                    ) || (table.feature_id == "system.extension-center"
                        && !table.plugin_owners.is_empty())),
                "untrusted archive table owner"
            );
        }
        if table.prerequisite {
            ensure!(
                table.name == "extension_installations",
                "invalid implicit prerequisite table"
            );
        } else {
            let selection = selections
                .get(table.feature_id.as_str())
                .context("table is outside archive selection")?;
            ensure!(
                if table.data {
                    selection.data
                } else {
                    selection.structure
                },
                "table is outside selected row kind"
            );
        }
        let columns = schema::columns(connection, &table.name).await?;
        let columns = if table.name == "extension_installations" {
            super::plugin_identity::columns(columns)
        } else {
            columns
        };
        if columns.is_empty() {
            failure(&mut preview, format!("missing table: {}", table.name));
            continue;
        }
        if columns != table.columns {
            failure(&mut preview, format!("incompatible fields: {}", table.name));
            continue;
        }
        let pk = schema::primary_key(connection, &table.name).await?;
        if pk.is_empty() || pk != table.primary_key {
            failure(
                &mut preview,
                format!("incompatible primary key: {}", table.name),
            );
            continue;
        }
        let fks = foreign_keys(connection, &table.name).await?;
        let unique = unique_queries(connection, table).await?;
        valid.insert(table.name.as_str(), (table, fks, unique));
    }
    preview.table_count = header.tables.len() as u64;
    preview.selected_tables = names.iter().cloned().collect();
    let installed:BTreeSet<String>=sqlx::query_scalar("select plugin_id from extension_installations where plugin_id is not null and id in (select installation_id from extension_artifact_instances where artifact_status='ready')").fetch_all(&mut *connection).await?.into_iter().collect();
    preview.missing_plugins = header
        .plugins
        .iter()
        .filter(|p| !installed.contains(*p))
        .cloned()
        .collect();
    let mut parent_keys: BTreeMap<String, BTreeSet<Vec<String>>> = BTreeMap::new();
    for (_, fks, _) in valid.values() {
        for fk in fks {
            if names.contains(&fk.parent) {
                parent_keys
                    .entry(fk.parent.clone())
                    .or_default()
                    .insert(fk.parent_columns.clone());
            }
        }
    }
    let mut keys = super::key_index::KeyIndex::new()?;
    let mut index_reader = scratch.reader().await?;
    let _ = archive::header(&mut index_reader).await?;
    let mut has_compact = false;
    let mut target_ordered = true;
    let mut target_event: Option<String> = None;
    let mut target_subscriber: Option<String> = None;
    let mut target_count = 0u64;
    let mut target_hash = Sha256::new();
    while let Some(bytes) = archive::line(&mut index_reader).await? {
        let mut record: Record = serde_json::from_slice(&bytes)?;
        archive::normalize_record(&mut record, &header)?;
        if record.table == "lifecycle_outbox"
            && (compact(&record.row) || record.row["canonical_payload"] == "")
        {
            has_compact = true;
        }
        let event_key = vec!["event_id".to_owned()];
        if record.table == "lifecycle_outbox_deliveries" {
            let event = record.row["event_id"]
                .as_str()
                .context("invalid archive target event identity")?;
            let subscriber = record.row["subscriber_id"]
                .as_str()
                .context("invalid archive subscriber identity")?;
            let handler = record.row["handler_id"]
                .as_str()
                .context("invalid archive handler identity")?;
            let version = record.row["handler_version"]
                .as_str()
                .context("invalid archive handler version")?;
            if target_event.as_deref() != Some(event) {
                if let Some(previous) = target_event.take() {
                    target_ordered &= previous.as_str() < event;
                    index_receipt_manifest(
                        &mut keys,
                        &previous,
                        target_count,
                        std::mem::take(&mut target_hash),
                    )?;
                }
                target_event = Some(event.to_owned());
                target_subscriber = None;
                target_count = 0;
            }
            if let Some(previous) = &target_subscriber {
                target_ordered &= previous.as_str() < subscriber;
            }
            target_subscriber = Some(subscriber.to_owned());
            target_count += 1;
            archive::hash_receipt_identity(&mut target_hash, subscriber, handler, version);
            keys.insert("notification_target_parent", &event_key, &record.row)?;
            keys.insert(
                "notification_frozen_target",
                &[
                    "event_id".into(),
                    "subscriber_id".into(),
                    "handler_id".into(),
                    "handler_version".into(),
                ],
                &record.row,
            )?;
            if record.row["status"] != "delivered" {
                keys.insert("notification_unfinished_target", &event_key, &record.row)?;
            }
        }
        if let Some(column_sets) = parent_keys.get(&record.table) {
            for columns in column_sets {
                keys.insert(&record.table, columns, &record.row)?;
            }
        }
    }
    if let Some(event) = target_event {
        index_receipt_manifest(&mut keys, &event, target_count, target_hash)?;
    }
    ensure!(
        !has_compact || target_ordered,
        "compact archive targets must have unique sorted event/subscriber identities"
    );
    keys.finish()?;
    while let Some(bytes) = archive::line(&mut reader).await? {
        let mut record: Record = serde_json::from_slice(&bytes)?;
        archive::normalize_record(&mut record, &header)?;
        ensure!(
            names.contains(&record.table),
            "row references unknown archive table"
        );
        preview.row_count += 1;
        if let Err(error) =
            archive::normalize_publication(&mut record, header.legacy_notification_digest)
        {
            failure(
                &mut preview,
                format!("lifecycle publication metadata: {error}"),
            );
            continue;
        }
        if record.table == "model_definitions"
            && record.row.get("source_kind").and_then(Value::as_str) == Some("main_source")
            && record
                .row
                .get("data_source_instance_id")
                .is_none_or(Value::is_null)
        {
            if let Some(physical) = record
                .row
                .get("physical_table_name")
                .and_then(Value::as_str)
            {
                if !names.contains(physical)
                    && !dependency_names
                        .iter()
                        .any(|name| name.as_str() == physical)
                {
                    failure(
                        &mut preview,
                        format!("missing physical schema description: {physical}"),
                    );
                }
            } else {
                failure(
                    &mut preview,
                    "model definition has no physical table name".into(),
                );
            }
        }

        let Some((table, fks, unique)) = valid.get(record.table.as_str()) else {
            continue;
        };
        let object = record
            .row
            .as_object()
            .context("archive row is not an object")?;
        if object.len() != table.columns.len()
            || table.columns.iter().any(|c| !object.contains_key(&c.name))
        {
            failure(&mut preview, format!("row field mismatch: {}", table.name));
            continue;
        }
        if table
            .primary_key
            .iter()
            .any(|pk| object.get(pk).is_none_or(Value::is_null))
        {
            failure(
                &mut preview,
                format!("row primary key is null: {}", table.name),
            );
            continue;
        }
        if table.name == "extension_installations" {
            if let Some(existing) =
                super::plugin_identity::existing(connection, &record.row).await?
            {
                if !super::plugin_identity::compatible(&existing, &record.row) {
                    failure(
                        &mut preview,
                        "plugin logical identity conflicts with target primary key".into(),
                    );
                }
                // Existing identity is preserved, so incoming actor/policy dependencies do not apply.
                continue;
            }
        }
        if let Err(error) = rekey(&record.table, &mut record.row, source, target) {
            failure(
                &mut preview,
                format!("secret compatibility: {}: {error}", table.name),
            );
            continue;
        }
        // PostgreSQL itself checks JSON-to-domain conversion before any row mutation.
        let cast = format!(
            "select to_jsonb(r) from jsonb_populate_record(null::{},$1) r",
            schema::quote(&table.name)?
        );
        let normalized: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(cast.as_str()))
            .bind(&record.row)
            .fetch_one(&mut *connection)
            .await?;
        if record.notification_receipts.is_some()
            && !(table.name == "lifecycle_outbox" && compact(&normalized))
        {
            failure(
                &mut preview,
                "notification receipt manifest only allowed on compact facts".into(),
            );
        }
        if table.name == "lifecycle_outbox" {
            if compact(&normalized) {
                if let Some(manifest) = &record.notification_receipts {
                    let manifest_row = serde_json::json!({"event_id":normalized["event_id"],"target_count":manifest.target_count,"target_identity_digest":manifest.target_identity_digest});
                    let columns = vec![
                        "event_id".into(),
                        "target_count".into(),
                        "target_identity_digest".into(),
                    ];
                    if manifest.target_count == 0
                        || !keys.contains(
                            "notification_receipt_manifest",
                            &columns,
                            &columns,
                            &manifest_row,
                        )?
                    {
                        failure(
                            &mut preview,
                            "compact notification receipt manifest mismatch".into(),
                        );
                    }
                } else {
                    failure(
                        &mut preview,
                        "compact notification receipt manifest missing".into(),
                    );
                }
            }
            if let Err(error) = notification_digest(&normalized) {
                failure(&mut preview, format!("notification payload: {error}"));
                continue;
            }
            let event_key = vec!["event_id".to_owned()];
            if compact(&normalized)
                && (!keys.contains(
                    "notification_target_parent",
                    &event_key,
                    &event_key,
                    &normalized,
                )? || keys.contains(
                    "notification_unfinished_target",
                    &event_key,
                    &event_key,
                    &normalized,
                )?)
            {
                failure(
                    &mut preview,
                    "compact notification requires all frozen target receipts delivered".into(),
                );
            }
            // Existing frozen targets may neither disappear nor be retargeted by a restore.
            if typed_notification(&normalized) {
                let targets: Vec<Value> = sqlx::query_scalar("select to_jsonb(t) from lifecycle_outbox_deliveries t where event_id=($1->>'event_id')::uuid")
                    .bind(&normalized).fetch_all(&mut *connection).await?;
                let target_key = vec![
                    "event_id".into(),
                    "subscriber_id".into(),
                    "handler_id".into(),
                    "handler_version".into(),
                ];
                for target in targets {
                    if !keys.contains(
                        "notification_frozen_target",
                        &target_key,
                        &target_key,
                        &target,
                    )? {
                        failure(
                            &mut preview,
                            "immutable lifecycle frozen target set conflict".into(),
                        );
                    }
                }
            }
        }
        if table.name == "lifecycle_outbox_deliveries"
            && delivery_conflict(connection, &normalized).await?
        {
            failure(
                &mut preview,
                "immutable lifecycle target conflict: lifecycle_outbox_deliveries".into(),
            );
        }
        if table.name == "lifecycle_outbox_deliveries" {
            let extra: bool = sqlx::query_scalar("select exists(select 1 from lifecycle_outbox p cross join jsonb_populate_record(null::lifecycle_outbox_deliveries,$1) r where p.event_id=r.event_id and p.contract_version='v1' and p.contract_id in ('credit.integration_notification','model_definition.integration_notification') and exists(select 1 from lifecycle_outbox_deliveries d where d.event_id=p.event_id) and not exists(select 1 from lifecycle_outbox_deliveries d where d.event_id=r.event_id and d.subscriber_id=r.subscriber_id))")
                .bind(&normalized).fetch_one(&mut *connection).await?;
            if extra {
                failure(
                    &mut preview,
                    "immutable lifecycle frozen target set conflict".into(),
                );
            }
        }
        if table.columns.iter().any(|c| {
            !c.nullable && !c.generated && normalized.get(&c.name).is_none_or(Value::is_null)
        }) {
            failure(
                &mut preview,
                format!("required field is null: {}", table.name),
            );
        }
        for fk in fks {
            if missing_parent(connection, &mut keys, table, &record.row, fk, &names).await? {
                failure(
                    &mut preview,
                    format!(
                        "missing prerequisite: {} -> {} ({})",
                        table.name,
                        fk.parent,
                        fk.columns.join(",")
                    ),
                );
            }
        }
        for (index, sql) in unique {
            // unique_queries uses quoted names and index expressions read from the target catalog.
            // Archive row values remain bound as JSON rather than inserted into the SQL text.
            if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(&record.row)
                .fetch_one(&mut *connection)
                .await?
            {
                failure(
                    &mut preview,
                    format!("unique identity conflict: {} ({index})", table.name),
                );
            }
        }
        if table.name == "lifecycle_outbox" {
            let conflict = publication_conflict(connection, &record.row).await?;
            if conflict {
                failure(
                    &mut preview,
                    "immutable lifecycle publication conflict: lifecycle_outbox".into(),
                );
            }
        }
        if matches!(
            table.name.as_str(),
            "runtime_canonical_contents"
                | "runtime_native_snapshot_items"
                | "runtime_native_snapshot_manifests"
                | "runtime_native_snapshot_references"
                | "flow_run_recovery_history"
                | "i18n_catalog_releases"
        ) {
            let condition = table
                .primary_key
                .iter()
                .map(|pk| Ok(format!("t.{}=r.{}", schema::quote(pk)?, schema::quote(pk)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" and ");
            let sql = format!(
                "select exists(select 1 from {} t cross join jsonb_populate_record(null::{},$1) r where {condition} and to_jsonb(t) is distinct from to_jsonb(r))",
                schema::quote(&table.name)?,
                schema::quote(&table.name)?
            );
            // The table and primary-key names above are quoted; the archive row remains bound.
            if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(&record.row)
                .fetch_one(&mut *connection)
                .await?
            {
                failure(
                    &mut preview,
                    format!("immutable row conflict: {}", table.name),
                );
            }
        }
    }
    Ok((header, preview))
}
