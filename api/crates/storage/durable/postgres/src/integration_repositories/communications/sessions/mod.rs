//! Deterministic scoped transactions. Classification/model policy belongs to the service/host.
use super::*;
use sqlx::PgConnection;

pub(super) mod admission;
pub(super) mod lifecycle;
pub(super) mod processing;

// Pre-upgrade pending inputs may be classified, but a linked run or uncertain send is
// always retained by the legacy observation path. Never silently redispatch it as a turn.
const ADMISSIBLE: &str = "m.admission_status IN ('pending','legacy') AND m.processing_status='pending' AND m.run_id IS NULL AND m.send_status='none' AND (m.claim_id IS NULL OR m.claim_expires_at<=clock_timestamp())";

async fn lock_chat(
    db: &mut PgConnection,
    s: &CommunicationConversationScope,
    enabled: bool,
) -> Result<Option<PgRow>> {
    if enabled {
        let exists = sqlx::query("SELECT n.id FROM notification_instances n JOIN communication_bindings b ON b.instance_id=n.id AND b.scope_id=n.workspace_id WHERE n.workspace_id=$1 AND n.id=$2 AND n.status='enabled' FOR SHARE OF n,b")
            .bind(s.workspace_id).bind(s.instance_id).fetch_optional(&mut *db).await?;
        if exists.is_none() {
            return Ok(None);
        }
    }
    // Receive, admission, rotation and seal all acquire this same row lock. Channel locks
    // precede it, matching commit_receive's authorization CAS and avoiding reverse order.
    Ok(sqlx::query("SELECT * FROM communication_conversations WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND id=$4 FOR UPDATE")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_optional(db).await?)
}

fn turn(row: &PgRow) -> Result<CommunicationReplyTurn> {
    Ok(CommunicationReplyTurn {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        session_id: row.try_get("session_id")?,
        session_epoch: row.try_get("session_epoch")?,
        anchor_input_id: row.try_get("anchor_input_id")?,
        state: serde_json::from_value(Value::String(row.try_get("state")?))?,
        run_id: row.try_get("run_id")?,
        selected_model: row.try_get("selected_model")?,
        received_watermark: row.try_get("received_watermark")?,
        consumed_watermark: row.try_get("consumed_watermark")?,
        checkpoint_revision: row.try_get("checkpoint_revision")?,
        processing_status: serde_json::from_value(Value::String(
            row.try_get("processing_status")?,
        ))?,
        send_status: serde_json::from_value(Value::String(row.try_get("send_status")?))?,
        failure_code: row.try_get("failure_code")?,
        outbound_message_id: row.try_get("outbound_message_id")?,
    })
}

fn session(row: &PgRow) -> Result<CommunicationSession> {
    Ok(CommunicationSession {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        epoch: row.try_get("epoch")?,
        active: row.try_get("active")?,
        selected_model: row.try_get("selected_model")?,
    })
}

async fn session_by_id(
    db: &mut PgConnection,
    s: &CommunicationConversationScope,
    id: Uuid,
) -> Result<CommunicationSession> {
    let row = sqlx::query("SELECT a.*,c.selected_model FROM communication_sessions a JOIN communication_conversations c ON c.id=a.conversation_id AND c.scope_id=a.scope_id AND c.instance_id=a.instance_id AND c.external_identity=a.external_identity WHERE a.scope_id=$1 AND a.instance_id=$2 AND a.external_identity=$3 AND a.conversation_id=$4 AND a.id=$5")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(id).fetch_one(db).await?;
    session(&row)
}

async fn ensure_session(
    db: &mut PgConnection,
    s: &CommunicationConversationScope,
) -> Result<CommunicationSession> {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM communication_sessions WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND active")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_optional(&mut *db).await? {
        return session_by_id(db, s, id).await;
    }
    let epoch: i64 = sqlx::query_scalar("UPDATE communication_conversations SET session_epoch=session_epoch+1 WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND id=$4 RETURNING session_epoch")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).fetch_one(&mut *db).await?;
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO communication_sessions(id,scope_id,instance_id,external_identity,conversation_id,epoch) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(id).bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(epoch).execute(&mut *db).await?;
    session_by_id(db, s, id).await
}

async fn pending_ids(
    db: &mut PgConnection,
    s: &CommunicationConversationScope,
) -> Result<Vec<Uuid>> {
    let sql = format!("SELECT m.id FROM communication_inputs m WHERE m.scope_id=$1 AND m.instance_id=$2 AND m.external_identity=$3 AND m.conversation_id=$4 AND ({ADMISSIBLE}) ORDER BY m.id");
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(s.workspace_id)
        .bind(s.instance_id)
        .bind(&s.external_identity)
        .bind(s.conversation_id)
        .fetch_all(db)
        .await?)
}

async fn claim_row(
    db: &mut PgConnection,
    c: &CommunicationTurnClaim,
    active: bool,
) -> Result<Option<PgRow>> {
    if lock_chat(db, &c.scope, active).await?.is_none() {
        return Ok(None);
    }
    Ok(sqlx::query("SELECT t.* FROM communication_reply_turns t WHERE t.scope_id=$1 AND t.instance_id=$2 AND t.external_identity=$3 AND t.conversation_id=$4 AND t.id=$5 AND t.session_id=$6 AND t.session_epoch=$7 AND t.claim_id=$8 AND t.claim_expires_at>clock_timestamp() AND (NOT $9 OR (t.state<>'invalidated' AND EXISTS(SELECT 1 FROM communication_sessions s JOIN communication_conversations h ON h.id=s.conversation_id AND h.scope_id=s.scope_id AND h.instance_id=s.instance_id AND h.external_identity=s.external_identity WHERE s.id=t.session_id AND s.scope_id=t.scope_id AND s.instance_id=t.instance_id AND s.external_identity=t.external_identity AND s.conversation_id=t.conversation_id AND s.epoch=t.session_epoch AND s.active AND h.session_epoch=s.epoch))) FOR UPDATE")
        .bind(c.scope.workspace_id).bind(c.scope.instance_id).bind(&c.scope.external_identity).bind(c.scope.conversation_id).bind(c.turn_id).bind(c.session_id).bind(c.session_epoch).bind(c.claim_id).bind(active).fetch_optional(db).await?)
}

async fn snapshot(
    db: &mut PgConnection,
    s: &CommunicationConversationScope,
    row: PgRow,
    key: &str,
) -> Result<CommunicationTurnInputs> {
    let checkpoint = row.try_get("checkpoint_json")?;
    let turn = turn(&row)?;
    let rows = sqlx::query("SELECT m.*,(SELECT t.content FROM communication_messages t WHERE t.input_id=m.id AND t.scope_id=m.scope_id AND t.instance_id=m.instance_id AND t.external_identity=m.external_identity AND t.conversation_id=m.conversation_id AND t.type='text' AND t.direction='inbound') AS content FROM communication_inputs m WHERE m.scope_id=$1 AND m.instance_id=$2 AND m.external_identity=$3 AND m.conversation_id=$4 AND m.turn_id=$5 AND m.turn_sequence>$6 ORDER BY m.turn_sequence")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(turn.id).bind(turn.consumed_watermark).fetch_all(&mut *db).await?;
    let mut inputs = Vec::with_capacity(rows.len());
    for row in rows {
        let input_id: Uuid = row.try_get("id")?;
        let files = sqlx::query("SELECT * FROM communication_messages WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND input_id=$5 AND type='file' AND direction='inbound' ORDER BY id")
            .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(input_id).fetch_all(&mut *db).await?;
        let attachments = files
            .iter()
            .map(|file| -> Result<_> {
                let record = attachment(file)?;
                let reference = decrypt_secret_json_with_aad(
                    &file.try_get::<Value, _>("encrypted_reference")?,
                    key,
                    &private_aad(s.workspace_id, s.instance_id, &record.id.to_string()),
                )?;
                Ok(CommunicationPendingAttachment { record, reference })
            })
            .collect::<Result<Vec<_>>>()?;
        inputs.push(CommunicationTurnInput {
            input_id,
            input_sequence: row.try_get("turn_sequence")?,
            external_message_key: row.try_get("external_message_key")?,
            received_at_ms: row.try_get("received_at_ms")?,
            content: row.try_get("content")?,
            attachments,
        });
    }
    let pending_admission_input_ids = pending_ids(db, s).await?;
    Ok(CommunicationTurnInputs {
        turn,
        checkpoint,
        inputs,
        pending_admission_input_ids,
    })
}

async fn project_progress(db: &mut PgConnection, id: Uuid) -> Result<()> {
    // Canonical run_id stays on the turn; only its anchor keeps the legacy unique input
    // association. History projects the turn run to every member without duplicating it.
    sqlx::query("UPDATE communication_inputs m SET processing_status=t.processing_status,send_status=t.send_status,failure_code=t.failure_code,run_id=CASE WHEN m.id=t.anchor_input_id THEN t.run_id ELSE NULL END FROM communication_reply_turns t WHERE t.id=$1 AND m.turn_id=t.id AND m.scope_id=t.scope_id AND m.instance_id=t.instance_id AND m.external_identity=t.external_identity AND m.conversation_id=t.conversation_id")
        .bind(id).execute(db).await?;
    Ok(())
}
