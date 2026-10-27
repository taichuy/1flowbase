use super::*;
use serde_json::Value;
use std::time::Duration;

async fn fixture() -> (PgControlPlaneStore, Uuid, ManagedInstallationSwitch, Uuid) {
    let store = store().await;
    let actor = actor(&store).await;
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "settings-fixture")
        .await
        .unwrap()
        .id;
    let mut installations = Vec::new();
    for version in ["8.0.0", "9.0.0"] {
        let mut input = managed_input(actor, version);
        input.installation.metadata_json = json!({"binding_targets":["workspace"]});
        input.settings_templates.clear();
        installations.push(store.commit_plugin_installation(&input).await.unwrap());
    }
    let current = &installations[0];
    let target = &installations[1];
    store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: current.id,
            workspace_id: workspace,
            provider_code: current.provider_code.clone(),
            actor_user_id: actor,
        })
        .await
        .unwrap();
    desired(
        &store,
        current.id,
        actor,
        PluginDesiredState::ActiveRequested,
    )
    .await
    .unwrap();
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id,workspace_id) values($1,$2)")
        .bind(target.id).bind(workspace).execute(store.pool()).await.unwrap();
    let instance = Uuid::now_v7();
    sqlx::query("insert into notification_instances(id,workspace_id,type_code,installation_id,display_name,config_json,encrypted_secret_json,encrypted_managed_secret_json,authorization_json,authorization_revision) values($1,$2,'fixture/private',$3,'Keep channel','{\"mode\":\"private\"}','{\"ciphertext\":\"config\"}','{\"ciphertext\":\"managed\"}','{\"status\":\"authorized\"}',7)")
        .bind(instance).bind(workspace).bind(current.id).execute(store.pool()).await.unwrap();
    (
        store,
        actor,
        ManagedInstallationSwitch {
            workspace_id: workspace,
            current_installation_id: current.id,
            target_installation_id: target.id,
            node_id: "test-node".into(),
            installation_ids: vec![current.id, target.id],
        },
        instance,
    )
}

async fn state(store: &PgControlPlaneStore, instance: Uuid) -> Value {
    sqlx::query_scalar("select to_jsonb(n) from notification_instances n where id=$1")
        .bind(instance)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn notification_switch_preserves_channel_credentials_history_and_scope() {
    let (store, actor, command, instance) = fixture().await;
    let disabled = Uuid::now_v7();
    let unrelated = Uuid::now_v7();
    let other_workspace = store
        .upsert_workspace(store.upsert_root_tenant().await.unwrap().id, "other")
        .await
        .unwrap()
        .id;
    let outside = Uuid::now_v7();
    for (id, workspace, installation, status) in [
        (
            disabled,
            command.workspace_id,
            command.current_installation_id,
            "disabled",
        ),
        (
            unrelated,
            command.workspace_id,
            command.target_installation_id,
            "enabled",
        ),
        (
            outside,
            other_workspace,
            command.current_installation_id,
            "enabled",
        ),
    ] {
        sqlx::query("insert into notification_instances(id,workspace_id,type_code,installation_id,display_name,status) values($1,$2,'fixture/private',$3,'Other',$4)")
            .bind(id).bind(workspace).bind(installation).bind(status).execute(store.pool()).await.unwrap();
    }
    let conversation = Uuid::now_v7();
    sqlx::query("insert into communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) values($1,$2,$3,'user','chat')")
        .bind(conversation).bind(command.workspace_id).bind(instance).execute(store.pool()).await.unwrap();
    sqlx::query("insert into communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) values($1,$2,$3,'user',$4,$1,'text','inbound',1,'Keep history')")
        .bind(Uuid::now_v7()).bind(command.workspace_id).bind(instance).bind(conversation).execute(store.pool()).await.unwrap();
    let before = state(&store, instance).await;
    let outside_before = state(&store, outside).await;
    let unrelated_before = state(&store, unrelated).await;
    let count = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit_with_catalog(
            audit(actor, command.target_installation_id),
            None,
            vec!["fixture/private".into()],
        )
        .await
        .unwrap();
    assert_eq!(count, 2);
    let mut after = state(&store, instance).await;
    assert_eq!(
        after["installation_id"],
        json!(command.target_installation_id)
    );
    assert_eq!(after["authorization_revision"], 8);
    for key in ["installation_id", "authorization_revision", "updated_at"] {
        after[key] = before[key].clone();
    }
    assert_eq!(
        after, before,
        "all other channel fields, including ciphertext, survive"
    );
    let disabled_after = state(&store, disabled).await;
    assert_eq!(
        disabled_after["installation_id"],
        json!(command.target_installation_id)
    );
    assert_eq!(disabled_after["status"], "disabled");
    assert_eq!(state(&store, outside).await, outside_before);
    assert_eq!(state(&store, unrelated).await, unrelated_before);
    assert_eq!(
        store.list_assignments(command.workspace_id).await.unwrap()[0].installation_id,
        command.target_installation_id
    );
    let history: String =
        sqlx::query_scalar("select content from communication_messages where conversation_id=$1")
            .bind(conversation)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(history, "Keep history");
    let stale = sqlx::query("update notification_instances set authorization_json='{}' where id=$1 and authorization_revision=7")
        .bind(instance).execute(store.pool()).await.unwrap();
    assert_eq!(stale.rows_affected(), 0, "old-version writes are fenced");
    let count: i64 = sqlx::query_scalar("select (payload->>'migrated_instance_count')::bigint from audit_logs where event_code='plugin.version_switched'")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn notification_switch_rejects_incompatible_channels_and_rolls_back_audit_failure() {
    let (store, actor, command, instance) = fixture().await;
    let before = state(&store, instance).await;
    let error = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit(audit(actor, command.target_installation_id))
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("notification_type_migration_incompatible"));
    assert_eq!(state(&store, instance).await, before);
    assert_eq!(
        store.list_assignments(command.workspace_id).await.unwrap()[0].installation_id,
        command.current_installation_id
    );
    sqlx::query("create function reject_channel_switch() returns trigger language plpgsql as $$ begin raise exception 'fixture audit rejected'; end $$")
        .execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_channel_switch before insert on audit_logs for each row execute function reject_channel_switch()")
        .execute(store.pool()).await.unwrap();
    let error = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap()
        .commit_with_catalog(
            audit(actor, command.target_installation_id),
            None,
            vec!["fixture/private".into()],
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fixture audit rejected"));
    assert_eq!(state(&store, instance).await, before);
    assert_eq!(
        store.list_assignments(command.workspace_id).await.unwrap()[0].installation_id,
        command.current_installation_id
    );
}

#[tokio::test]
async fn notification_switch_serializes_creation_and_rejects_stale_installation() {
    let (store, actor, command, instance) = fixture().await;
    let mut record = store
        .get_notification_instance(command.workspace_id, instance)
        .await
        .unwrap()
        .unwrap();
    record.id = Uuid::now_v7();
    let lease = store
        .lock_managed_installation_switch(&command)
        .await
        .unwrap();
    let writer_store = store.clone();
    let mut writer = tokio::spawn(async move {
        writer_store
            .save_notification_instance(SaveNotificationInstanceInput {
                record,
                secret_config_json: json!({}),
                master_key: "fixture".into(),
                create: true,
                initial_authorization: None,
            })
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut writer)
            .await
            .is_err()
    );
    lease
        .commit_with_catalog(
            audit(actor, command.target_installation_id),
            None,
            vec!["fixture/private".into()],
        )
        .await
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(5), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("type_unavailable"));
}
