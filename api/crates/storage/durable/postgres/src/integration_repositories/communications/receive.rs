use super::*;

pub(super) async fn commit(
    store: &PgControlPlaneStore,
    input: CommitCommunicationReceiveInput,
) -> Result<Option<i64>> {
    let auth = input.authorization;
    auth.authorization
        .validate()
        .map_err(|_| Error::Conflict("notification_authorization_state"))?;
    let w = auth.workspace_id;
    let i = auth.instance_id;
    let encrypted = encrypt_secret_json_with_aad(
        &serde_json::to_value(auth.managed_secrets)?,
        &auth.master_key,
        &crate::notification_repository::managed_aad(w, i),
    )?;
    let cursor = input
        .candidate_cursor
        .map(|v| encrypt_secret_json_with_aad(&v, &auth.master_key, &private_aad(w, i, "cursor")))
        .transpose()?;
    let mut tx = store.pool().begin().await?;
    // This lock is shared with disable/delete/config/authorization writes. A stale receive
    // cannot persist messages, secrets or cursor after authorization has changed.
    let revision:Option<i64>=sqlx::query_scalar("UPDATE notification_instances SET authorization_json=$4,encrypted_managed_secret_json=$5,authorization_revision=authorization_revision+1,updated_at=now() WHERE workspace_id=$1 AND id=$2 AND authorization_revision=$3 AND status='enabled' AND EXISTS(SELECT 1 FROM communication_bindings b WHERE b.instance_id=notification_instances.id AND b.scope_id=$1) RETURNING authorization_revision")
        .bind(w).bind(i).bind(auth.expected_revision).bind(serde_json::to_value(auth.authorization)?).bind(encrypted).fetch_optional(&mut *tx).await?;
    if revision.is_none() {
        return Ok(None);
    }
    for m in input.messages {
        if m.provider_message_id.trim().is_empty()
            || m.conversation_id.trim().is_empty()
            || m.sender_id.trim().is_empty()
            || m.received_at_ms < 0
        {
            return Err(Error::InvalidInput("communication_received_message").into());
        }
        let conversation_id:Uuid=sqlx::query_scalar("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key,sender_display_name) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(scope_id,instance_id,external_identity,external_conversation_key) DO UPDATE SET sender_display_name=excluded.sender_display_name RETURNING id")
            .bind(Uuid::now_v7()).bind(w).bind(i).bind(&m.sender_id).bind(m.conversation_id).bind(m.sender_display_name).fetch_one(&mut *tx).await?;
        let message_id:Option<Uuid>=sqlx::query_scalar("INSERT INTO communication_messages(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms,content) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(scope_id,instance_id,external_identity,external_message_key) DO NOTHING RETURNING id")
            .bind(Uuid::now_v7()).bind(w).bind(i).bind(conversation_id).bind(m.sender_id).bind(m.provider_message_id).bind(m.received_at_ms).bind(m.text).fetch_optional(&mut *tx).await?;
        if let Some(message_id) = message_id {
            for a in m.attachments {
                if a.provider_attachment_id.trim().is_empty() || a.file_name.trim().is_empty() {
                    return Err(Error::InvalidInput("communication_attachment").into());
                }
                let id = Uuid::now_v7();
                let mut reference = encrypt_secret_json_with_aad(
                    &a.reference,
                    &auth.master_key,
                    &private_aad(w, i, &id.to_string()),
                )?;
                // Public scope metadata makes this private envelope independently rekeyable
                // in backups. Runtime decryption still derives AAD from trusted message rows.
                reference["communication_scope"] =
                    serde_json::json!({"workspace_id":w,"instance_id":i});
                sqlx::query("INSERT INTO communication_attachments(id,scope_id,message_id,provider_attachment_id,file_name,media_type,size_bytes,encrypted_reference) VALUES($1,$8,$2,$3,$4,$5,$6,$7)")
                    .bind(id).bind(message_id).bind(a.provider_attachment_id).bind(a.file_name).bind(a.media_type).bind(a.size_bytes).bind(reference).bind(w).execute(&mut *tx).await?;
            }
        }
    }
    sqlx::query("UPDATE communication_bindings SET encrypted_cursor=COALESCE($3,encrypted_cursor) WHERE scope_id=$1 AND instance_id=$2").bind(w).bind(i).bind(cursor).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(revision)
}
