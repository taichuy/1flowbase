//! Seed through actual durable ports and record-permission service. SQL below only
//! observes complete physical rows/catalog identities; it does not fabricate runtime facts.
use super::{
    fixture::{assert_permission_denied, Fixture, Installed},
    inputs::{OLD_VERSION, OWNER},
};
use anyhow::{ensure, Context, Result};
use control_plane::{ports::*, record_permissions::RecordPermissionService};
use control_plane_contracts::PluginCredentialRepository;
use domain::*;
use extension_contracts::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use storage_durable_postgres::PgPluginCredentialRepository;
use uuid::Uuid;

pub struct Seed {
    pub credentials: PgPluginCredentialRepository,
    pub credential_binding: PluginCredentialBinding,
    pub credential_id: String,
    pub secret: Value,
    pub data_binding: PluginDataBinding,
    pub projections: Vec<(PluginDataRequest, PluginDataResponse)>,
    pub acl_key: RecordPermissionKey,
    pub acl: RecordPermissionPolicy,
}

fn data_binding(f: &Fixture, installed: &Installed) -> PluginDataBinding {
    PluginDataBinding {
        managed_subject: None,
        record_permissions: None,
        publisher_namespace: "taichuy".into(),
        plugin_code: "ssh".into(),
        plugin_version: installed.manifest.version.clone(),
        storage_binding: "main".into(),
        workspace_id: SYSTEM_SCOPE_ID.to_string(),
        actor_id: Some(f.root_actor.user_id.to_string()),
        provider_instance_id: installed.result.installation.id.to_string(),
        permissions: BTreeSet::from([PluginDataPermission::Read, PluginDataPermission::Write]),
        deadline_unix_ms: i64::MAX,
    }
}

fn one(operation: PluginDataOperation) -> PluginDataRequest {
    PluginDataRequest {
        idempotency_key: None,
        operations: vec![operation],
    }
}

fn value(field: &PluginOwnedField, host: &str, credential: &str) -> PluginDataValue {
    // Every field from the authoritative old archive is populated, including nullable
    // historical columns. No synthetic cut-down schema or copied commercial manifest.
    let string = match field.field_code.as_str() {
        "host_id" => host.to_owned(),
        "credential_id" => credential.to_owned(),
        "address" => "127.0.0.1".into(),
        "username" => "fixture-only".into(),
        "auth_type" => "password".into(),
        _ => format!("fixture-{}", field.field_code),
    };
    match field.field_type {
        PluginDataFieldType::String | PluginDataFieldType::Text => PluginDataValue::String(string),
        PluginDataFieldType::Number => PluginDataValue::Number(
            match field.field_code.as_str() {
                "port" => "22",
                "timeout_ms" => "30000",
                "max_output_bytes" => "65536",
                _ => "12345",
            }
            .into(),
        ),
        PluginDataFieldType::Boolean => PluginDataValue::Boolean(true),
        PluginDataFieldType::Datetime => PluginDataValue::Datetime("2026-11-21T00:00:00Z".into()),
        PluginDataFieldType::Json => PluginDataValue::Json(match field.field_code.as_str() {
            "credential_ids" | "retired_credential_ids" => json!([credential]),
            "jump_hosts" => json!([]),
            _ => json!({"fixture_preserve": true}),
        }),
        PluginDataFieldType::Uuid => PluginDataValue::Uuid(Uuid::now_v7().to_string()),
    }
}

pub async fn seed(f: &Fixture, old: &Installed) -> Result<Seed> {
    ensure!(
        old.manifest.version == OLD_VERSION,
        "only the authoritative historical manifest may seed the old schema"
    );
    let host_id = Uuid::now_v7().to_string();
    let credential_id = Uuid::now_v7().to_string();
    let data_binding = data_binding(f, old);
    let mut projections = Vec::new();
    let mut collections = BTreeSet::new();
    for model in &old.manifest.data_models {
        ensure!(
            model.extension_fields.is_empty(),
            "SSH historical fixture cannot silently omit extension-field row seeding"
        );
        for collection in &model.owned_collections {
            ensure!(
                collections.insert(collection.collection_code.clone()),
                "duplicate historical collection"
            );
            let values = collection
                .fields
                .iter()
                .map(|field| {
                    (
                        field.field_code.clone(),
                        value(field, &host_id, &credential_id),
                    )
                })
                .collect();
            let target = PluginDataTarget::OwnedCollection {
                collection_code: collection.collection_code.clone(),
            };
            f.store
                .execute(
                    &data_binding,
                    &one(PluginDataOperation::Insert {
                        target: target.clone(),
                        values,
                    }),
                )
                .await?;
            let read = one(PluginDataOperation::Find {
                target,
                fields: std::iter::once("id".to_string())
                    .chain(
                        collection
                            .fields
                            .iter()
                            .map(|field| field.field_code.clone()),
                    )
                    .collect(),
                filters: vec![],
                order: vec![],
                page: PluginDataPage {
                    limit: 2,
                    offset: 0,
                },
            });
            let response = f.store.execute(&data_binding, &read).await?;
            ensure!(
                matches!(&response.results[..], [PluginDataOperationResult::Rows { rows }] if rows.len() == 1),
                "complete historical collection was not seeded/read"
            );
            projections.push((read, response));
        }
    }
    ensure!(
        collections.contains("ssh_hosts") && !projections.is_empty(),
        "authoritative old schema must contain SSH hosts"
    );
    let credentials =
        PgPluginCredentialRepository::new(f.pool.clone(), Uuid::now_v7().to_string())?;
    let credential_binding = PluginCredentialBinding {
        contribution_id: "ssh.hosts.test".into(),
        installation_id: old.result.installation.id.to_string(),
        publisher_namespace: "taichuy".into(),
        plugin_code: "ssh".into(),
        plugin_version: OLD_VERSION.into(),
        scope_id: SYSTEM_SCOPE_ID.to_string(),
        deadline_unix_ms: i64::MAX,
    };
    let secret =
        json!({"password": Uuid::now_v7().to_string(), "private_key": null, "passphrase": null});
    credentials
        .execute_plugin_credential(
            &credential_binding,
            &PluginCredentialRequest::Put {
                credential_id: credential_id.clone(),
                value: secret.clone(),
            },
        )
        .await?;
    let acl_key = RecordPermissionKey {
        workspace_id: f.root_actor.current_workspace_id,
        resource: RecordPermissionResourceKey {
            publisher_namespace: "taichuy".into(),
            plugin_code: "ssh".into(),
            resource_code: "hosts".into(),
        },
        record_id: host_id,
    };
    let service = RecordPermissionService::new(f.store.clone());
    let roles =
        RecordPermissionRepository::record_permission_effective_role_ids(&f.store, &f.member)
            .await?;
    let role = service
        .roles(&f.root_actor)
        .await?
        .into_iter()
        .find(|role| roles.contains(&role.role_id))
        .context("real member has no workspace role eligible for a record grant")?;
    let acl = service
        .replace(
            &f.root_actor,
            &acl_key,
            &ReplaceRecordPermissions {
                mode: RecordPermissionMode::Restricted,
                grants: vec![RecordPermissionGrant {
                    role_id: role.role_id,
                    actions: vec!["view".into(), "open".into()],
                }],
                expected_revision: Some(0),
            },
            &["view".into(), "open".into(), "update".into()],
        )
        .await?;
    let seed = Seed {
        credentials,
        credential_binding,
        credential_id,
        secret,
        data_binding,
        projections,
        acl_key,
        acl,
    };
    seed.assert_access(f, old).await?;
    Ok(seed)
}

impl Seed {
    pub async fn assert_access(&self, f: &Fixture, installed: &Installed) -> Result<()> {
        for (request, expected) in &self.projections {
            // The original complete version projection remains readable after the upgrade.
            ensure!(
                f.store.execute(&self.data_binding, request).await? == *expected,
                "original full-version row projection changed"
            );
        }
        let binding = PluginCredentialBinding {
            installation_id: installed.result.installation.id.to_string(),
            plugin_version: installed.manifest.version.clone(),
            ..self.credential_binding.clone()
        };
        let get = PluginCredentialRequest::Get {
            credential_id: self.credential_id.clone(),
        };
        let decrypted = self
            .credentials
            .execute_plugin_credential(&binding, &get)
            .await?;
        ensure!(
            decrypted.value.as_ref() == Some(&self.secret),
            "version upgrade changed credential decryptability/AAD identity"
        );
        // Same ciphertext cannot be read under another stable owner or scope.
        let foreign = PluginCredentialBinding {
            scope_id: Uuid::now_v7().to_string(),
            ..binding.clone()
        };
        ensure!(
            self.credentials
                .execute_plugin_credential(&foreign, &get)
                .await?
                .value
                .is_none(),
            "credential crossed scope"
        );
        let foreign = PluginCredentialBinding {
            plugin_code: "other-fixture-owner".into(),
            ..binding
        };
        ensure!(
            self.credentials
                .execute_plugin_credential(&foreign, &get)
                .await?
                .value
                .is_none(),
            "credential crossed plugin owner"
        );
        let service = RecordPermissionService::new(f.store.clone());
        ensure!(
            service.get(&f.root_actor, &self.acl_key).await? == self.acl,
            "original record policy/grants changed"
        );
        service
            .authorize(&f.member, &self.acl_key, "view", true)
            .await?;
        service
            .authorize(&f.member, &self.acl_key, "open", true)
            .await?;
        assert_permission_denied(
            service
                .authorize(&f.member, &self.acl_key, "update", true)
                .await,
            "record_permission_denied",
        )?;
        assert_permission_denied(
            service
                .authorize(&f.member, &self.acl_key, "view", false)
                .await,
            "permission_denied",
        )?;
        let cross_workspace = RecordPermissionKey {
            workspace_id: Uuid::now_v7(),
            ..self.acl_key.clone()
        };
        assert_permission_denied(
            service
                .authorize(&f.member, &cross_workspace, "view", true)
                .await,
            "record_permission_workspace",
        )?;
        Ok(())
    }
}

#[derive(PartialEq)]
struct Table {
    identity: Value,
    columns: BTreeMap<String, Value>,
    rows: Vec<Value>,
}

pub struct Snapshot {
    tables: BTreeMap<String, Table>,
    ownership: BTreeMap<String, Value>,
    old_membership: Vec<String>,
    credentials: Value,
    record_acl: Value,
    old_settings: Value,
}

async fn aggregate(pool: &PgPool, table: &str) -> Result<Value> {
    let sql = format!(
        "select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text), '[]'::jsonb) from {} t",
        identifier(table)?
    );
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(pool)
        .await?)
}

fn identifier(value: &str) -> Result<String> {
    ensure!(
        !value.is_empty()
            && value
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
        "unsafe physical identifier"
    );
    Ok(format!("\"{value}\""))
}

impl Snapshot {
    pub async fn capture(f: &Fixture, old_id: Uuid) -> Result<Self> {
        let ownership: Vec<(String, Value)> = sqlx::query_as(
            "select ownership_key, to_jsonb(o) - 'owner_version' - 'plan_fingerprint' - 'updated_at' - 'active' from plugin_schema_ownership o where owner_id=$1 order by ownership_key"
        ).bind(OWNER).fetch_all(&f.pool).await?;
        let ownership = ownership.into_iter().collect();
        let old_membership = sqlx::query_scalar("select ownership_key from plugin_schema_version_objects where owner_id=$1 and owner_version=$2 order by ownership_key")
            .bind(OWNER).bind(OLD_VERSION).fetch_all(&f.pool).await?;
        let names: Vec<String> = sqlx::query_scalar("select physical_table from plugin_schema_ownership where owner_id=$1 and object_kind='owned_collection' order by physical_table")
            .bind(OWNER).fetch_all(&f.pool).await?;
        ensure!(
            names
                .iter()
                .any(|name| name == "plg_6ad77626fdd9_ssh_hosts"),
            "original taichuy/ssh physical SSH table identity changed"
        );
        let mut tables = BTreeMap::new();
        for name in names {
            let identity: Value = sqlx::query_scalar("select jsonb_build_object('oid', c.oid::bigint, 'schema', n.nspname, 'name', c.relname) from pg_class c join pg_namespace n on n.oid=c.relnamespace where c.oid=to_regclass($1)")
                .bind(&name).fetch_one(&f.pool).await?;
            let columns: Vec<(String, Value)> = sqlx::query_as("select a.attname::text, jsonb_build_object('attnum', a.attnum, 'type', a.atttypid::bigint, 'typmod', a.atttypmod, 'not_null', a.attnotnull, 'default', pg_get_expr(d.adbin, d.adrelid)) from pg_attribute a left join pg_attrdef d on d.adrelid=a.attrelid and d.adnum=a.attnum where a.attrelid=to_regclass($1) and a.attnum>0 and not a.attisdropped order by a.attnum")
                .bind(&name).fetch_all(&f.pool).await?;
            let rows: Vec<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "select to_jsonb(t) from {} t order by id",
                identifier(&name)?
            )))
            .fetch_all(&f.pool)
            .await?;
            ensure!(
                rows.len() == 1,
                "each original collection must retain its complete seeded row"
            );
            tables.insert(
                name,
                Table {
                    identity,
                    columns: columns.into_iter().collect(),
                    rows,
                },
            );
        }
        let credentials = aggregate(&f.pool, "plugin_credentials").await?;
        let record_acl = json!({
            "policies": aggregate(&f.pool, "plugin_record_permission_policies").await?,
            "grants": aggregate(&f.pool, "plugin_record_permission_grants").await?,
            "revisions": aggregate(&f.pool, "plugin_record_permission_scope_revisions").await?,
        });
        let old_settings = sqlx::query_scalar("select coalesce(jsonb_agg(to_jsonb(t) order by contribution_code), '[]'::jsonb) from plugin_settings_template_defaults t where installation_id=$1")
            .bind(old_id).fetch_one(&f.pool).await?;
        Ok(Self {
            tables,
            ownership,
            old_membership,
            credentials,
            record_acl,
            old_settings,
        })
    }

    pub fn assert_preserved_by(&self, after: &Self) -> Result<()> {
        ensure!(
            self.credentials == after.credentials,
            "credential ciphertext or stable row identity changed"
        );
        ensure!(
            self.record_acl == after.record_acl,
            "original record policies/grants/revisions changed"
        );
        ensure!(
            self.old_settings == after.old_settings,
            "old installation settings defaults changed"
        );
        ensure!(
            !self.old_membership.is_empty() && self.old_membership == after.old_membership,
            "historical full-version membership changed"
        );
        for (key, old) in &self.ownership {
            ensure!(
                after.ownership.get(key) == Some(old),
                "historical physical ownership changed"
            );
        }
        for (name, before) in &self.tables {
            let after = after
                .tables
                .get(name)
                .context("original physical table disappeared")?;
            ensure!(
                before.identity == after.identity,
                "table was renamed/recreated/moved"
            );
            for (column, details) in &before.columns {
                ensure!(
                    after.columns.get(column) == Some(details),
                    "original physical column changed"
                );
            }
            ensure!(
                before.rows.len() == after.rows.len(),
                "historical row count changed"
            );
            for (old_row, new_row) in before.rows.iter().zip(&after.rows) {
                for (field, value) in old_row.as_object().context("invalid row snapshot")? {
                    ensure!(
                        new_row.get(field) == Some(value),
                        "historical row field changed"
                    );
                }
            }
        }
        Ok(())
    }
}
