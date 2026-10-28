use super::*;

pub(in super::super) async fn claim(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    lease: u32,
    key: &str,
) -> Result<Option<ClaimedCommunicationTurn>> {
    let mut tx = store.pool().begin().await?;
    let row = sqlx::query("SELECT b.* FROM communication_bindings b JOIN notification_instances n ON n.id=b.instance_id AND n.workspace_id=b.scope_id WHERE b.scope_id=$1 AND b.instance_id=$2 AND n.status='enabled' FOR SHARE OF n,b")
        .bind(w).bind(i).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let binding = binding(row)?;
    let row = sqlx::query("SELECT c.*,t.id AS candidate_turn_id FROM communication_conversations c JOIN communication_sessions s ON s.scope_id=c.scope_id AND s.instance_id=c.instance_id AND s.external_identity=c.external_identity AND s.conversation_id=c.id AND s.epoch=c.session_epoch AND s.active JOIN communication_reply_turns t ON t.scope_id=s.scope_id AND t.instance_id=s.instance_id AND t.external_identity=s.external_identity AND t.conversation_id=s.conversation_id AND t.session_id=s.id AND t.session_epoch=s.epoch WHERE c.scope_id=$1 AND c.instance_id=$2 AND (t.claim_id IS NULL OR t.claim_expires_at<=clock_timestamp()) AND ((t.state='open' AND t.processing_status IN ('pending','processing')) OR (t.state='sealed' AND t.send_status IN ('pending','sending'))) AND NOT EXISTS(SELECT 1 FROM communication_reply_turns p WHERE p.scope_id=t.scope_id AND p.instance_id=t.instance_id AND p.external_identity=t.external_identity AND p.conversation_id=t.conversation_id AND p.id<t.id AND p.state<>'invalidated' AND (p.processing_status IN ('pending','processing') OR p.send_status IN ('pending','sending'))) AND NOT EXISTS(SELECT 1 FROM communication_inputs p WHERE p.scope_id=t.scope_id AND p.instance_id=t.instance_id AND p.external_identity=t.external_identity AND p.conversation_id=t.conversation_id AND p.turn_id IS NULL AND p.admission_status='legacy' AND (p.processing_status='processing' OR p.send_status IN ('pending','sending'))) ORDER BY t.id LIMIT 1 FOR UPDATE OF c SKIP LOCKED")
        .bind(w).bind(i).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let turn_id: Uuid = row.try_get("candidate_turn_id")?;
    let conversation = conversation(row);
    let scope = CommunicationConversationScope {
        workspace_id: w,
        instance_id: i,
        external_identity: conversation.external_identity.clone(),
        conversation_id: conversation.id,
    };
    // The candidate SELECT may have observed an earlier snapshot before another claimant
    // released the chat lock. Recheck the lease after owning that lock, not just before it.
    let row = sqlx::query("SELECT * FROM communication_reply_turns WHERE id=$1 AND (claim_id IS NULL OR claim_expires_at<=clock_timestamp()) AND ((state='open' AND processing_status IN ('pending','processing')) OR (state='sealed' AND send_status IN ('pending','sending'))) FOR UPDATE")
        .bind(turn_id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    // A crashed sender is observation-only. Never turn an uncertain provider submission
    // back into an executable reservation just because its lease expired.
    if row.try_get::<String, _>("send_status")? == "sending" {
        sqlx::query("UPDATE communication_reply_turns SET send_status='unknown',failure_code='send_interrupted',claim_id=NULL,claim_expires_at=NULL WHERE id=$1")
            .bind(turn_id).execute(&mut *tx).await?;
        project_progress(&mut tx, turn_id).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let claim_id = Uuid::now_v7();
    let row = sqlx::query("UPDATE communication_reply_turns SET processing_status=CASE WHEN state='open' THEN 'processing' ELSE processing_status END,claim_id=$2,claim_expires_at=clock_timestamp()+make_interval(secs=>$3) WHERE id=$1 RETURNING *")
        .bind(turn_id).bind(claim_id).bind(f64::from(lease.clamp(10,3600))).fetch_one(&mut *tx).await?;
    project_progress(&mut tx, turn_id).await?;
    let session = session_by_id(&mut tx, &scope, row.try_get("session_id")?).await?;
    let claim = CommunicationTurnClaim {
        scope,
        session_id: session.id,
        session_epoch: session.epoch,
        turn_id,
        claim_id,
    };
    let final_reply = sqlx::query_scalar("SELECT content FROM communication_messages WHERE id=$1 AND scope_id=$2 AND instance_id=$3 AND external_identity=$4 AND conversation_id=$5 AND direction='outbound' AND type='text'")
        .bind(row.try_get::<Option<Uuid>, _>("outbound_message_id")?).bind(w).bind(i).bind(&claim.scope.external_identity).bind(claim.scope.conversation_id).fetch_optional(&mut *tx).await?.flatten();
    let pending = snapshot(&mut tx, &claim.scope, row, key).await?;
    tx.commit().await?;
    Ok(Some(ClaimedCommunicationTurn {
        binding,
        conversation,
        session,
        claim,
        pending,
        final_reply,
    }))
}

pub(in super::super) async fn renew(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    lease: u32,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    if claim_row(&mut tx, c, true).await?.is_none() {
        return Ok(false);
    }
    sqlx::query("UPDATE communication_reply_turns SET claim_expires_at=clock_timestamp()+make_interval(secs=>$2) WHERE id=$1")
        .bind(c.turn_id).bind(f64::from(lease.clamp(10,3600))).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

pub(in super::super) async fn inputs(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    key: &str,
) -> Result<Option<CommunicationTurnInputs>> {
    let mut tx = store.pool().begin().await?;
    let Some(row) = claim_row(&mut tx, c, true).await? else {
        return Ok(None);
    };
    let result = snapshot(&mut tx, &c.scope, row, key).await?;
    tx.commit().await?;
    Ok(Some(result))
}

pub(in super::super) async fn record_attachment(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    u: CommunicationAttachmentUpdate,
) -> Result<bool> {
    let ready = u.failure_code.is_none() && u.file_table_id.is_some() && u.file_record_id.is_some();
    if !ready
        && (u.failure_code.is_none() || u.file_table_id.is_some() || u.file_record_id.is_some())
    {
        return Err(Error::InvalidInput("communication_file_result").into());
    }
    let mut tx = store.pool().begin().await?;
    let Some(row) = claim_row(&mut tx, c, true).await? else {
        return Ok(false);
    };
    if row.try_get::<String, _>("state")? != "open" {
        return Ok(false);
    }
    let changed = sqlx::query("UPDATE communication_messages a SET status=$6,file_table_id=$7,file_record_id=$8,failure_code=$9 FROM communication_inputs m WHERE a.input_id=m.id AND a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id AND m.scope_id=$1 AND m.instance_id=$2 AND m.external_identity=$3 AND m.conversation_id=$4 AND m.turn_id=$5 AND a.id=$10 AND a.type='file' AND a.status<>'ready'")
        .bind(c.scope.workspace_id).bind(c.scope.instance_id).bind(&c.scope.external_identity).bind(c.scope.conversation_id).bind(c.turn_id).bind(if ready {"ready"} else {"failed"}).bind(u.file_table_id).bind(u.file_record_id).bind(u.failure_code).bind(u.attachment_id).execute(&mut *tx).await?.rows_affected()==1;
    tx.commit().await?;
    Ok(changed)
}

pub(in super::super) async fn link_run(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    run_id: Uuid,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    if claim_row(&mut tx, c, true).await?.is_none() {
        return Ok(false);
    }
    let changed = sqlx::query("UPDATE communication_reply_turns t SET run_id=$2 WHERE t.id=$1 AND t.state='open' AND t.processing_status='processing' AND t.run_id IS NULL AND NOT EXISTS(SELECT 1 FROM communication_inputs m JOIN communication_messages a ON a.input_id=m.id AND a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id WHERE m.turn_id=t.id AND m.scope_id=t.scope_id AND m.instance_id=t.instance_id AND m.external_identity=t.external_identity AND m.conversation_id=t.conversation_id AND a.type='file' AND a.status<>'ready')")
        .bind(c.turn_id).bind(run_id).execute(&mut *tx).await?.rows_affected()==1;
    if changed {
        project_progress(&mut tx, c.turn_id).await?;
    }
    tx.commit().await?;
    Ok(changed)
}

pub(in super::super) async fn checkpoint(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    checkpoint: CommunicationTurnCheckpoint,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    if claim_row(&mut tx, c, true).await?.is_none() {
        return Ok(false);
    }
    let changed = sqlx::query("UPDATE communication_reply_turns t SET consumed_watermark=$3,checkpoint_revision=checkpoint_revision+1,checkpoint_json=$4 WHERE id=$1 AND state='open' AND processing_status='processing' AND run_id IS NOT NULL AND checkpoint_revision=$2 AND $3>=consumed_watermark AND $3<=received_watermark AND NOT EXISTS(SELECT 1 FROM communication_inputs m JOIN communication_messages a ON a.input_id=m.id AND a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id WHERE m.turn_id=t.id AND m.scope_id=t.scope_id AND m.instance_id=t.instance_id AND m.external_identity=t.external_identity AND m.conversation_id=t.conversation_id AND m.turn_sequence<=$3 AND a.type='file' AND a.status<>'ready')")
        .bind(c.turn_id).bind(checkpoint.expected_checkpoint_revision).bind(checkpoint.consumed_watermark).bind(checkpoint.checkpoint).execute(&mut *tx).await?.rows_affected()==1;
    tx.commit().await?;
    Ok(changed)
}

pub(in super::super) async fn seal(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    candidate: CommunicationTurnFinalCandidate,
    key: &str,
) -> Result<CommunicationTurnSeal> {
    if candidate.final_reply.trim().is_empty() {
        return Err(Error::InvalidInput("communication_final_reply").into());
    }
    let mut tx = store.pool().begin().await?;
    let Some(row) = claim_row(&mut tx, c, true).await? else {
        return Ok(CommunicationTurnSeal::Fenced);
    };
    let current = turn(&row)?;
    if current.state != CommunicationTurnState::Open
        || current.processing_status != CommunicationProcessingStatus::Processing
        || current.run_id.is_none()
    {
        return Ok(CommunicationTurnSeal::Fenced);
    }
    if !candidate.matches_checkpoint(&current) || !pending_ids(&mut tx, &c.scope).await?.is_empty()
    {
        let pending = snapshot(&mut tx, &c.scope, row, key).await?;
        tx.commit().await?;
        return Ok(CommunicationTurnSeal::Pending(pending));
    }
    let outbound = Uuid::now_v7();
    sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,$4,$5,$6,'text','outbound',(extract(epoch FROM clock_timestamp())*1000)::bigint,$7)")
        .bind(outbound).bind(c.scope.workspace_id).bind(c.scope.instance_id).bind(&c.scope.external_identity).bind(c.scope.conversation_id).bind(current.anchor_input_id).bind(&candidate.final_reply).execute(&mut *tx).await?;
    let row = sqlx::query("UPDATE communication_reply_turns SET state='sealed',processing_status='succeeded',send_status='sending',failure_code=NULL,outbound_message_id=$2 WHERE id=$1 RETURNING *")
        .bind(c.turn_id).bind(outbound).fetch_one(&mut *tx).await?;
    project_progress(&mut tx, c.turn_id).await?;
    tx.commit().await?;
    Ok(CommunicationTurnSeal::Reserved {
        turn: turn(&row)?,
        final_reply: candidate.final_reply,
    })
}

pub(in super::super) async fn reserve_send(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
) -> Result<Option<String>> {
    let mut tx = store.pool().begin().await?;
    let Some(row) = claim_row(&mut tx, c, true).await? else {
        return Ok(None);
    };
    let current = turn(&row)?;
    if current.state != CommunicationTurnState::Sealed
        || current.send_status != CommunicationSendStatus::Pending
    {
        return Ok(None);
    }
    let reply = sqlx::query_scalar("SELECT content FROM communication_messages WHERE id=$1 AND scope_id=$2 AND instance_id=$3 AND external_identity=$4 AND conversation_id=$5 AND input_id=$6 AND type='text' AND direction='outbound'")
        .bind(current.outbound_message_id).bind(c.scope.workspace_id).bind(c.scope.instance_id).bind(&c.scope.external_identity).bind(c.scope.conversation_id).bind(current.anchor_input_id).fetch_one(&mut *tx).await?;
    sqlx::query("UPDATE communication_reply_turns SET send_status='sending' WHERE id=$1")
        .bind(c.turn_id)
        .execute(&mut *tx)
        .await?;
    project_progress(&mut tx, c.turn_id).await?;
    tx.commit().await?;
    Ok(Some(reply))
}

pub(in super::super) async fn progress(
    store: &PgControlPlaneStore,
    c: &CommunicationTurnClaim,
    p: CommunicationTurnProgress,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    let run_failure = matches!(&p, CommunicationTurnProgress::RunFailed { .. });
    // Terminal provider facts remain recordable after disable/rotation, but these calls
    // never grant another send. All execution/continuation writes require the active epoch.
    let Some(row) = claim_row(&mut tx, c, run_failure).await? else {
        return Ok(false);
    };
    let current = turn(&row)?;
    let submitted = current.send_status == CommunicationSendStatus::Sending
        || (current.state == CommunicationTurnState::Invalidated
            && current.send_status == CommunicationSendStatus::Unknown
            && current.failure_code.as_deref() == Some("session_rotated"));
    let (state, processing, send, failure) = match p {
        CommunicationTurnProgress::RunFailed { failure_code }
            if current.state == CommunicationTurnState::Open
                && current.processing_status == CommunicationProcessingStatus::Processing =>
        {
            ("failed", "failed", "none", Some(failure_code))
        }
        CommunicationTurnProgress::SendSucceeded if submitted => {
            (row.get("state"), "succeeded", "succeeded", None)
        }
        CommunicationTurnProgress::SendFailed { failure_code } if submitted => {
            (row.get("state"), "succeeded", "failed", Some(failure_code))
        }
        CommunicationTurnProgress::SendUnknown { failure_code } if submitted => {
            (row.get("state"), "succeeded", "unknown", Some(failure_code))
        }
        _ => return Ok(false),
    };
    sqlx::query("UPDATE communication_reply_turns SET state=$2,processing_status=$3,send_status=$4,failure_code=$5,claim_id=NULL,claim_expires_at=NULL WHERE id=$1")
        .bind(c.turn_id).bind(state).bind(processing).bind(send).bind(failure).execute(&mut *tx).await?;
    project_progress(&mut tx, c.turn_id).await?;
    tx.commit().await?;
    Ok(true)
}

pub(in super::super) async fn retry(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    input_id: Uuid,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    let row = sqlx::query("SELECT conversation_id,external_identity,turn_id FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND id=$3 AND turn_id IS NOT NULL")
        .bind(w).bind(i).bind(input_id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(false) };
    let scope = CommunicationConversationScope {
        workspace_id: w,
        instance_id: i,
        external_identity: row.try_get("external_identity")?,
        conversation_id: row.try_get("conversation_id")?,
    };
    let id: Uuid = row.try_get("turn_id")?;
    if lock_chat(&mut tx, &scope, true).await?.is_none() {
        return Ok(false);
    }
    let sql = format!("SELECT m.id FROM communication_inputs m WHERE m.id=$1 AND m.scope_id=$2 AND m.instance_id=$3 AND ({RETRY_ELIGIBILITY}) FOR UPDATE");
    if sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(input_id)
        .bind(w)
        .bind(i)
        .fetch_optional(&mut *tx)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    sqlx::query("UPDATE communication_reply_turns SET state=CASE WHEN state='failed' THEN 'open' ELSE state END,processing_status=CASE WHEN processing_status='failed' THEN 'pending' ELSE processing_status END,send_status=CASE WHEN send_status='failed' THEN 'pending' ELSE send_status END,run_id=CASE WHEN processing_status='failed' THEN NULL ELSE run_id END,consumed_watermark=CASE WHEN processing_status='failed' THEN 0 ELSE consumed_watermark END,checkpoint_revision=CASE WHEN processing_status='failed' THEN 0 ELSE checkpoint_revision END,checkpoint_json=CASE WHEN processing_status='failed' THEN NULL ELSE checkpoint_json END,failure_code=NULL,claim_id=NULL,claim_expires_at=NULL WHERE id=$1")
        .bind(id).execute(&mut *tx).await?;
    project_progress(&mut tx, id).await?;
    sqlx::query("UPDATE communication_messages a SET status='pending',failure_code=NULL FROM communication_inputs m WHERE m.turn_id=$1 AND a.input_id=m.id AND a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id AND a.type='file' AND a.status='failed'")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}
