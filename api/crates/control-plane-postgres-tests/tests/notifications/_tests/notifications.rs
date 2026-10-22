use control_plane::{
    notifications::{NotificationSenderPort, NotificationService, PatchNotificationInstance},
    ports::{CreateMemberInput, NotificationMessageListInput},
};
use domain::NotificationInstanceStatus;
use serde_json::json;
use uuid::Uuid;

#[derive(Default)]
struct CountingAvailability {
    calls: std::sync::atomic::AtomicUsize,
    available: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl control_plane::notifications::NotificationProviderPort for CountingAvailability {
    async fn available(
        &self,
        _: &domain::ActorContext,
        _: Uuid,
        _: &extension_contracts::ContributionId,
    ) -> anyhow::Result<bool> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self.available.load(std::sync::atomic::Ordering::SeqCst))
    }

    async fn send(
        &self,
        _: &domain::ActorContext,
        _: Uuid,
        _: &extension_contracts::NotificationTypeDescriptor,
        _: serde_json::Value,
        _: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        panic!("unavailable notification provider must not receive a send")
    }
}

#[tokio::test]
async fn instance_query_shares_catalog_lazily_and_send_checks_fresh_availability() {
    use control_plane::{
        notifications::{system_notification_descriptor, NotificationInstanceListInput},
        ports::{
            CreatePluginAssignmentInput, NotificationRepository, PluginRepository,
            SaveNotificationInstanceInput, UpsertPluginInstallationInput,
        },
    };
    use extension_contracts::NotificationAuthorizationStatus;
    use std::sync::{atomic::Ordering, Arc};

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
    let provider = Arc::new(CountingAvailability::default());
    provider.available.store(true, Ordering::SeqCst);
    let service = NotificationService::new(store.clone(), "catalog-fixture-key".into())
        .with_provider(provider.clone());
    assert!(service.list_instances(&actor).await.unwrap().is_empty());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);

    for (name, type_code, status, authorization) in [
        (
            "Authorized",
            "acme/catalog/notification",
            NotificationInstanceStatus::Enabled,
            Some("authorized"),
        ),
        (
            "Unauthorised",
            "acme/catalog/notification",
            NotificationInstanceStatus::Enabled,
            None,
        ),
        (
            "Disabled",
            "acme/catalog/missing",
            NotificationInstanceStatus::Disabled,
            Some("unauthorized"),
        ),
        (
            "Missing",
            "acme/catalog/missing",
            NotificationInstanceStatus::Enabled,
            Some("authorized"),
        ),
    ] {
        let now = time::OffsetDateTime::now_utc();
        store
            .save_notification_instance(SaveNotificationInstanceInput {
                record: domain::NotificationInstanceRecord {
                    id: Uuid::now_v7(),
                    workspace_id: workspace.id,
                    type_code: type_code.into(),
                    installation_id: Some(installation.id),
                    display_name: name.into(),
                    status,
                    config_json: json!({}),
                    configured_secret_fields: vec![],
                    created_at: now,
                    updated_at: now,
                },
                secret_config_json: json!({}),
                master_key: "catalog-fixture-key".into(),
                create: true,
                initial_authorization: authorization
                    .map(|status| serde_json::from_value(json!({"status":status})).unwrap()),
            })
            .await
            .unwrap();
    }
    for input in [
        NotificationInstanceListInput {
            display_name: Some("absent".into()),
            ..Default::default()
        },
        NotificationInstanceListInput {
            type_code: Some("absent".into()),
            ..Default::default()
        },
        NotificationInstanceListInput {
            display_name: Some("disabled".into()),
            status: Some(NotificationInstanceStatus::Enabled),
            ..Default::default()
        },
    ] {
        assert!(service
            .query_instances(&actor, input)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }
    let records = store
        .list_notification_instances(workspace.id)
        .await
        .unwrap();
    let instances = service.list_instances(&actor).await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        serde_json::to_value(
            instances
                .iter()
                .map(|item| &item.record)
                .collect::<Vec<_>>()
        )
        .unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    assert!(instances[0].available && instances[1].available);
    let authorized = instances[0].authorization.as_ref().unwrap();
    assert_eq!(
        authorized.status,
        NotificationAuthorizationStatus::Authorized
    );
    assert_eq!(
        authorized.delivery_window.as_ref().unwrap().status,
        extension_contracts::NotificationDeliveryWindowStatus::Unknown
    );
    assert_eq!(
        instances[1].authorization.as_ref().unwrap().status,
        NotificationAuthorizationStatus::Unauthorized
    );
    assert_eq!(instances[2].unavailable_reason, Some("instance_disabled"));
    assert_eq!(
        instances[2].authorization.as_ref().unwrap().status,
        NotificationAuthorizationStatus::Unauthorized
    );
    assert_eq!(instances[3].unavailable_reason, Some("type_unavailable"));
    assert_eq!(
        instances[3].authorization.as_ref().unwrap().status,
        NotificationAuthorizationStatus::Authorized
    );
    assert!(instances[3]
        .authorization
        .as_ref()
        .unwrap()
        .delivery_window
        .is_none());

    provider.available.store(false, Ordering::SeqCst);
    let error = service
        .send(
            &actor,
            domain::NotificationSource::Api,
            instances[0].record.id,
            json!({}),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::Conflict(
            "plugin_unavailable"
        ))
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    let unavailable = service.list_instances(&actor).await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert!(unavailable.iter().all(|instance| !instance.available));
    assert_eq!(
        unavailable[0].unavailable_reason,
        Some("plugin_unavailable")
    );
    assert_eq!(unavailable[2].unavailable_reason, Some("instance_disabled"));
    assert_eq!(unavailable[3].unavailable_reason, Some("type_unavailable"));
}

#[tokio::test]
async fn committed_inbox_deduplicates_and_enforces_recipient_workspace_and_management_scope() {
    let (store, workspace, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let member = store
        .create_member_with_default_role(&CreateMemberInput {
            actor_user_id: root.id,
            workspace_id: workspace.id,
            account: "notification-member".into(),
            email: "notification@example.com".into(),
            phone: None,
            password_hash: "hash".into(),
            name: "Member".into(),
            nickname: "Member".into(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        })
        .await
        .unwrap();
    let member_actor = store
        .load_actor_context(member.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), "test-notification-master-key".into());
    service.bootstrap().await.unwrap();
    let instances = service.list_instances(&actor).await.unwrap();
    assert_eq!(instances.len(), 1);
    let id = instances[0].record.id;
    assert!(service.list_instances(&member_actor).await.is_err());
    let catalog = service.types(&member_actor).await.unwrap();
    assert!(catalog[0].descriptor.input_form.fields[0]
        .options
        .iter()
        .any(|option| option.value == json!(member.id)));
    let sent = service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"user_ids":[root.id,member.id,member.id],"title":"Ready","content":"Committed"}),
        )
        .await
        .unwrap();
    assert_eq!(sent.result["recipient_count"], 2);
    let logs = service.logs(&actor, Default::default()).await.unwrap();
    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].status, "success");
    assert_eq!(logs.items[0].recipient_count, Some(2));
    assert!(service
        .logs(&member_actor, Default::default())
        .await
        .is_err());
    assert_eq!(service.unread_count(&actor).await.unwrap(), 1);
    assert_eq!(service.unread_count(&member_actor).await.unwrap(), 1);
    let root_page = service
        .messages(&actor, NotificationMessageListInput::default())
        .await
        .unwrap();
    let member_page = service
        .messages(&member_actor, NotificationMessageListInput::default())
        .await
        .unwrap();
    assert_eq!(member_page.items.len(), 1);
    assert!(service
        .read(&member_actor, root_page.items[0].id)
        .await
        .is_err());
    assert!(service
        .unread(&member_actor, root_page.items[0].id)
        .await
        .is_err());
    assert_eq!(service.read_channel(&member_actor, id).await.unwrap(), 1);
    assert_eq!(service.unread_count(&actor).await.unwrap(), 1);
    service
        .unread(&member_actor, member_page.items[0].id)
        .await
        .unwrap();
    let read = service
        .read(&member_actor, member_page.items[0].id)
        .await
        .unwrap();
    assert_eq!(
        service.read(&member_actor, read.id).await.unwrap().read_at,
        read.read_at
    );
    assert_eq!(service.read_all(&member_actor).await.unwrap(), 0);
    assert_eq!(service.read_all(&actor).await.unwrap(), 1);
    for input in [
        json!({"user_ids":[],"title":"t","content":"c"}),
        json!({"user_ids":[Uuid::now_v7()],"title":"t","content":"c"}),
        json!({"user_ids":[root.id],"title":"","content":"c"}),
    ] {
        assert!(service
            .send(&actor, domain::NotificationSource::Api, id, input)
            .await
            .is_err());
    }
    assert_eq!(
        service
            .messages(&actor, Default::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    service
        .patch_instance(
            &actor,
            id,
            PatchNotificationInstance {
                status: Some(NotificationInstanceStatus::Disabled),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"user_ids":[root.id],"title":"t","content":"c"})
        )
        .await
        .is_err());
    let other = store
        .upsert_workspace(workspace.tenant_id, "Other notification workspace")
        .await
        .unwrap();
    let foreign_actor = store
        .load_actor_context(root.id, workspace.tenant_id, other.id, None)
        .await
        .unwrap();
    // Runtime workspace creation must provision its default instance without a restart/bootstrap.
    let other_instances = service.list_instances(&foreign_actor).await.unwrap();
    assert_eq!(other_instances.len(), 1);
    assert_eq!(other_instances[0].record.type_code, "system");
    assert_eq!(other_instances[0].record.workspace_id, other.id);
    assert_eq!(
        other_instances[0].record.status,
        NotificationInstanceStatus::Enabled
    );
    assert!(service.get_instance(&foreign_actor, id).await.is_err());
    assert!(service
        .read(&foreign_actor, root_page.items[0].id)
        .await
        .is_err());
    assert_eq!(service.unread_count(&foreign_actor).await.unwrap(), 0);
    let failure_logs = service
        .logs(
            &actor,
            control_plane::ports::NotificationLogListInput {
                status: Some("failure".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(failure_logs.total, 4);
    assert!(failure_logs
        .items
        .iter()
        .all(|log| log.title.is_none() && log.content.is_none()));
    assert!(service
        .log(&foreign_actor, failure_logs.items[0].id)
        .await
        .is_err());
    service.delete_instance(&actor, id).await.unwrap();
    assert_eq!(
        service
            .messages(&actor, Default::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"user_ids":[root.id],"title":"t","content":"c"})
        )
        .await
        .is_err());
    let missing = service
        .logs(
            &actor,
            control_plane::ports::NotificationLogListInput {
                status: Some("failure".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        missing.items[0].failure_code.as_deref(),
        Some("instance_missing")
    );
    assert!(missing.items[0].display_name.is_none());
}

#[tokio::test]
async fn credentials_are_encrypted_bound_to_instance_and_excluded_from_public_records() {
    use control_plane::ports::{NotificationRepository, SaveNotificationInstanceInput};
    let (store, workspace, _) = super::support::seed_store().await;
    let now = time::OffsetDateTime::now_utc();
    let id = Uuid::now_v7();
    let record = domain::NotificationInstanceRecord {
        id,
        workspace_id: workspace.id,
        type_code: "fixture/provider/email".into(),
        installation_id: Some(Uuid::now_v7()),
        display_name: "Email".into(),
        status: NotificationInstanceStatus::Enabled,
        config_json: json!({"endpoint":"https://example.test"}),
        configured_secret_fields: vec!["token".into()],
        created_at: now,
        updated_at: now,
    };
    let key = "notification-fixture-master-key";
    let saved = store
        .save_notification_instance(SaveNotificationInstanceInput {
            record,
            secret_config_json: json!({"token":"private-token"}),
            master_key: key.into(),
            create: true,
            initial_authorization: None,
        })
        .await
        .unwrap();
    assert!(!serde_json::to_string(&saved)
        .unwrap()
        .contains("private-token"));
    let encrypted: serde_json::Value =
        sqlx::query_scalar("SELECT encrypted_secret_json FROM notification_instances WHERE id=$1")
            .bind(id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(!encrypted.to_string().contains("private-token"));
    assert_eq!(
        store
            .resolve_notification_secret_config(workspace.id, id, key)
            .await
            .unwrap(),
        json!({"token":"private-token"})
    );
    assert!(store
        .resolve_notification_secret_config(workspace.id, id, "wrong-key")
        .await
        .is_err());
    let other_id = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_instances (id,workspace_id,type_code,display_name,encrypted_secret_json) VALUES ($1,$2,'system','copied',$3)").bind(other_id).bind(workspace.id).bind(encrypted).execute(store.pool()).await.unwrap();
    assert!(store
        .resolve_notification_secret_config(workspace.id, other_id, key)
        .await
        .is_err());
}

#[tokio::test]
async fn notification_managed_secrets_are_workspace_bound_encrypted_and_revision_fenced() {
    use control_plane::ports::{
        NotificationRepository, SaveNotificationAuthorizationInput, SaveNotificationInstanceInput,
    };
    use extension_contracts::NotificationAuthorizationView;
    let (store, workspace, _) = super::support::seed_store().await;
    let now = time::OffsetDateTime::now_utc();
    let id = Uuid::now_v7();
    let view: NotificationAuthorizationView =
        serde_json::from_value(json!({"status":"unauthorized"})).unwrap();
    store
        .save_notification_instance(SaveNotificationInstanceInput {
            record: domain::NotificationInstanceRecord {
                id,
                workspace_id: workspace.id,
                type_code: "fixture/managed/notification".into(),
                installation_id: Some(Uuid::now_v7()),
                display_name: "Managed".into(),
                status: NotificationInstanceStatus::Disabled,
                config_json: json!({}),
                configured_secret_fields: vec![],
                created_at: now,
                updated_at: now,
            },
            secret_config_json: json!({}),
            master_key: "managed-fixture-key".into(),
            create: true,
            initial_authorization: Some(view.clone()),
        })
        .await
        .unwrap();
    let save = |expected_revision, authorization| SaveNotificationAuthorizationInput {
        workspace_id: workspace.id,
        instance_id: id,
        expected_revision,
        authorization,
        managed_secrets: std::collections::BTreeMap::from([(
            "bot_token".into(),
            json!("private-managed-bot-token"),
        )]),
        master_key: "managed-fixture-key".into(),
    };
    let initial = store
        .resolve_notification_authorization(workspace.id, id, "managed-fixture-key")
        .await
        .unwrap();
    assert_eq!(initial.revision, 0);
    assert!(initial.managed_secrets.is_empty());
    let authorized: NotificationAuthorizationView =
        serde_json::from_value(json!({"status":"authorized"})).unwrap();
    assert_eq!(
        store
            .save_notification_authorization(save(0, authorized.clone()))
            .await
            .unwrap(),
        Some(1)
    );
    // CAS rejects stale poll and any unrelated workspace write without replacing a new state.
    assert!(store
        .save_notification_authorization(save(0, view))
        .await
        .unwrap()
        .is_none());
    let mut foreign = save(1, authorized);
    foreign.workspace_id = Uuid::now_v7();
    assert!(store
        .save_notification_authorization(foreign)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .resolve_notification_authorization(Uuid::now_v7(), id, "managed-fixture-key")
        .await
        .is_err());
    assert!(store
        .resolve_notification_authorization(workspace.id, id, "wrong-master-key")
        .await
        .is_err());
    let restored = store
        .resolve_notification_authorization(workspace.id, id, "managed-fixture-key")
        .await
        .unwrap();
    assert_eq!(restored.revision, 1);
    assert_eq!(
        restored.managed_secrets["bot_token"],
        "private-managed-bot-token"
    );
    let record = store
        .get_notification_instance(workspace.id, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, NotificationInstanceStatus::Disabled);
    assert!(!serde_json::to_string(&record)
        .unwrap()
        .contains("private-managed-bot-token"));
    let cipher: serde_json::Value = sqlx::query_scalar(
        "SELECT encrypted_managed_secret_json FROM notification_instances WHERE id=$1",
    )
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(!cipher.to_string().contains("private-managed-bot-token"));
    // Moving ciphertext between identity domains cannot recover credentials.
    let other = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_instances(id,workspace_id,type_code,display_name,encrypted_managed_secret_json) VALUES($1,$2,'fixture','Other',$3)").bind(other).bind(workspace.id).bind(cipher).execute(store.pool()).await.unwrap();
    assert!(store
        .resolve_notification_authorization(workspace.id, other, "managed-fixture-key")
        .await
        .is_err());
}
