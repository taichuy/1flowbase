use super::*;

pub(in super::super) async fn scan(
    store: &PgControlPlaneStore,
    w: Uuid,
    i: Uuid,
    limit: u32,
) -> Result<Vec<CommunicationPendingAdmission>> {
    let sql = format!(
        "SELECT candidate.* FROM (SELECT DISTINCT ON (m.conversation_id) c.*,m.id AS input_id,m.external_message_key,m.received_at_ms,(SELECT a.content FROM communication_messages a WHERE a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id AND a.input_id=m.id AND a.type='text' AND a.direction='inbound') AS content,EXISTS(SELECT 1 FROM communication_messages a WHERE a.scope_id=m.scope_id AND a.instance_id=m.instance_id AND a.external_identity=m.external_identity AND a.conversation_id=m.conversation_id AND a.input_id=m.id AND a.type='file' AND a.direction='inbound') AS has_attachments FROM communication_inputs m JOIN communication_conversations c ON c.id=m.conversation_id AND c.scope_id=m.scope_id AND c.instance_id=m.instance_id AND c.external_identity=m.external_identity JOIN notification_instances n ON n.id=m.instance_id AND n.workspace_id=m.scope_id AND n.status='enabled' JOIN communication_bindings b ON b.scope_id=m.scope_id AND b.instance_id=m.instance_id WHERE m.scope_id=$1 AND m.instance_id=$2 AND ({ADMISSIBLE}) ORDER BY m.conversation_id,m.id) candidate WHERE NOT EXISTS(SELECT 1 FROM communication_command_events e WHERE e.scope_id=candidate.scope_id AND e.instance_id=candidate.instance_id AND e.input_id=candidate.input_id) ORDER BY candidate.input_id LIMIT $3"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(w)
        .bind(i)
        .bind(i64::from(limit.clamp(1, 100)))
        .fetch_all(store.pool())
        .await?;
    rows.into_iter()
        .map(|row| {
            let input_id = row.try_get("input_id")?;
            let external_message_key = row.try_get("external_message_key")?;
            let received_at_ms = row.try_get("received_at_ms")?;
            let content = row.try_get("content")?;
            let has_attachments = row.try_get("has_attachments")?;
            Ok(CommunicationPendingAdmission {
                conversation: conversation(row),
                input_id,
                external_message_key,
                received_at_ms,
                content,
                has_attachments,
            })
        })
        .collect()
}

pub(in super::super) async fn pending(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
) -> Result<Vec<Uuid>> {
    let mut tx = store.pool().begin().await?;
    if lock_chat(&mut tx, s, true).await?.is_none() {
        return Ok(Vec::new());
    }
    let ids = pending_ids(&mut tx, s).await?;
    tx.commit().await?;
    Ok(ids)
}

pub(in super::super) async fn admit(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
    input_id: Uuid,
) -> Result<Option<CommunicationAdmission>> {
    let mut tx = store.pool().begin().await?;
    if lock_chat(&mut tx, s, true).await?.is_none() {
        return Ok(None);
    }
    let member = sqlx::query("SELECT turn_id,turn_sequence FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND id=$5 FOR UPDATE")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(input_id).fetch_optional(&mut *tx).await?;
    let Some(member) = member else {
        return Ok(None);
    };
    if let Some(turn_id) = member.try_get::<Option<Uuid>, _>("turn_id")? {
        let row = sqlx::query("SELECT * FROM communication_reply_turns WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND id=$5")
            .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(turn_id).fetch_one(&mut *tx).await?;
        let turn = turn(&row)?;
        let session = session_by_id(&mut tx, s, turn.session_id).await?;
        let result = CommunicationAdmission {
            session,
            turn,
            input_id,
            input_sequence: member.try_get("turn_sequence")?,
        };
        tx.commit().await?;
        return Ok(Some(result));
    }
    // Admission order is durable receive order, even when classifiers race. The host
    // resolves earlier inputs first; it may explicitly exclude a command from any position.
    if pending_ids(&mut tx, s).await?.first() != Some(&input_id) {
        return Ok(None);
    }
    let session = ensure_session(&mut tx, s).await?;
    let row = sqlx::query("SELECT * FROM communication_reply_turns WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND session_id=$5 AND session_epoch=$6 AND state='open' FOR UPDATE")
        .bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(session.id).bind(session.epoch).fetch_optional(&mut *tx).await?;
    let row = match row {
        Some(row) => row,
        None => sqlx::query("INSERT INTO communication_reply_turns(id,scope_id,instance_id,external_identity,conversation_id,session_id,session_epoch,anchor_input_id,selected_model) SELECT $1,$2,$3,$4,$5,$6,$7,$8,COALESCE(c.selected_model,b.config_json->>'model') FROM communication_conversations c JOIN communication_bindings b ON b.scope_id=c.scope_id AND b.instance_id=c.instance_id WHERE c.id=$5 AND c.scope_id=$2 AND c.instance_id=$3 AND c.external_identity=$4 RETURNING *")
            .bind(Uuid::now_v7()).bind(s.workspace_id).bind(s.instance_id).bind(&s.external_identity).bind(s.conversation_id).bind(session.id).bind(session.epoch).bind(input_id).fetch_one(&mut *tx).await?,
    };
    let id: Uuid = row.try_get("id")?;
    let row = sqlx::query("UPDATE communication_reply_turns SET received_watermark=received_watermark+1 WHERE id=$1 RETURNING *")
        .bind(id).fetch_one(&mut *tx).await?;
    let turn = turn(&row)?;
    sqlx::query("UPDATE communication_inputs SET admission_status='admitted',turn_id=$2,turn_sequence=$3,claim_id=NULL,claim_expires_at=NULL,processing_status=$4 WHERE id=$1")
        .bind(input_id).bind(turn.id).bind(turn.received_watermark).bind(row.try_get::<String, _>("processing_status")?).execute(&mut *tx).await?;
    let input_sequence = turn.received_watermark;
    tx.commit().await?;
    Ok(Some(CommunicationAdmission {
        session,
        turn,
        input_id,
        input_sequence,
    }))
}

pub(in super::super) async fn exclude(
    store: &PgControlPlaneStore,
    s: &CommunicationConversationScope,
    input_id: Uuid,
) -> Result<bool> {
    let mut tx = store.pool().begin().await?;
    if lock_chat(&mut tx, s, true).await?.is_none() {
        return Ok(false);
    }
    let sql = format!(
        "UPDATE communication_inputs m SET admission_status='excluded',claim_id=NULL,claim_expires_at=NULL WHERE m.scope_id=$1 AND m.instance_id=$2 AND m.external_identity=$3 AND m.conversation_id=$4 AND m.id=$5 AND (({ADMISSIBLE}) OR m.admission_status='excluded')"
    );
    let changed = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(s.workspace_id)
        .bind(s.instance_id)
        .bind(&s.external_identity)
        .bind(s.conversation_id)
        .bind(input_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        == 1;
    tx.commit().await?;
    Ok(changed)
}
