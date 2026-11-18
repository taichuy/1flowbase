//! Channel send receipts link the actual provider attempt to the shared conversation timeline.
use super::*;
use anyhow::Context;
use sqlx::PgConnection;

async fn history_message(
    db: &mut PgConnection,
    workspace: Uuid,
    instance: Uuid,
    target: &extension_contracts::NotificationSendTarget,
    context: Option<&CommunicationSendContext>,
    content: &str,
) -> Result<(Uuid, Uuid, Option<Uuid>)> {
    let row = sqlx::query("SELECT m.id AS input_id,c.* FROM communication_inputs m JOIN communication_conversations c ON c.id=m.conversation_id AND c.scope_id=m.scope_id AND c.instance_id=m.instance_id AND c.external_identity=m.external_identity WHERE m.scope_id=$1 AND m.instance_id=$2 AND m.external_identity=$3 AND c.external_conversation_key=$4 AND m.external_message_key=$5 FOR UPDATE OF c")
        .bind(workspace).bind(instance).bind(&target.sender_id).bind(&target.conversation_id).bind(&target.provider_message_id).fetch_optional(&mut *db).await?.ok_or(Error::Conflict("communication_send_target_missing"))?;
    let input_id: Uuid = row.try_get("input_id")?;
    let conversation_id: Uuid = row.try_get("id")?;
    if let Some(context) = context {
        anyhow::ensure!(
            context.scope.workspace_id == workspace
                && context.scope.instance_id == instance
                && context.scope.conversation_id == conversation_id
                && context.scope.external_identity == target.sender_id
                && context.input_id == input_id,
            "communication_send_context_mismatch"
        );
    }
    let existing = sqlx::query("SELECT id,content FROM communication_messages WHERE scope_id=$1 AND instance_id=$2 AND conversation_id=$3 AND external_identity=$4 AND input_id=$5 AND type='text' AND direction='outbound' FOR UPDATE")
        .bind(workspace).bind(instance).bind(conversation_id).bind(&target.sender_id).bind(input_id).fetch_optional(&mut *db).await?;
    let message_id = if let Some(existing) = existing {
        anyhow::ensure!(
            existing.try_get::<String, _>("content")? == content,
            "communication_send_reply_content_mismatch"
        );
        existing.try_get("id")?
    } else {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,$4,$5,$6,'text','outbound',(extract(epoch FROM clock_timestamp())*1000)::bigint,$7)")
            .bind(id).bind(workspace).bind(instance).bind(&target.sender_id).bind(conversation_id).bind(input_id).bind(content).execute(&mut *db).await?;
        id
    };
    Ok((message_id, conversation_id, Some(input_id)))
}

pub(crate) async fn prepare(
    store: &PgControlPlaneStore,
    input: PrepareCommunicationSend,
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    let row = sqlx::query("SELECT content FROM notification_send_logs WHERE workspace_id=$1 AND instance_id=$2 AND id=$3 AND status='pending' FOR UPDATE")
        .bind(input.workspace_id).bind(input.instance_id).bind(input.send_log_id).fetch_optional(&mut *tx).await?.ok_or(Error::Conflict("notification_log_state"))?;
    let content: String = row
        .try_get::<Option<String>, _>("content")?
        .ok_or(Error::Conflict("communication_send_content_missing"))?;
    anyhow::ensure!(
        !content.trim().is_empty(),
        "communication_send_content_missing"
    );
    let existing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM communication_message_sends WHERE scope_id=$1 AND send_log_id=$2)")
        .bind(input.workspace_id).bind(input.send_log_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(!existing, "communication_send_already_reserved");
    let message = if let Some(target) = &input.target {
        Some(
            history_message(
                &mut tx,
                input.workspace_id,
                input.instance_id,
                target,
                input.context.as_ref(),
                &content,
            )
            .await?,
        )
    } else {
        anyhow::ensure!(
            input.context.is_none(),
            "communication_send_context_requires_target"
        );
        None
    };
    sqlx::query("INSERT INTO communication_message_sends(send_log_id,scope_id,instance_id,message_id,conversation_id,input_id,run_id,target_json,send_status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'sending')")
        .bind(input.send_log_id).bind(input.workspace_id).bind(input.instance_id)
        .bind(message.map(|m|m.0)).bind(message.map(|m|m.1)).bind(message.and_then(|m|m.2))
        .bind(input.context.as_ref().and_then(|c|c.run_id)).bind(input.target.map(serde_json::to_value).transpose()?)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn record(
    store: &PgControlPlaneStore,
    input: RecordCommunicationSendReceipt,
) -> Result<()> {
    let status = serde_json::to_value(input.send_status)?
        .as_str()
        .context("send status")?
        .to_owned();
    anyhow::ensure!(
        matches!(status.as_str(), "succeeded" | "failed" | "unknown"),
        "communication_send_nonterminal_receipt"
    );
    let mut tx = store.pool().begin().await?;
    let row = sqlx::query("SELECT s.*,l.content FROM communication_message_sends s JOIN notification_send_logs l ON l.id=s.send_log_id AND l.workspace_id=s.scope_id AND l.instance_id=s.instance_id WHERE s.scope_id=$1 AND s.send_log_id=$2 FOR UPDATE OF s")
        .bind(input.workspace_id).bind(input.send_log_id).fetch_optional(&mut *tx).await?.ok_or(Error::Conflict("communication_send_history_missing"))?;
    if row.try_get::<String, _>("send_status")? != "sending" {
        // Never replace a provider-confirmed outcome with an error from local finalization.
        tx.commit().await?;
        return Ok(());
    }
    let instance: Uuid = row.try_get("instance_id")?;
    let mut message_id: Option<Uuid> = row.try_get("message_id")?;
    let mut conversation_id: Option<Uuid> = row.try_get("conversation_id")?;
    if let Some(receipt) = &input.receipt {
        anyhow::ensure!(
            receipt.request_id == input.send_log_id.to_string(),
            "communication_send_receipt_mismatch"
        );
        if let Some(target) = row.try_get::<Option<Value>, _>("target_json")? {
            let target: extension_contracts::NotificationSendTarget =
                serde_json::from_value(target)?;
            anyhow::ensure!(
                target.sender_id == receipt.recipient_id
                    && target.conversation_id == receipt.conversation_id,
                "communication_send_receipt_target_mismatch"
            );
        } else {
            // An unprompted send is its own message, with no fabricated inbound receipt.
            let conversation: Uuid = sqlx::query_scalar("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) VALUES($1,$2,$3,$4,$5) ON CONFLICT(scope_id,instance_id,external_identity,external_conversation_key) DO UPDATE SET external_conversation_key=excluded.external_conversation_key RETURNING id")
                .bind(Uuid::now_v7()).bind(input.workspace_id).bind(instance).bind(&receipt.recipient_id).bind(&receipt.conversation_id).fetch_one(&mut *tx).await?;
            let id = Uuid::now_v7();
            let content: String = row.try_get("content")?;
            sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,$4,$5,NULL,'text','outbound',(extract(epoch FROM clock_timestamp())*1000)::bigint,$6)")
                .bind(id).bind(input.workspace_id).bind(instance).bind(&receipt.recipient_id).bind(conversation).bind(content).execute(&mut *tx).await?;
            message_id = Some(id);
            conversation_id = Some(conversation);
        }
    }
    anyhow::ensure!(
        status != "succeeded" || message_id.is_some(),
        "communication_send_success_missing_destination"
    );
    // A successful send log must never commit without its receipt and timeline projection.
    let log_status = match status.as_str() {
        "succeeded" => "success",
        "failed" => "failure",
        _ => "unknown",
    };
    sqlx::query("UPDATE notification_send_logs SET status=$3,failure_code=$4,extra=extra || coalesce($5,'{}'::jsonb),completed_at=now() WHERE workspace_id=$1 AND id=$2 AND status='pending'")
        .bind(input.workspace_id).bind(input.send_log_id).bind(log_status)
        .bind(&input.failure_code).bind(input.log_extra).execute(&mut *tx).await?;
    sqlx::query("UPDATE communication_message_sends SET message_id=$3,conversation_id=$4,receipt_json=$5,send_status=$6,failure_code=$7 WHERE scope_id=$1 AND send_log_id=$2")
        .bind(input.workspace_id).bind(input.send_log_id).bind(message_id).bind(conversation_id)
        .bind(input.receipt.map(serde_json::to_value).transpose()?).bind(status).bind(input.failure_code).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
