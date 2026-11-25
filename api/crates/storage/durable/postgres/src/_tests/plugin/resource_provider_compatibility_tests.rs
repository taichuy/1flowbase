//! Adapter-level compatibility fixture, not proof of an actual old-package upgrade.
//!
//! The SSH 0.1.25 manifest exports `ssh-hosts-v1` from `ssh_hosts` with read-only
//! credentials. This finite local schema uses its real `host_id`, `name`,
//! `credential_id`, and `connection_options` fields. `legacy_note` is a synthetic
//! nullable historical field, not a claim about the complete 0.1.21 manifest.
//! Resource export must not move ownership away from `taichuy/ssh`.
//!
//! Use the canonical ManagedSchemaPlan storage port: manifest compilation,
//! export resolution/authorization, installation, and service activation belong
//! to the Root's separate cross-layer acceptance, not this adapter crate.

use std::collections::{BTreeMap, BTreeSet};

use control_plane_contracts::ports::{
    ManagedSchemaFieldType, ManagedSchemaOperation, ManagedSchemaPlan, ManagedSchemaRepository,
    PluginCredentialRepository,
};
use extension_contracts::{
    PluginCredentialBinding, PluginCredentialRequest, PluginDataBinding, PluginDataErrorKind,
    PluginDataFilter, PluginDataFilterOperator, PluginDataOperation, PluginDataOperationResult,
    PluginDataPage, PluginDataPermission, PluginDataPort, PluginDataRequest, PluginDataTarget,
    PluginDataValue,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::plugin_credential_repository::credential_associated_data;
use crate::secret_crypto::decrypt_secret_json_with_aad;
use crate::{run_migrations, PgControlPlaneStore, PgPluginCredentialRepository};

const OWNER: &str = "taichuy/ssh";
const OLD_VERSION: &str = "0.1.21";
const NEW_VERSION: &str = "0.1.25";
// Historical compiler identity: plg_<first 12 SHA256 hex of taichuy/ssh>_ssh_hosts.
// Neither a version nor the new export code is part of this physical name.
const TABLE: &str = "plg_6ad77626fdd9_ssh_hosts";
const MASTER_KEY: &str = "ssh-provider-compatibility-fixture-key";

// Same store()/PostgresTestSchema fixture pattern as the adjacent adapter tests.
// Both production adapters below share this one migrated, disposable schema.
async fn store() -> (PgControlPlaneStore, PgPool) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    (PgControlPlaneStore::new(pool.clone()), pool)
}

fn field(name: &str, field_type: ManagedSchemaFieldType, nullable: bool) -> ManagedSchemaOperation {
    ManagedSchemaOperation::EnsureOwnedField {
        logical_collection: "ssh_hosts".into(),
        logical_field: name.into(),
        physical_table: TABLE.into(),
        physical_column: name.into(),
        field_type,
        nullable,
    }
}

fn schema(version: &str) -> ManagedSchemaPlan {
    let mut operations = vec![ManagedSchemaOperation::EnsureOwnedCollection {
        logical_collection: "ssh_hosts".into(),
        physical_table: TABLE.into(),
    }];
    for name in ["host_id", "name", "credential_id"] {
        operations.push(field(name, ManagedSchemaFieldType::String, false));
    }
    match version {
        OLD_VERSION => operations.push(field("legacy_note", ManagedSchemaFieldType::String, true)),
        NEW_VERSION => {
            operations.push(field(
                "connection_options",
                ManagedSchemaFieldType::Json,
                true,
            ));
            operations.push(ManagedSchemaOperation::RetainInactive {
                ownership_key: format!("column:{TABLE}.legacy_note"),
            });
        }
        _ => panic!("unsupported compatibility fixture version"),
    }
    ManagedSchemaPlan {
        owner_id: OWNER.into(),
        owner_version: version.into(),
        fingerprint: format!("ssh-provider-compatibility-{version}"),
        lock_timeout_ms: 1_000,
        operations,
    }
}

fn binding(scope: Uuid, installation: Uuid) -> PluginDataBinding {
    PluginDataBinding {
        managed_subject: None,
        record_permissions: None,
        publisher_namespace: "taichuy".into(),
        plugin_code: "ssh".into(),
        plugin_version: OLD_VERSION.into(),
        storage_binding: "main".into(),
        workspace_id: scope.to_string(),
        actor_id: Some(Uuid::now_v7().to_string()),
        provider_instance_id: installation.to_string(),
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

fn target() -> PluginDataTarget {
    PluginDataTarget::OwnedCollection {
        collection_code: "ssh_hosts".into(),
    }
}

fn find(host_id: &str, fields: &[&str]) -> PluginDataRequest {
    one(PluginDataOperation::Find {
        target: target(),
        fields: fields.iter().map(|name| (*name).into()).collect(),
        filters: vec![PluginDataFilter {
            field: "host_id".into(),
            operator: PluginDataFilterOperator::Equal,
            value: Some(PluginDataValue::String(host_id.into())),
        }],
        order: vec![],
        page: PluginDataPage::default(),
    })
}

fn aad(binding: &PluginCredentialBinding, credential_id: &str) -> Vec<u8> {
    credential_associated_data(
        &binding.publisher_namespace,
        &binding.plugin_code,
        &binding.scope_id,
        credential_id,
    )
    .unwrap()
}

async fn ciphertext(pool: &PgPool, scope: Uuid, credential_id: &str) -> Vec<u8> {
    // Compare the stored encrypted envelope byte-for-byte, not only its plaintext.
    sqlx::query_scalar(
        "select convert_to(encrypted_value::text, 'UTF8') from plugin_credentials \
         where publisher_namespace='taichuy' and plugin_code='ssh' \
         and scope_id=$1 and credential_id=$2",
    )
    .bind(scope)
    .bind(credential_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn physical_identity(pool: &PgPool) -> (i64, String, String) {
    sqlx::query_as(
        "select c.oid::bigint, n.nspname::text, c.relname::text \
         from pg_class c join pg_namespace n on n.oid=c.relnamespace \
         where c.oid=to_regclass($1)",
    )
    .bind(TABLE)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn historical_columns(pool: &PgPool) -> Vec<(String, i16)> {
    sqlx::query_as(
        "select attname::text, attnum from pg_attribute \
         where attrelid=to_regclass($1) and attnum>0 and not attisdropped \
         and attname <> 'connection_options' order by attnum",
    )
    .bind(TABLE)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn ssh_0_1_21_rows_and_credentials_survive_0_1_25_provider_projection() {
    let (store, pool) = store().await;
    let credentials = PgPluginCredentialRepository::new(pool.clone(), MASTER_KEY).unwrap();
    let scope = Uuid::now_v7();
    let installation = Uuid::now_v7();
    let host_id = Uuid::now_v7().to_string();
    let credential_id = Uuid::now_v7().to_string();
    let old = binding(scope, installation);
    let old_credential = PluginCredentialBinding {
        contribution_id: "ssh.hosts.test".into(),
        installation_id: installation.to_string(),
        publisher_namespace: old.publisher_namespace.clone(),
        plugin_code: old.plugin_code.clone(),
        plugin_version: OLD_VERSION.into(),
        scope_id: scope.to_string(),
        deadline_unix_ms: i64::MAX,
    };

    store
        .apply_managed_schema(&schema(OLD_VERSION))
        .await
        .unwrap();
    let values = BTreeMap::from([
        ("host_id".into(), PluginDataValue::String(host_id.clone())),
        (
            "name".into(),
            PluginDataValue::String("Saved SSH host".into()),
        ),
        (
            "credential_id".into(),
            PluginDataValue::String(credential_id.clone()),
        ),
        (
            "legacy_note".into(),
            PluginDataValue::String("saved before resource export".into()),
        ),
    ]);
    store
        .execute(
            &old,
            &one(PluginDataOperation::Insert {
                target: target(),
                values,
            }),
        )
        .await
        .unwrap();
    let old_read = find(
        &host_id,
        &["id", "host_id", "name", "credential_id", "legacy_note"],
    );
    let before_rows = store.execute(&old, &old_read).await.unwrap();
    let PluginDataOperationResult::Rows { rows } = &before_rows.results[0] else {
        panic!("expected saved SSH row");
    };
    assert_eq!(rows.len(), 1);
    let mut expected_new_values = rows[0].values.clone();
    expected_new_values.remove("legacy_note");
    expected_new_values.insert("connection_options".into(), PluginDataValue::Null);

    let secret = json!({"password": "saved-ssh-password", "private_key": null, "passphrase": null});
    credentials
        .execute_plugin_credential(
            &old_credential,
            &PluginCredentialRequest::Put {
                credential_id: credential_id.clone(),
                value: secret.clone(),
            },
        )
        .await
        .unwrap();
    let get = PluginCredentialRequest::Get {
        credential_id: credential_id.clone(),
    };
    assert_eq!(
        credentials
            .execute_plugin_credential(&old_credential, &get)
            .await
            .unwrap()
            .value,
        Some(secret.clone())
    );
    let before_ciphertext = ciphertext(&pool, scope, &credential_id).await;
    assert!(!String::from_utf8_lossy(&before_ciphertext).contains("saved-ssh-password"));
    let before_aad = aad(&old_credential, &credential_id);
    assert_eq!(
        before_aad,
        serde_json::to_vec(&(
            "plugin_credential/v1",
            "taichuy",
            "ssh",
            scope.to_string(),
            &credential_id
        ))
        .unwrap()
    );
    let before_identity = physical_identity(&pool).await;
    assert_eq!(before_identity.2, TABLE);
    let before_columns = historical_columns(&pool).await;
    let before_ownership = store.list_managed_schema_ownership().await.unwrap();
    assert_eq!(before_ownership.len(), 5);

    // A new export is a projection of the original provider, not a new data owner.
    // No runtime registry or process-global environment mutation is required here.
    store
        .apply_managed_schema(&schema(NEW_VERSION))
        .await
        .unwrap();
    let mut new = old.clone();
    new.plugin_version = NEW_VERSION.into();
    let mut new_credential = old_credential.clone();
    new_credential.plugin_version = NEW_VERSION.into();
    let after_rows = store
        .execute(
            &new,
            &find(
                &host_id,
                &[
                    "id",
                    "host_id",
                    "name",
                    "credential_id",
                    "connection_options",
                ],
            ),
        )
        .await
        .unwrap();
    let PluginDataOperationResult::Rows { rows } = &after_rows.results[0] else {
        panic!("expected historical SSH row through new version projection");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values, expected_new_values);
    assert_eq!(
        rows[0].values["host_id"],
        PluginDataValue::String(host_id.clone())
    );
    assert_eq!(
        rows[0].values["name"],
        PluginDataValue::String("Saved SSH host".into())
    );
    assert_eq!(
        rows[0].values["credential_id"],
        PluginDataValue::String(credential_id.clone())
    );
    assert_eq!(store.execute(&old, &old_read).await.unwrap(), before_rows);
    // This fails if the new binding accidentally selects the old projection or
    // simply exposes every physically retained column.
    assert_eq!(
        store
            .execute(&new, &find(&host_id, &["legacy_note"]))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::OwnershipDenied
    );
    assert_eq!(
        store
            .execute(&old, &find(&host_id, &["connection_options"]))
            .await
            .unwrap_err()
            .kind,
        PluginDataErrorKind::OwnershipDenied
    );
    assert_eq!(physical_identity(&pool).await, before_identity);
    assert_eq!(historical_columns(&pool).await, before_columns);
    let after_ownership = store.list_managed_schema_ownership().await.unwrap();
    assert_eq!(after_ownership.len(), 6);
    for historical in &before_ownership {
        let current = after_ownership
            .iter()
            .find(|row| row.ownership_key == historical.ownership_key)
            .expect("historical ownership must not be moved or deleted");
        assert_eq!(current.owner_id, OWNER);
        assert_eq!(current.owner_version, NEW_VERSION);
        assert_eq!(current.logical_name, historical.logical_name);
        assert_eq!(current.physical_table, historical.physical_table);
        assert_eq!(current.physical_column, historical.physical_column);
        assert_eq!(
            current.active,
            current.physical_column.as_deref() != Some("legacy_note")
        );
    }

    assert_eq!(aad(&new_credential, &credential_id), before_aad);
    assert_eq!(
        credentials
            .execute_plugin_credential(&new_credential, &get)
            .await
            .unwrap()
            .value,
        Some(secret.clone())
    );
    let encrypted: Value = serde_json::from_slice(&before_ciphertext).unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(
            &encrypted,
            MASTER_KEY,
            &aad(&new_credential, &credential_id)
        )
        .unwrap(),
        secret
    );
    for wrong_axis in ["publisher", "plugin"] {
        let mut foreign_data = new.clone();
        let mut foreign_credential = new_credential.clone();
        match wrong_axis {
            "publisher" => {
                foreign_data.publisher_namespace = "other".into();
                foreign_credential.publisher_namespace = "other".into();
            }
            _ => {
                // Even the export code is not an alternative physical/credential owner.
                foreign_data.plugin_code = "ssh-hosts-v1".into();
                foreign_credential.plugin_code = "ssh-hosts-v1".into();
            }
        }
        assert_eq!(
            store
                .execute(&foreign_data, &find(&host_id, &["host_id", "name"]))
                .await
                .unwrap_err()
                .kind,
            PluginDataErrorKind::OwnershipDenied
        );
        assert!(credentials
            .execute_plugin_credential(&foreign_credential, &get)
            .await
            .unwrap()
            .value
            .is_none());
        assert!(decrypt_secret_json_with_aad(
            &encrypted,
            MASTER_KEY,
            &aad(&foreign_credential, &credential_id)
        )
        .is_err());
    }
    // Neither schema reconciliation, new-version reads nor denied foreign reads
    // may rotate/re-encrypt/copy the historical credential.
    assert_eq!(
        ciphertext(&pool, scope, &credential_id).await,
        before_ciphertext
    );
    let credential_count: i64 = sqlx::query_scalar("select count(*) from plugin_credentials")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(credential_count, 1);
}
