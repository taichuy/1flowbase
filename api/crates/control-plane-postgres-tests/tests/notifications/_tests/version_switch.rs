use control_plane::{
    notifications::{system_notification_descriptor, NotificationService},
    ports::*,
};
use domain::NotificationInstanceStatus;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

struct SwitchDuringAvailability {
    store: storage_durable_postgres::PgControlPlaneStore,
    instance_id: Uuid,
    target: Uuid,
    switched: AtomicBool,
}
#[async_trait::async_trait]
impl control_plane::notifications::NotificationProviderPort for SwitchDuringAvailability {
    async fn available(
        &self,
        actor: &domain::ActorContext,
        old: Uuid,
        _: &extension_contracts::ContributionId,
    ) -> anyhow::Result<bool> {
        if !self.switched.swap(true, Ordering::SeqCst) {
            // Deterministic interleaving: service already read the old channel/descriptor;
            // commit a version switch before it resolves the authorization snapshot.
            let mut tx = self.store.pool().begin().await?;
            sqlx::query("update plugin_assignments set installation_id=$3 where workspace_id=$1 and installation_id=$2")
                .bind(actor.current_workspace_id).bind(old).bind(self.target).execute(&mut *tx).await?;
            sqlx::query("update notification_instances set installation_id=$2,authorization_revision=authorization_revision+1,updated_at=now() where id=$1")
                .bind(self.instance_id).bind(self.target).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        Ok(true)
    }
    async fn send(
        &self,
        _: &domain::ActorContext,
        _: Uuid,
        _: &extension_contracts::NotificationTypeDescriptor,
        _: Value,
        _: Value,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("old installation must not receive a dispatch")
    }
}

#[tokio::test]
async fn version_switch_between_channel_and_authorization_reads_preserves_credentials() {
    let (store, workspace, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let mut descriptor = system_notification_descriptor();
    descriptor.type_code = "acme/catalog/notification".into();
    descriptor.contribution_id = extension_contracts::ContributionId::new("acme.catalog").unwrap();
    descriptor.authorization = Some(
        serde_json::from_value(json!({
            "actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],
            "managed_secret_keys":["token"], "supports_delivery_window":true
        }))
        .unwrap(),
    );
    let installation = store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "acme".into(),
            provider_code: "catalog".into(),
            plugin_id: "acme.catalog".into(),
            plugin_version: "1.0.0".into(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Catalog fixture".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"notification_types":[descriptor]}),
            is_system_reserved: false,
            actor_user_id: root.id,
        })
        .await
        .unwrap();
    store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: workspace.id,
            provider_code: installation.provider_code,
            actor_user_id: root.id,
        })
        .await
        .unwrap();

    let target = Uuid::now_v7();
    sqlx::query("insert into extension_installations select (jsonb_populate_record(null::extension_installations, to_jsonb(i) || jsonb_build_object('id',$2::uuid,'artifact_version','2.0.0','plugin_id','acme.catalog@2.0.0'))).* from extension_installations i where id=$1")
        .bind(installation.id).bind(target).execute(store.pool()).await.unwrap();
    let id = Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    store
        .save_notification_instance(SaveNotificationInstanceInput {
            record: domain::NotificationInstanceRecord {
                id,
                workspace_id: workspace.id,
                type_code: "acme/catalog/notification".into(),
                installation_id: Some(installation.id),
                display_name: "Keep authorization".into(),
                status: NotificationInstanceStatus::Enabled,
                config_json: json!({}),
                configured_secret_fields: vec![],
                created_at: now,
                updated_at: now,
            },
            secret_config_json: json!({}),
            master_key: "fixture".into(),
            create: true,
            initial_authorization: None,
        })
        .await
        .unwrap();
    store
        .save_notification_authorization(SaveNotificationAuthorizationInput {
            workspace_id: workspace.id,
            instance_id: id,
            expected_revision: 0,
            authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
            managed_secrets: std::collections::BTreeMap::from([(
                "token".into(),
                json!("keep-token"),
            )]),
            master_key: "fixture".into(),
        })
        .await
        .unwrap()
        .unwrap();
    let before: Value = sqlx::query_scalar(
        "select encrypted_managed_secret_json from notification_instances where id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let service = NotificationService::new(store.clone(), "fixture".into()).with_provider(
        Arc::new(SwitchDuringAvailability {
            store: store.clone(),
            instance_id: id,
            target,
            switched: AtomicBool::new(false),
        }),
    );
    let error = service
        .authorize_instance(
            &actor,
            id,
            extension_contracts::NotificationAuthorizationOperation::Begin {
                action: "qr".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("notification_authorization_stale"),
        "{error}"
    );
    let after = store
        .resolve_notification_authorization(workspace.id, id, "fixture")
        .await
        .unwrap();
    assert_eq!(after.installation_id, Some(target));
    assert_eq!(after.revision, 2);
    assert_eq!(
        after.authorization.unwrap().status,
        extension_contracts::NotificationAuthorizationStatus::Authorized
    );
    assert_eq!(after.managed_secrets["token"], "keep-token");
    let ciphertext: Value = sqlx::query_scalar(
        "select encrypted_managed_secret_json from notification_instances where id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(ciphertext, before);
}
