use super::*;

pub(in super::super) async fn active(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
) -> Result<Option<CommunicationSession>> {
    let row = sqlx::query("SELECT a.*,c.selected_model FROM communication_sessions a JOIN communication_conversations c ON c.id=a.conversation_id AND c.scope_id=a.scope_id AND c.instance_id=a.instance_id AND c.external_identity=a.external_identity AND c.session_epoch=a.epoch WHERE a.scope_id=$1 AND a.instance_id=$2 AND a.external_identity=$3 AND a.conversation_id=$4 AND a.active")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_optional(store.pool()).await?;
    row.as_ref().map(session).transpose()
}

pub(in super::super) async fn select_model(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
    model: Option<&str>,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    if lock_chat(&mut tx, s, true).await?.is_none() {
        return Ok(false);
    }
    sqlx::query("UPDATE communication_conversations SET selected_model=$5 WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND id=$4")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(model).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

pub(in super::super) async fn rotate(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
    receipt_key: &str,
) -> Result<Option<CommunicationSessionRotation>> {
    if receipt_key.trim().is_empty() {
        return Err(Error::InvalidInput("communication_rotation_receipt").into());
    }
    let mut tx = store.pool().begin().await?;
    if lock_chat(&mut tx, s, true).await?.is_none() {
        return Ok(None);
    }
    if let Some(value) = sqlx::query_scalar::<_, Value>("SELECT result_json FROM communication_session_rotations WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND receipt_key=$5")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(receipt_key).fetch_optional(&mut *tx).await? {
        let mut result: CommunicationSessionRotation = serde_json::from_value(value)?;
        result.replayed = true;
        tx.commit().await?;
        return Ok(Some(result));
    }
    let old: Option<Uuid> = sqlx::query_scalar("UPDATE communication_sessions SET active=false WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND active RETURNING id")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_optional(&mut *tx).await?;
    let mut cancel_run_ids: Vec<Uuid> = sqlx::query_scalar("SELECT run_id FROM communication_reply_turns WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND session_id=$5 AND run_id IS NOT NULL AND (processing_status IN ('pending','processing') OR send_status IN ('pending','sending')) ORDER BY id")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(old).fetch_all(&mut *tx).await?;
    let invalidated: Vec<Uuid> = sqlx::query_scalar("UPDATE communication_reply_turns SET state='invalidated',processing_status=CASE WHEN processing_status IN ('pending','processing') THEN 'failed' ELSE processing_status END,send_status=CASE WHEN send_status='sending' THEN 'unknown' WHEN send_status='pending' THEN 'failed' ELSE send_status END,failure_code='session_rotated',claim_id=CASE WHEN send_status='sending' THEN claim_id ELSE NULL END,claim_expires_at=CASE WHEN send_status='sending' THEN claim_expires_at ELSE NULL END WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND session_id=$5 AND state<>'invalidated' AND (processing_status IN ('pending','processing') OR send_status IN ('pending','sending','failed') OR state='failed') RETURNING id")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(old).fetch_all(&mut *tx).await?;
    for id in invalidated {
        project_progress(&mut tx, id).await?;
    }
    // Fence old workers too, including runs reserved before this migration. Submitted
    // sends become uncertain and remain non-retryable; no history or run fact is erased.
    let legacy: Vec<Uuid> = sqlx::query_scalar("SELECT run_id FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND turn_id IS NULL AND admission_status='legacy' AND run_id IS NOT NULL AND (processing_status IN ('pending','processing') OR send_status IN ('pending','sending'))")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_all(&mut *tx).await?;
    cancel_run_ids.extend(legacy);
    cancel_run_ids.sort_unstable();
    cancel_run_ids.dedup();
    sqlx::query("UPDATE communication_inputs SET processing_status=CASE WHEN processing_status IN ('pending','processing') THEN 'failed' ELSE processing_status END,send_status=CASE WHEN send_status='sending' THEN 'unknown' WHEN send_status='pending' THEN 'failed' ELSE send_status END,failure_code='session_rotated',admission_status='excluded',claim_id=NULL,claim_expires_at=NULL WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND turn_id IS NULL AND admission_status='legacy' AND (processing_status IN ('processing','failed') OR (processing_status='pending' AND run_id IS NOT NULL) OR send_status IN ('pending','sending','failed'))")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).execute(&mut *tx).await?;
    let result = CommunicationSessionRotation {
        session: ensure_session(&mut tx, s).await?,
        invalidated_session_id: old,
        cancel_run_ids,
        replayed: false,
    };
    sqlx::query("INSERT INTO communication_session_rotations(scope_id,instance_id,external_identity,conversation_id,receipt_key,result_json) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(receipt_key).bind(serde_json::to_value(&result)?).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(result))
}
