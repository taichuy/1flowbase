use super::*;
use crate::secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad};
use serde_json::json;

#[test]
fn notification_rekey_preserves_bound_identity_and_never_accepts_wrong_keys() {
    let workspace = Uuid::now_v7();
    let instance = Uuid::now_v7();
    let aad = crate::notification_repository::aad(workspace, instance);
    let plain = json!({"token":"notification-backup-secret"});
    let managed_aad = crate::notification_repository::managed_aad(workspace, instance);
    let managed = json!({"bot_token":"managed-backup-secret","context_token":"private-context"});
    let original = json!({
        "id":instance,"workspace_id":workspace,
        "encrypted_secret_json":encrypt_secret_json_with_aad(&plain,"source",&aad).unwrap(),
        "encrypted_managed_secret_json":encrypt_secret_json_with_aad(&managed,"source",&managed_aad).unwrap()
    });
    let mut same = original.clone();
    validation::rekey("notification_instances", &mut same, "source", "source").unwrap();
    assert_eq!(same, original);
    let mut rekeyed = original.clone();
    validation::rekey("notification_instances", &mut rekeyed, "source", "target").unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(&rekeyed["encrypted_secret_json"], "target", &aad).unwrap(),
        plain
    );
    assert!(
        decrypt_secret_json_with_aad(&rekeyed["encrypted_secret_json"], "source", &aad).is_err()
    );
    assert!(!rekeyed.to_string().contains("notification-backup-secret"));
    assert!(!rekeyed.to_string().contains("managed-backup-secret"));
    assert_eq!(
        decrypt_secret_json_with_aad(
            &rekeyed["encrypted_managed_secret_json"],
            "target",
            &managed_aad
        )
        .unwrap(),
        managed
    );
    assert!(decrypt_secret_json_with_aad(
        &rekeyed["encrypted_managed_secret_json"],
        "target",
        &aad
    )
    .is_err());
    assert!(validation::rekey(
        "notification_instances",
        &mut original.clone(),
        "wrong",
        "target"
    )
    .is_err());
    for field in ["id", "workspace_id"] {
        let mut tampered = original.clone();
        tampered[field] = json!(Uuid::now_v7());
        assert!(
            validation::rekey("notification_instances", &mut tampered, "source", "target").is_err()
        );
    }
    let mut builtin = json!({"encrypted_secret_json":null});
    validation::rekey("notification_instances", &mut builtin, "source", "target").unwrap();
    assert_eq!(builtin, json!({"encrypted_secret_json":null}));
}

#[tokio::test]
async fn selective_notification_configuration_roundtrips_credentials_without_inbox_history() {
    let (db, actor) = fixture().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Notification backup")
        .await
        .unwrap();
    let instance = Uuid::now_v7();
    let aad = crate::notification_repository::aad(workspace.id, instance);
    let plain = json!({"token":"notification-backup-secret"});
    let encrypted = encrypt_secret_json_with_aad(&plain, "source", &aad).unwrap();
    let managed_aad = crate::notification_repository::managed_aad(workspace.id, instance);
    let managed_plain = json!({"bot_token":"notification-managed-backup-secret"});
    let managed = encrypt_secret_json_with_aad(&managed_plain, "source", &managed_aad).unwrap();
    sqlx::query("insert into notification_instances(id,workspace_id,type_code,display_name,encrypted_secret_json,configured_secret_fields) values($1,$2,'fixture','Original',$3,'[\"token\"]'::jsonb)")
        .bind(instance).bind(workspace.id).bind(encrypted).execute(&db).await.unwrap();
    sqlx::query("UPDATE notification_instances SET encrypted_managed_secret_json=$2,authorization_json='{\"status\":\"authorized\"}'::jsonb,authorization_revision=7 WHERE id=$1").bind(instance).bind(managed).execute(&db).await.unwrap();
    let message = Uuid::now_v7();
    sqlx::query("insert into system_notifications(id,workspace_id,instance_id,recipient_user_id,title,content) values($1,$2,$3,$4,'Inbox','Original inbox')")
        .bind(message).bind(workspace.id).bind(instance).bind(actor).execute(&db).await.unwrap();
    let send_log = Uuid::now_v7();
    sqlx::query("INSERT INTO notification_send_logs(id,workspace_id,actor_user_id,instance_id,source,status,title) VALUES($1,$2,$3,$4,'api','success','Original send')")
        .bind(send_log).bind(workspace.id).bind(actor).bind(instance).execute(&db).await.unwrap();
    let repository = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repository, select("notifications", true, false)).await;
    let full_bytes = capture(&repository, select("notifications", true, true)).await;
    let preview = repository
        .preflight(reader(bytes.clone()), "source", "target")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert_eq!(
        preview.selected_tables,
        vec![
            "communication_bindings".to_owned(),
            "notification_instances".to_owned()
        ]
    );
    let full_preview = repository
        .preflight(reader(full_bytes.clone()), "source", "target")
        .await
        .unwrap();
    assert!(
        full_preview.failures.is_empty(),
        "{:?}",
        full_preview.failures
    );
    assert!(full_preview
        .selected_tables
        .contains(&"notification_send_logs".to_owned()));
    assert!(full_preview
        .selected_tables
        .contains(&"system_notifications".to_owned()));
    sqlx::query("UPDATE notification_send_logs SET title='Target send' WHERE id=$1")
        .bind(send_log)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("update notification_instances set display_name='Changed' where id=$1")
        .bind(instance)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("update system_notifications set content='Target inbox' where id=$1")
        .bind(message)
        .execute(&db)
        .await
        .unwrap();
    assert!(repository
        .restore(reader(bytes.clone()), "wrong", "target", true)
        .await
        .is_err());
    let unchanged: String =
        sqlx::query_scalar("select display_name from notification_instances where id=$1")
            .bind(instance)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        unchanged, "Changed",
        "invalid source key must not overwrite target configuration"
    );
    let pending = repository
        .prepare_restore(reader(full_bytes.clone()), "source", "target", true)
        .await
        .unwrap();
    pending.rollback().await.unwrap();
    let unchanged_inbox: String =
        sqlx::query_scalar("select content from system_notifications where id=$1")
            .bind(message)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        unchanged_inbox, "Target inbox",
        "rollback must preserve target history"
    );
    repository
        .restore(reader(bytes), "source", "target", true)
        .await
        .unwrap();
    let name: String =
        sqlx::query_scalar("select display_name from notification_instances where id=$1")
            .bind(instance)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(name, "Original");
    let restored = sqlx::query_scalar::<_, serde_json::Value>(
        "select encrypted_secret_json from notification_instances where id=$1",
    )
    .bind(instance)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(&restored, "target", &aad).unwrap(),
        plain
    );
    let restored_managed: serde_json::Value = sqlx::query_scalar(
        "select encrypted_managed_secret_json from notification_instances where id=$1",
    )
    .bind(instance)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(&restored_managed, "target", &managed_aad).unwrap(),
        managed_plain
    );
    let revision: i64 =
        sqlx::query_scalar("select authorization_revision from notification_instances where id=$1")
            .bind(instance)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(revision, 7);
    let inbox: String = sqlx::query_scalar("select content from system_notifications where id=$1")
        .bind(message)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(inbox, "Target inbox");
    let target_send: String =
        sqlx::query_scalar("SELECT title FROM notification_send_logs WHERE id=$1")
            .bind(send_log)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        target_send, "Target send",
        "config-only restore and rolled-back full restore must preserve send history"
    );

    sqlx::query("update system_notifications set read_at=now() where id=$1")
        .bind(message)
        .execute(&db)
        .await
        .unwrap();
    repository
        .restore(reader(full_bytes), "source", "target", true)
        .await
        .unwrap();
    let (content, unread): (String, bool) =
        sqlx::query_as("select content,read_at IS NULL from system_notifications where id=$1")
            .bind(message)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(content, "Original inbox");
    let original_send: String =
        sqlx::query_scalar("SELECT title FROM notification_send_logs WHERE id=$1")
            .bind(send_log)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        original_send, "Original send",
        "data restore must restore durable send history"
    );

    assert!(
        unread,
        "data restore must restore the unread acknowledgement state"
    );
}
