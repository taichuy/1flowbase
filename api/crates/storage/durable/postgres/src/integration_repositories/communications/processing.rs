use super::*;

pub(super) async fn claim(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    l: u32,
    k: &str,
    legacy_only: bool,
) -> Result<Option<ClaimedCommunicationInput>> {
    let mut tx = store.pool().begin().await?;
    let row=sqlx::query("SELECT b.* FROM communication_bindings b JOIN notification_instances n ON n.id=b.instance_id AND n.workspace_id=b.scope_id WHERE b.scope_id=$1 AND b.instance_id=$2 AND n.status='enabled' FOR SHARE OF n,b")
        .bind(w).bind(i).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let binding = binding(row)?;
    // External sends may have happened before a crash: never repeat an uncertain send.
    sqlx::query("UPDATE communication_inputs SET send_status='unknown',failure_code='send_interrupted',claim_id=NULL,claim_expires_at=NULL WHERE scope_id=$1 AND instance_id=$2 AND turn_id IS NULL AND send_status='sending' AND claim_expires_at<=now()")
        .bind(w).bind(i).execute(&mut *tx).await?;
    // Reclaim the observation lease, retaining any reserved run ID. The execution owner
    // inspects that run and must never invoke it a second time after a restart.
    let sql = format!(
        "SELECT m.*, (SELECT content FROM communication_messages t WHERE t.input_id=m.id AND t.type='text' AND t.direction='inbound') AS content,(SELECT content FROM communication_messages t WHERE t.input_id=m.id AND t.type='text' AND t.direction='outbound') AS final_reply,({RETRY_ELIGIBILITY}) AS can_retry FROM communication_inputs m JOIN communication_conversations c ON c.id=m.conversation_id AND c.scope_id=m.scope_id AND c.instance_id=m.instance_id AND c.external_identity=m.external_identity WHERE m.scope_id=$1 AND m.instance_id=$2 AND m.turn_id IS NULL AND m.admission_status<>'excluded' AND (NOT $3 OR m.admission_status='legacy') AND ((m.processing_status='pending') OR (m.processing_status='processing' AND m.claim_expires_at<=now()) OR (m.processing_status='succeeded' AND m.send_status='pending' AND (m.claim_id IS NULL OR m.claim_expires_at<=now()))) AND NOT EXISTS(SELECT 1 FROM communication_inputs p WHERE p.conversation_id=m.conversation_id AND p.id<m.id AND p.admission_status<>'excluded' AND (p.processing_status IN ('pending','processing') OR p.send_status IN ('pending','sending'))) ORDER BY m.id LIMIT 1 FOR UPDATE OF c,m SKIP LOCKED"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(legacy_only)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let mut message = input(row)?;
    let claim_id = Uuid::now_v7();
    sqlx::query("UPDATE communication_inputs SET admission_status='legacy',processing_status=CASE WHEN processing_status='succeeded' THEN processing_status ELSE 'processing' END,claim_id=$2,claim_expires_at=now()+make_interval(secs=>$3) WHERE id=$1")
        .bind(message.id).bind(claim_id).bind(f64::from(l.clamp(10,3600))).execute(&mut *tx).await?;
    if message.processing_status != CommunicationProcessingStatus::Succeeded {
        message.processing_status = CommunicationProcessingStatus::Processing;
    }
    let conversation = conversation(
        sqlx::query("SELECT * FROM communication_conversations WHERE id=$1")
            .bind(message.conversation_id)
            .fetch_one(&mut *tx)
            .await?,
    );
    let rows = sqlx::query(
        "SELECT * FROM communication_messages WHERE input_id=$1 AND scope_id=$2 AND instance_id=$3 AND type='file' ORDER BY id",
    )
    .bind(message.id)
    .bind(w)
    .bind(i)
    .fetch_all(&mut *tx)
    .await?;
    let attachments = rows
        .iter()
        .map(|row| -> Result<_> {
            let record = attachment(row)?;
            let reference = decrypt_secret_json_with_aad(
                &row.get::<Value, _>("encrypted_reference"),
                k,
                &private_aad(w, i, &record.id.to_string()),
            )?;
            Ok(CommunicationPendingAttachment { record, reference })
        })
        .collect::<Result<Vec<_>>>()?;
    tx.commit().await?;
    Ok(Some(ClaimedCommunicationInput {
        binding,
        conversation,
        message,
        claim_id,
        attachments,
    }))
}
pub(super) async fn renew(
    store: &PgControlPlaneStore,
    c: &CommunicationClaim,
    l: u32,
) -> Result<bool> {
    Ok(sqlx::query("UPDATE communication_inputs m SET claim_expires_at=now()+make_interval(secs=>$5) WHERE scope_id=$1 AND instance_id=$2 AND id=$3 AND turn_id IS NULL AND admission_status<>'excluded' AND claim_id=$4 AND claim_expires_at>now() AND EXISTS(SELECT 1 FROM notification_instances n WHERE n.id=m.instance_id AND n.workspace_id=m.scope_id AND n.status='enabled')")
        .bind(c.workspace_id).bind(c.instance_id).bind(c.message_id).bind(c.claim_id).bind(f64::from(l.clamp(10,3600))).execute(store.pool()).await?.rows_affected()==1)
}
pub(super) async fn record_attachment(
    store: &PgControlPlaneStore,
    c: &CommunicationClaim,
    u: CommunicationAttachmentUpdate,
) -> Result<bool> {
    let ready = u.failure_code.is_none() && u.file_table_id.is_some() && u.file_record_id.is_some();
    if !ready
        && (u.failure_code.is_none() || u.file_table_id.is_some() || u.file_record_id.is_some())
    {
        return Err(Error::InvalidInput("communication_file_result").into());
    }
    Ok(sqlx::query("UPDATE communication_messages a SET status=$6,file_table_id=$7,file_record_id=$8,failure_code=$9 FROM communication_inputs m WHERE a.input_id=m.id AND a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id AND m.scope_id=$1 AND m.instance_id=$2 AND m.id=$3 AND m.turn_id IS NULL AND m.admission_status<>'excluded' AND m.claim_id=$4 AND m.claim_expires_at>now() AND a.id=$5 AND a.status<>'ready'")
        .bind(c.workspace_id).bind(c.instance_id).bind(c.message_id).bind(c.claim_id).bind(u.attachment_id).bind(if ready{"ready"}else{"failed"}).bind(u.file_table_id).bind(u.file_record_id).bind(u.failure_code).execute(store.pool()).await?.rows_affected()==1)
}
pub(super) async fn progress(
    store: &PgControlPlaneStore,
    c: &CommunicationClaim,
    p: CommunicationProgress,
) -> Result<bool> {
    let starts_external_work = matches!(
        &p,
        CommunicationProgress::RunStarted { .. } | CommunicationProgress::SendStarted
    );
    let (set, condition, run, reply, failure) = match p {
        CommunicationProgress::RunStarted { run_id } => (
            "processing_status='processing'",
            "processing_status='processing' AND run_id IS NULL AND NOT EXISTS(SELECT 1 FROM communication_messages a WHERE a.input_id=m.id AND a.status<>'ready')",
            Some(run_id),
            None,
            None,
        ),
        CommunicationProgress::RunSucceeded { final_reply } => {
            if final_reply.trim().is_empty() {
                return Err(Error::InvalidInput("communication_final_reply").into());
            }
            (
                "processing_status='succeeded',send_status='pending'",
                "processing_status='processing' AND run_id IS NOT NULL",
                None,
                Some(final_reply),
                None,
            )
        }
        CommunicationProgress::RunFailed { failure_code } => (
            "processing_status='failed',claim_id=NULL,claim_expires_at=NULL",
            "processing_status='processing'",
            None,
            None,
            Some(failure_code),
        ),
        CommunicationProgress::SendStarted => (
            "send_status='sending'",
            "processing_status='succeeded' AND send_status='pending' AND EXISTS(SELECT 1 FROM communication_messages t WHERE t.input_id=m.id AND t.direction='outbound' AND t.type='text')",
            None,
            None,
            None,
        ),
        CommunicationProgress::SendSucceeded => (
            "send_status='succeeded',claim_id=NULL,claim_expires_at=NULL",
            "send_status='sending'",
            None,
            None,
            None,
        ),
        CommunicationProgress::SendFailed { failure_code } => (
            "send_status='failed',claim_id=NULL,claim_expires_at=NULL",
            "send_status IN ('pending','sending')",
            None,
            None,
            Some(failure_code),
        ),
        CommunicationProgress::SendUnknown { failure_code } => (
            "send_status='unknown',claim_id=NULL,claim_expires_at=NULL",
            "send_status='sending'",
            None,
            None,
            Some(failure_code),
        ),
    };
    // Terminal facts remain recordable if the channel is disabled while provider I/O is in
    // flight. Only starting another run/send requires the channel to still be enabled.
    let enabled = if starts_external_work {
        "AND EXISTS(SELECT 1 FROM notification_instances n WHERE n.id=m.instance_id AND n.workspace_id=m.scope_id AND n.status='enabled')"
    } else {
        ""
    };
    let sql = format!(
        "UPDATE communication_inputs m SET {set},run_id=COALESCE($5,run_id),failure_code=$6 WHERE scope_id=$1 AND instance_id=$2 AND id=$3 AND turn_id IS NULL AND admission_status<>'excluded' AND claim_id=$4 AND claim_expires_at>now() AND ({condition}) {enabled}"
    );
    let mut tx = store.pool().begin().await?;
    let changed = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(c.workspace_id)
        .bind(c.instance_id)
        .bind(c.message_id)
        .bind(c.claim_id)
        .bind(run)
        .bind(failure)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        == 1;
    if changed {
        if let Some(reply) = reply {
            sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) SELECT $2,scope_id,instance_id,external_identity,conversation_id,id,'text','outbound',(extract(epoch FROM clock_timestamp())*1000)::bigint,$3 FROM communication_inputs WHERE id=$1 ON CONFLICT(input_id,direction) WHERE type='text' DO UPDATE SET content=excluded.content")
                .bind(c.message_id).bind(Uuid::now_v7()).bind(reply).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(changed)
}
pub(super) async fn retry(store: &PgControlPlaneStore, w: Uuid, i: Uuid, m: Uuid) -> Result<bool> {
    let grouped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND id=$3 AND turn_id IS NOT NULL)").bind(w).bind(i).bind(m).fetch_one(store.pool()).await?;
    if grouped {
        return sessions::processing::retry(store, w, i, m).await;
    }
    let mut tx = store.pool().begin().await?;
    let sql = format!(
        "UPDATE communication_inputs m SET processing_status=CASE WHEN processing_status='failed' THEN 'pending' ELSE processing_status END,send_status=CASE WHEN send_status='failed' THEN 'pending' ELSE send_status END,run_id=CASE WHEN processing_status='failed' THEN NULL ELSE run_id END,failure_code=NULL,claim_id=NULL,claim_expires_at=NULL WHERE scope_id=$1 AND instance_id=$2 AND id=$3 AND ({RETRY_ELIGIBILITY}) RETURNING id"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(m)
        .fetch_optional(&mut *tx)
        .await?;
    if row.is_some() {
        sqlx::query("UPDATE communication_messages SET status='pending',failure_code=NULL WHERE input_id=$1 AND type='file' AND status='failed'").bind(m).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(row.is_some())
}
