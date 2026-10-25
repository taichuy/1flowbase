use super::*;
use crate::secret_crypto::{decrypt_secret_json_with_aad, encrypt_secret_json_with_aad};
use serde_json::{json, Value};
use std::io::{Read, Write};

fn rewrite_scope(bytes: &[u8], scope: Option<Value>) -> Vec<u8> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for line in text.lines() {
        let mut record: Value = serde_json::from_str(line).unwrap();
        if record["table"] == "communication_attachments" {
            match &scope {
                Some(scope) => {
                    record["row"]["encrypted_reference"]["communication_scope"] = scope.clone()
                }
                None => {
                    record["row"]["encrypted_reference"]
                        .as_object_mut()
                        .unwrap()
                        .remove("communication_scope");
                }
            }
        }
        writeln!(encoder, "{record}").unwrap();
    }
    encoder.finish().unwrap()
}

#[tokio::test]
async fn communication_backup_rekeys_cursor_and_attachment_after_fk_deferred_restore() {
    let (db, actor) = fixture().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Communication backup")
        .await
        .unwrap();
    let instance = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    let message = Uuid::now_v7();
    let attachment = Uuid::now_v7();
    let file_table = Uuid::now_v7();
    let file_record = Uuid::now_v7();
    let cursor_aad = format!("communication:{}:{instance}:cursor", workspace.id);
    let reference_aad = format!("communication:{}:{instance}:{attachment}", workspace.id);
    let cursor = json!({"cursor":"private-provider-cursor"});
    let reference = json!({"download_token":"private-download-credential"});
    sqlx::query("INSERT INTO notification_instances(id,workspace_id,type_code,display_name) VALUES($1,$2,'communication.fixture','Backup channel')").bind(instance).bind(workspace.id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO communication_bindings(instance_id,scope_id,actor_user_id,config_json,plugin_mcp_instance_id,encrypted_cursor) VALUES($1,$2,$3,'{}'::jsonb,'plugin-tools',$4)")
        .bind(instance).bind(workspace.id).bind(actor).bind(encrypt_secret_json_with_aad(&cursor,"source",cursor_aad.as_bytes()).unwrap()).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) VALUES($1,$2,$3,'alice','chat')").bind(conversation).bind(workspace.id).bind(instance).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms,content,processing_status,send_status) VALUES($1,$2,$3,$4,'alice','provider-message',42,'retained input','failed','none')").bind(message).bind(workspace.id).bind(instance).bind(conversation).execute(&db).await.unwrap();
    let mut encrypted_reference =
        encrypt_secret_json_with_aad(&reference, "source", reference_aad.as_bytes()).unwrap();
    encrypted_reference["communication_scope"] =
        json!({"workspace_id":workspace.id,"instance_id":instance});
    sqlx::query("INSERT INTO communication_attachments(id,scope_id,message_id,provider_attachment_id,file_name,media_type,encrypted_reference,status,file_table_id,file_record_id) VALUES($1,$6,$2,'provider-attachment','file.png','image/png',$3,'ready',$4,$5)")
        .bind(attachment).bind(message).bind(encrypted_reference).bind(file_table).bind(file_record).bind(workspace.id).execute(&db).await.unwrap();
    let repository = PgSelectiveBackupRepository::new(db.clone());
    let config = capture(&repository, select("notifications", true, false)).await;
    let full = capture(&repository, select("notifications", true, true)).await;
    let preview = repository
        .preflight(reader(config.clone()), "source", "target")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert!(preview
        .selected_tables
        .contains(&"communication_bindings".into()));
    assert!(!preview
        .selected_tables
        .contains(&"communication_messages".into()));
    for malformed in [
        rewrite_scope(&full, None),
        rewrite_scope(
            &full,
            Some(json!({"workspace_id":Uuid::now_v7(),"instance_id":instance})),
        ),
    ] {
        let preview = repository
            .preflight(reader(malformed), "source", "target")
            .await
            .unwrap();
        assert!(
            !preview.failures.is_empty(),
            "attachment envelope scope must authenticate its ciphertext"
        );
    }
    // Remove parent rows so attachments (alphabetically first in the archive) must be deferred.
    sqlx::query("DELETE FROM communication_attachments WHERE id=$1")
        .bind(attachment)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_messages WHERE id=$1")
        .bind(message)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_conversations WHERE id=$1")
        .bind(conversation)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM notification_instances WHERE id=$1")
        .bind(instance)
        .execute(&db)
        .await
        .unwrap();
    assert!(repository
        .restore(reader(full.clone()), "wrong-source", "target", true)
        .await
        .is_err());
    repository
        .restore(reader(full), "source", "target", true)
        .await
        .unwrap();
    let restored_cursor: Value = sqlx::query_scalar(
        "SELECT encrypted_cursor FROM communication_bindings WHERE instance_id=$1",
    )
    .bind(instance)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(&restored_cursor, "target", cursor_aad.as_bytes()).unwrap(),
        cursor
    );
    assert!(
        decrypt_secret_json_with_aad(&restored_cursor, "source", cursor_aad.as_bytes()).is_err()
    );
    let row = sqlx::query("SELECT encrypted_reference,file_table_id,file_record_id,message_id FROM communication_attachments WHERE id=$1").bind(attachment).fetch_one(&db).await.unwrap();
    let restored_reference: Value = row.get("encrypted_reference");
    assert_eq!(
        decrypt_secret_json_with_aad(&restored_reference, "target", reference_aad.as_bytes())
            .unwrap(),
        reference
    );
    assert!(
        decrypt_secret_json_with_aad(&restored_reference, "source", reference_aad.as_bytes())
            .is_err()
    );
    assert_eq!(row.get::<Uuid, _>("file_table_id"), file_table);
    assert_eq!(row.get::<Uuid, _>("file_record_id"), file_record);
    assert_eq!(row.get::<Uuid, _>("message_id"), message);
    let content: String =
        sqlx::query_scalar("SELECT content FROM communication_messages WHERE id=$1")
            .bind(message)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(content, "retained input");
}

#[test]
fn communication_rekey_authenticates_scope_and_keeps_runtime_aad_authoritative() {
    let workspace = Uuid::now_v7();
    let instance = Uuid::now_v7();
    let attachment = Uuid::now_v7();
    let scope = json!({"workspace_id":workspace,"instance_id":instance});
    let aad = format!("communication:{workspace}:{instance}:{attachment}");
    let reference = json!({"download_token":"not-public"});
    let mut ciphertext =
        encrypt_secret_json_with_aad(&reference, "source", aad.as_bytes()).unwrap();
    ciphertext["communication_scope"] = scope.clone();
    let original = json!({"id":attachment,"scope_id":workspace,"encrypted_reference":ciphertext});
    let mut unchanged = original.clone();
    validation::rekey(
        "communication_attachments",
        &mut unchanged,
        "source",
        "source",
    )
    .unwrap();
    assert_eq!(unchanged, original);
    let mut rekeyed = original.clone();
    validation::rekey(
        "communication_attachments",
        &mut rekeyed,
        "source",
        "target",
    )
    .unwrap();
    assert_eq!(rekeyed["encrypted_reference"]["communication_scope"], scope);
    assert_eq!(
        decrypt_secret_json_with_aad(&rekeyed["encrypted_reference"], "target", aad.as_bytes())
            .unwrap(),
        reference
    );
    assert!(!rekeyed.to_string().contains("not-public"));
    assert!(validation::rekey(
        "communication_attachments",
        &mut original.clone(),
        "wrong",
        "target"
    )
    .is_err());
    for field in ["workspace_id", "instance_id"] {
        let mut forged = original.clone();
        forged["encrypted_reference"]["communication_scope"][field] = json!(Uuid::now_v7());
        assert!(
            validation::rekey("communication_attachments", &mut forged, "source", "target")
                .is_err()
        );
    }
    let mut wrong_physical_scope = original.clone();
    wrong_physical_scope["scope_id"] = json!(Uuid::now_v7());
    assert!(validation::rekey(
        "communication_attachments",
        &mut wrong_physical_scope,
        "source",
        "target"
    )
    .is_err());
    let mut moved = original.clone();
    moved["id"] = json!(Uuid::now_v7());
    assert!(
        validation::rekey("communication_attachments", &mut moved, "source", "target").is_err()
    );
    let wrong_runtime_aad = format!("communication:{}:{instance}:{attachment}", Uuid::now_v7());
    assert!(decrypt_secret_json_with_aad(
        &rekeyed["encrypted_reference"],
        "target",
        wrong_runtime_aad.as_bytes()
    )
    .is_err());
    let cursor_aad = format!("communication:{workspace}:{instance}:cursor");
    let cursor = json!({"opaque":"private-cursor"});
    let mut binding = json!({"scope_id":workspace,"instance_id":instance,"encrypted_cursor":encrypt_secret_json_with_aad(&cursor,"source",cursor_aad.as_bytes()).unwrap()});
    validation::rekey("communication_bindings", &mut binding, "source", "target").unwrap();
    assert_eq!(
        decrypt_secret_json_with_aad(
            &binding["encrypted_cursor"],
            "target",
            cursor_aad.as_bytes()
        )
        .unwrap(),
        cursor
    );
    let mut empty_cursor = json!({"encrypted_cursor":null});
    validation::rekey(
        "communication_bindings",
        &mut empty_cursor,
        "source",
        "target",
    )
    .unwrap();
    assert_eq!(empty_cursor, json!({"encrypted_cursor":null}));
}
