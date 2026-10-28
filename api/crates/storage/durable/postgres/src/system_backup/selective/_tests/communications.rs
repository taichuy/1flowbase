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
        if record["table"] == "communication_messages"
            && !record["row"]["encrypted_reference"].is_null()
        {
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
    sqlx::query("INSERT INTO communication_inputs(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms,processing_status,send_status) VALUES($1,$2,$3,$4,'alice','provider-message',42,'failed','none')").bind(message).bind(workspace.id).bind(instance).bind(conversation).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,conversation_id,external_identity,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,$4,'alice',$1,'text','inbound',42,'retained input')").bind(message).bind(workspace.id).bind(instance).bind(conversation).execute(&db).await.unwrap();
    let mut encrypted_reference =
        encrypt_secret_json_with_aad(&reference, "source", reference_aad.as_bytes()).unwrap();
    encrypted_reference["communication_scope"] =
        json!({"workspace_id":workspace.id,"instance_id":instance});
    sqlx::query("INSERT INTO communication_messages(id,scope_id,input_id,provider_attachment_id,file_name,media_type,encrypted_reference,status,file_table_id,file_record_id,instance_id,conversation_id,external_identity,type,direction,occurred_at_ms) VALUES($1,$6,$2,'provider-attachment','file.png','image/png',$3,'ready',$4,$5,$7,$8,'alice','file','inbound',42)")
        .bind(attachment).bind(message).bind(encrypted_reference).bind(file_table).bind(file_record).bind(workspace.id).bind(instance).bind(conversation).execute(&db).await.unwrap();
    let session = Uuid::now_v7();
    let turn = Uuid::now_v7();
    let run = Uuid::now_v7();
    sqlx::query("UPDATE communication_conversations SET session_epoch=1,selected_model='chat-model' WHERE id=$1").bind(conversation).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO communication_sessions(id,scope_id,instance_id,external_identity,conversation_id,epoch) VALUES($1,$2,$3,'alice',$4,1)").bind(session).bind(workspace.id).bind(instance).bind(conversation).execute(&db).await.unwrap();
    let checkpoint =
        json!({"completed_tool":{"id":"kept-side-effect"},"transcript":["kept result"]});
    sqlx::query("INSERT INTO communication_reply_turns(id,scope_id,instance_id,external_identity,conversation_id,session_id,session_epoch,anchor_input_id,state,run_id,received_watermark,consumed_watermark,checkpoint_revision,checkpoint_json,processing_status,send_status) VALUES($1,$2,$3,'alice',$4,$5,1,$6,'sealed',$7,1,1,1,$8,'succeeded','unknown')")
        .bind(turn).bind(workspace.id).bind(instance).bind(conversation).bind(session).bind(message).bind(run).bind(&checkpoint).execute(&db).await.unwrap();
    sqlx::query("UPDATE communication_inputs SET admission_status='admitted',turn_id=$2,turn_sequence=1,run_id=$3,processing_status='succeeded',send_status='unknown' WHERE id=$1").bind(message).bind(turn).bind(run).execute(&db).await.unwrap();
    let receipt = json!({"session":{"id":session,"conversation_id":conversation,"epoch":1,"active":true,"selected_model":"chat-model"},"invalidated_session_id":null,"cancel_run_ids":[],"replayed":false});
    sqlx::query("INSERT INTO communication_session_rotations(scope_id,instance_id,external_identity,conversation_id,receipt_key,result_json) VALUES($1,$2,'alice',$3,'restored-command',$4)").bind(workspace.id).bind(instance).bind(conversation).bind(&receipt).execute(&db).await.unwrap();
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
    for table in [
        "communication_sessions",
        "communication_reply_turns",
        "communication_session_rotations",
    ] {
        assert!(!preview.selected_tables.contains(&table.into()));
    }
    let data_preview = repository
        .preflight(reader(full.clone()), "source", "target")
        .await
        .unwrap();
    assert!(
        data_preview.failures.is_empty(),
        "{:?}",
        data_preview.failures
    );
    for table in [
        "communication_sessions",
        "communication_reply_turns",
        "communication_session_rotations",
    ] {
        assert!(
            data_preview.selected_tables.contains(&table.into()),
            "missing durable table {table}"
        );
    }
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
    sqlx::query("DELETE FROM communication_messages WHERE id=$1")
        .bind(attachment)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_messages WHERE id=$1")
        .bind(message)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_reply_turns WHERE id=$1")
        .bind(turn)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_sessions WHERE id=$1")
        .bind(session)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_session_rotations WHERE conversation_id=$1")
        .bind(conversation)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM communication_inputs WHERE id=$1")
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
    let row = sqlx::query("SELECT encrypted_reference,file_table_id,file_record_id,input_id FROM communication_messages WHERE id=$1").bind(attachment).fetch_one(&db).await.unwrap();
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
    assert_eq!(row.get::<Uuid, _>("input_id"), message);
    let content: String =
        sqlx::query_scalar("SELECT content FROM communication_messages WHERE id=$1")
            .bind(message)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(content, "retained input");
    let restored: (Uuid, Uuid, i64, i64, Value, String) = sqlx::query_as("SELECT scope_id,session_id,received_watermark,consumed_watermark,checkpoint_json,send_status FROM communication_reply_turns WHERE id=$1").bind(turn).fetch_one(&db).await.unwrap();
    assert_eq!(
        restored,
        (workspace.id, session, 1, 1, checkpoint, "unknown".into())
    );
    let membership: (Uuid, Uuid, i64, Uuid) = sqlx::query_as(
        "SELECT scope_id,turn_id,turn_sequence,run_id FROM communication_inputs WHERE id=$1",
    )
    .bind(message)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(membership, (workspace.id, turn, 1, run));
    let model: String = sqlx::query_scalar(
        "SELECT selected_model FROM communication_conversations WHERE scope_id=$1 AND id=$2",
    )
    .bind(workspace.id)
    .bind(conversation)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(model, "chat-model");
    let restored_receipt: Value = sqlx::query_scalar("SELECT result_json FROM communication_session_rotations WHERE scope_id=$1 AND instance_id=$2 AND external_identity='alice' AND conversation_id=$3 AND receipt_key='restored-command'").bind(workspace.id).bind(instance).bind(conversation).fetch_one(&db).await.unwrap();
    assert_eq!(restored_receipt, receipt);
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
    validation::rekey("communication_messages", &mut unchanged, "source", "source").unwrap();
    assert_eq!(unchanged, original);
    let mut rekeyed = original.clone();
    validation::rekey("communication_messages", &mut rekeyed, "source", "target").unwrap();
    assert_eq!(rekeyed["encrypted_reference"]["communication_scope"], scope);
    assert_eq!(
        decrypt_secret_json_with_aad(&rekeyed["encrypted_reference"], "target", aad.as_bytes())
            .unwrap(),
        reference
    );
    assert!(!rekeyed.to_string().contains("not-public"));
    assert!(validation::rekey(
        "communication_messages",
        &mut original.clone(),
        "wrong",
        "target"
    )
    .is_err());
    for field in ["workspace_id", "instance_id"] {
        let mut forged = original.clone();
        forged["encrypted_reference"]["communication_scope"][field] = json!(Uuid::now_v7());
        assert!(
            validation::rekey("communication_messages", &mut forged, "source", "target").is_err()
        );
    }
    let mut wrong_physical_scope = original.clone();
    wrong_physical_scope["scope_id"] = json!(Uuid::now_v7());
    assert!(validation::rekey(
        "communication_messages",
        &mut wrong_physical_scope,
        "source",
        "target"
    )
    .is_err());
    let mut moved = original.clone();
    moved["id"] = json!(Uuid::now_v7());
    assert!(validation::rekey("communication_messages", &mut moved, "source", "target").is_err());
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
