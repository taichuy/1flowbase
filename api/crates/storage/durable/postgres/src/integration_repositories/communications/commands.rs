//! Durable command inbox and reply receipts; plugin transport remains the lifecycle Outbox.
use super::*;
use extension_contracts::{CommunicationCommandReply, CommunicationCommandRequest};
use sqlx::PgConnection;

fn record(row: &PgRow) -> Result<CommunicationCommandRecord> {
    let graph: Option<String> = row.try_get("graph_fingerprint")?;
    Ok(CommunicationCommandRecord {
        event_id: row.try_get("event_id")?,
        scope: CommunicationConversationScope {
            workspace_id: row.try_get("scope_id")?,
            instance_id: row.try_get("instance_id")?,
            external_identity: row.try_get("external_identity")?,
            conversation_id: row.try_get("conversation_id")?,
        },
        request: serde_json::from_value(row.try_get("request_json")?)?,
        installation_id: row.try_get("installation_id")?,
        contribution_id: row.try_get("contribution_id")?,
        target: graph
            .map(|graph_fingerprint| {
                Ok::<_, anyhow::Error>(ManagedFrozenExecutionTarget {
                    graph_fingerprint,
                    handler_id: row.try_get("handler_id")?,
                    handler_version: row.try_get("handler_version")?,
                })
            })
            .transpose()?,
        subscriber_id: row.try_get("subscriber_id")?,
        reply: row
            .try_get::<Option<Value>, _>("reply_json")?
            .map(serde_json::from_value)
            .transpose()?,
        failure_code: row.try_get("failure_code")?,
        send_status: serde_json::from_value(Value::String(row.try_get("send_status")?))?,
        response_session_id: row.try_get("response_session_id")?,
    })
}

async fn scoped(
    db: &mut PgConnection,
    scope: &CommunicationConversationScope,
    event_id: Uuid,
) -> Result<Option<PgRow>> {
    Ok(sqlx::query("SELECT * FROM communication_command_events WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND event_id=$5 FOR UPDATE")
        .bind(scope.workspace_id).bind(scope.instance_id).bind(&scope.external_identity).bind(scope.conversation_id).bind(event_id).fetch_optional(db).await?)
}

async fn project(db: &mut PgConnection, event_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE communication_inputs m SET admission_status='excluded',processing_status=CASE WHEN e.failure_code IS NULL THEN 'succeeded' ELSE 'failed' END,send_status=e.send_status,failure_code=e.failure_code,claim_id=NULL,claim_expires_at=NULL FROM communication_command_events e WHERE e.event_id=$1 AND e.reply_json IS NOT NULL AND m.id=e.input_id AND m.scope_id=e.scope_id AND m.instance_id=e.instance_id AND m.external_identity=e.external_identity AND m.conversation_id=e.conversation_id AND m.turn_id IS NULL")
        .bind(event_id).execute(db).await?;
    Ok(())
}

async fn finish(
    db: &mut PgConnection,
    current: CommunicationCommandRecord,
    response_session_id: Uuid,
    reply: CommunicationCommandReply,
    failure_code: Option<String>,
) -> Result<CommunicationCommandRecord> {
    reply.validate()?;
    anyhow::ensure!(
        failure_code
            .as_ref()
            .is_none_or(|code| !code.trim().is_empty() && code.len() <= 512),
        "invalid command failure"
    );
    if current.reply.is_some() {
        // First committed result wins across retries; never overwrite a historical reply.
        return Ok(current);
    }
    let outbound = Uuid::now_v7();
    sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,external_identity,conversation_id,input_id,type,direction,occurred_at_ms,content) VALUES($1,$2,$3,$4,$5,$6,'text','outbound',(extract(epoch FROM clock_timestamp())*1000)::bigint,$7)")
        .bind(outbound).bind(current.scope.workspace_id).bind(current.scope.instance_id).bind(&current.scope.external_identity).bind(current.scope.conversation_id).bind(Uuid::parse_str(&current.request.input_id)?).bind(&reply.text).execute(&mut *db).await?;
    let row = sqlx::query("UPDATE communication_command_events SET reply_json=$2,failure_code=$3,response_session_id=$4,send_status='pending',outbound_message_id=$5,effect_claim_id=NULL,effect_claim_expires_at=NULL WHERE event_id=$1 RETURNING *")
        .bind(current.event_id).bind(serde_json::to_value(reply)?).bind(failure_code).bind(response_session_id).bind(outbound).fetch_one(&mut *db).await?;
    project(db, current.event_id).await?;
    record(&row)
}

/// Called only on the authority lease's connection after current contribution validation.
pub(crate) async fn complete_managed(
    db: &mut PgConnection,
    input: CompleteManagedCommunicationCommand,
) -> Result<CommunicationCommandRecord> {
    let workspace_id = Uuid::parse_str(input.subject.workspace_id().as_str())?;
    let row =
        sqlx::query("SELECT * FROM communication_command_events WHERE scope_id=$1 AND event_id=$2")
            .bind(workspace_id)
            .bind(input.event_id)
            .fetch_one(&mut *db)
            .await?;
    let scope = record(&row)?.scope;
    anyhow::ensure!(
        sessions::lock_chat(db, &scope, true).await?.is_some(),
        "command channel inactive"
    );
    let row = scoped(db, &scope, input.event_id)
        .await?
        .ok_or(Error::NotFound("communication_command"))?;
    let current = record(&row)?;
    anyhow::ensure!(
        current.installation_id == Some(Uuid::parse_str(input.subject.installation_id().as_str())?)
            && current.contribution_id.as_deref() == Some(input.subject.contribution_id().as_str())
            && current.subscriber_id.as_deref() == Some(input.target.subscriber_id.as_str())
            && current
                .target
                .as_ref()
                .is_some_and(|target| target.graph_fingerprint == input.graph_fingerprint
                    && target.handler_id == input.target.handler_id
                    && target.handler_version == input.target.handler_version),
        "command frozen target mismatch"
    );
    let config: Value = sqlx::query_scalar(
        "SELECT config_json FROM communication_bindings WHERE scope_id=$1 AND instance_id=$2",
    )
    .bind(scope.workspace_id)
    .bind(scope.instance_id)
    .fetch_one(&mut *db)
    .await?;
    let settings: CommunicationBindingInput = serde_json::from_value(config)?;
    anyhow::ensure!(
        !settings
            .command_settings
            .iter()
            .any(|setting| setting.command_id == current.request.command_id && !setting.enabled),
        "command disabled before reply commit"
    );
    let session = Uuid::parse_str(&current.request.session_id)?;
    finish(db, current, session, input.reply, input.failure_code).await
}

#[async_trait]
impl CommunicationCommandRepository for PgControlPlaneStore {
    async fn begin_communication_command(
        &self,
        input: BeginCommunicationCommand,
    ) -> Result<CommunicationCommandRecord> {
        let mut tx = self.pool().begin().await?;
        anyhow::ensure!(
            sessions::lock_chat(&mut tx, &input.scope, true)
                .await?
                .is_some(),
            "command channel inactive"
        );
        if let Some(existing) = scoped(&mut tx, &input.scope, input.input_id).await? {
            let existing = record(&existing)?;
            anyhow::ensure!(
                existing.request.command_id == input.command_id
                    && existing.request.arguments == input.arguments,
                "command input identity conflicts with persisted intent"
            );
            tx.commit().await?;
            return Ok(existing);
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND id=$5 AND admission_status IN ('pending','legacy') AND run_id IS NULL AND turn_id IS NULL AND send_status='none' AND processing_status='pending')")
            .bind(input.scope.workspace_id).bind(input.scope.instance_id).bind(&input.scope.external_identity).bind(input.scope.conversation_id).bind(input.input_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(exists, "command requires unadmitted persisted input");
        let session = sessions::ensure_session(&mut tx, &input.scope).await?;
        let request = CommunicationCommandRequest {
            command_id: input.command_id,
            arguments: input.arguments,
            channel_id: input.scope.instance_id.to_string(),
            conversation_id: input.scope.conversation_id.to_string(),
            session_id: session.id.to_string(),
            input_id: input.input_id.to_string(),
        };
        request.validate()?;
        let event_id = input.input_id;
        let (installation, contribution, graph, subscriber, handler, version) =
            if let Some(target) = &input.target {
                anyhow::ensure!(
                    !request.command_id.starts_with("host/")
                        && target.subject.workspace_id().as_str()
                            == input.scope.workspace_id.to_string()
                        && target.publication.plan.subscribers.len() == 1,
                    "command requires one workspace target"
                );
                let subscriber = &target.publication.plan.subscribers[0];
                let delivery = communication_command_delivery(event_id, &input.scope, &request)?;
                crate::lifecycle_outbox_repository::record_lifecycle_fact_in_transaction(
                    &mut tx,
                    &RecordLifecycleFactInput {
                        event_id,
                        transaction_id: event_id,
                        contract_id: extension_contracts::COMMUNICATION_COMMAND_POINT.into(),
                        contract_version: "1".into(),
                        canonical_payload: serde_json::to_vec(&delivery)?,
                        occurred_at: time::OffsetDateTime::now_utc(),
                        publication: target.publication.plan.clone(),
                    },
                )
                .await?;
                (
                    Some(Uuid::parse_str(target.subject.installation_id().as_str())?),
                    Some(target.subject.contribution_id().as_str().to_string()),
                    Some(target.publication.plan.graph_fingerprint.clone()),
                    Some(subscriber.subscriber_id.clone()),
                    Some(subscriber.handler_id.clone()),
                    Some(subscriber.handler_version.clone()),
                )
            } else {
                anyhow::ensure!(
                    matches!(
                        request.command_id.as_str(),
                        "host/new" | "host/model" | "host/help" | "host/rejected"
                    ),
                    "plugin command requires frozen execution target"
                );
                (None, None, None, None, None, None)
            };
        let row = sqlx::query("INSERT INTO communication_command_events(event_id,input_id,scope_id,instance_id,external_identity,conversation_id,session_id,request_json,installation_id,contribution_id,graph_fingerprint,subscriber_id,handler_id,handler_version) VALUES($1,$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING *")
            .bind(event_id).bind(input.scope.workspace_id).bind(input.scope.instance_id).bind(&input.scope.external_identity).bind(input.scope.conversation_id).bind(session.id).bind(serde_json::to_value(request)?).bind(installation).bind(contribution).bind(graph).bind(subscriber).bind(handler).bind(version).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        // Keep the frozen snapshot permit alive through the durable request+Outbox commit.
        drop(input.target);
        record(&row)
    }

    async fn communication_command_for_input(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
    ) -> Result<Option<CommunicationCommandRecord>> {
        sqlx::query("SELECT * FROM communication_command_events WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND input_id=$5")
            .bind(scope.workspace_id).bind(scope.instance_id).bind(&scope.external_identity).bind(scope.conversation_id).bind(input_id).fetch_optional(self.pool()).await?.as_ref().map(record).transpose()
    }
    async fn communication_command(
        &self,
        workspace_id: Uuid,
        event_id: Uuid,
    ) -> Result<Option<CommunicationCommandRecord>> {
        sqlx::query("SELECT * FROM communication_command_events WHERE scope_id=$1 AND event_id=$2")
            .bind(workspace_id)
            .bind(event_id)
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(record)
            .transpose()
    }

    async fn claim_builtin_communication_command(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
    ) -> Result<Option<ClaimedBuiltinCommunicationCommand>> {
        let claim_id = Uuid::now_v7();
        // Requests remain ordered per chat while separate chats and ordinary generations run.
        let row = sqlx::query("WITH candidate AS (SELECT e.event_id FROM communication_command_events e JOIN notification_instances n ON n.workspace_id=e.scope_id AND n.id=e.instance_id AND n.status='enabled' WHERE e.scope_id=$1 AND e.instance_id=$2 AND e.installation_id IS NULL AND e.reply_json IS NULL AND (e.effect_claim_id IS NULL OR e.effect_claim_expires_at<=clock_timestamp()) AND NOT EXISTS(SELECT 1 FROM communication_command_events earlier WHERE earlier.scope_id=e.scope_id AND earlier.instance_id=e.instance_id AND earlier.external_identity=e.external_identity AND earlier.conversation_id=e.conversation_id AND earlier.event_id<e.event_id AND earlier.reply_json IS NULL) ORDER BY e.event_id FOR UPDATE OF e SKIP LOCKED LIMIT 1) UPDATE communication_command_events e SET effect_claim_id=$3,effect_claim_expires_at=clock_timestamp()+make_interval(secs=>$4) FROM candidate c WHERE e.event_id=c.event_id RETURNING e.*")
            .bind(workspace_id).bind(instance_id).bind(claim_id).bind(f64::from(lease_seconds.clamp(1, 300))).fetch_optional(self.pool()).await?;
        row.as_ref()
            .map(|row| {
                Ok(ClaimedBuiltinCommunicationCommand {
                    record: record(row)?,
                    claim_id,
                })
            })
            .transpose()
    }

    async fn renew_builtin_communication_command(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        lease_seconds: u32,
    ) -> Result<bool> {
        Ok(sqlx::query("UPDATE communication_command_events e SET effect_claim_expires_at=clock_timestamp()+make_interval(secs=>$4) WHERE event_id=$1 AND scope_id=$2 AND effect_claim_id=$3 AND effect_claim_expires_at>clock_timestamp() AND reply_json IS NULL AND EXISTS(SELECT 1 FROM notification_instances n WHERE n.workspace_id=e.scope_id AND n.id=e.instance_id AND n.status='enabled')")
            .bind(claim.record.event_id).bind(claim.record.scope.workspace_id).bind(claim.claim_id).bind(f64::from(lease_seconds.clamp(1,300))).execute(self.pool()).await?.rows_affected() == 1)
    }

    async fn complete_builtin_communication_command(
        &self,
        claim: &ClaimedBuiltinCommunicationCommand,
        response_session_id: Uuid,
        reply: CommunicationCommandReply,
        failure_code: Option<String>,
    ) -> Result<CommunicationCommandRecord> {
        let mut tx = self.pool().begin().await?;
        anyhow::ensure!(
            sessions::lock_chat(&mut tx, &claim.record.scope, true)
                .await?
                .is_some(),
            "command channel inactive"
        );
        let row = scoped(&mut tx, &claim.record.scope, claim.record.event_id)
            .await?
            .ok_or(Error::NotFound("communication_command"))?;
        let current = record(&row)?;
        anyhow::ensure!(
            current.installation_id.is_none(),
            "plugin reply requires current authority"
        );
        if current.reply.is_some() {
            tx.commit().await?;
            return Ok(current);
        }
        let alive: bool = sqlx::query_scalar("SELECT effect_claim_id=$2 AND effect_claim_expires_at>clock_timestamp() FROM communication_command_events WHERE event_id=$1")
            .bind(current.event_id).bind(claim.claim_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(alive, "command effect claim expired or replaced");
        let requested = Uuid::parse_str(&current.request.session_id)?;
        anyhow::ensure!(
            response_session_id == requested || current.request.command_id == "host/new",
            "only new-session confirmation may change reply session"
        );
        let result = finish(&mut tx, current, response_session_id, reply, failure_code).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn pending_communication_command_responses(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        limit: u32,
    ) -> Result<Vec<CommunicationCommandRecord>> {
        let mut tx = self.pool().begin().await?;
        let expired: Vec<Uuid> = sqlx::query_scalar("UPDATE communication_command_events SET send_status='unknown',failure_code=COALESCE(failure_code,'command_send_outcome_unknown') WHERE scope_id=$1 AND instance_id=$2 AND send_status='sending' AND send_claim_expires_at<=clock_timestamp() RETURNING event_id")
            .bind(workspace_id).bind(instance_id).fetch_all(&mut *tx).await?;
        for event in expired {
            project(&mut tx, event).await?;
        }
        let rows = sqlx::query("SELECT e.* FROM communication_command_events e JOIN notification_instances n ON n.workspace_id=e.scope_id AND n.id=e.instance_id AND n.status='enabled' JOIN communication_sessions s ON s.id=e.response_session_id AND s.scope_id=e.scope_id AND s.instance_id=e.instance_id AND s.external_identity=e.external_identity AND s.conversation_id=e.conversation_id AND s.active WHERE e.scope_id=$1 AND e.instance_id=$2 AND e.send_status='pending' ORDER BY e.event_id LIMIT $3")
            .bind(workspace_id).bind(instance_id).bind(i64::from(limit.clamp(1,100))).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        rows.iter().map(record).collect()
    }

    async fn reserve_communication_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
    ) -> Result<Option<ClaimedCommunicationCommandResponse>> {
        let mut tx = self.pool().begin().await?;
        if sessions::lock_chat(&mut tx, scope, true).await?.is_none() {
            return Ok(None);
        }
        let Some(row) = scoped(&mut tx, scope, event_id).await? else {
            return Ok(None);
        };
        let current = record(&row)?;
        if current.send_status != CommunicationSendStatus::Pending {
            return Ok(None);
        }
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM communication_sessions WHERE id=$1 AND scope_id=$2 AND instance_id=$3 AND external_identity=$4 AND conversation_id=$5 AND active)")
            .bind(current.response_session_id).bind(scope.workspace_id).bind(scope.instance_id).bind(&scope.external_identity).bind(scope.conversation_id).fetch_one(&mut *tx).await?;
        if !active {
            return Ok(None);
        }
        let claim_id = Uuid::now_v7();
        let row = sqlx::query("UPDATE communication_command_events SET send_status='sending',send_claim_id=$2,send_claim_expires_at=clock_timestamp()+interval '5 minutes' WHERE event_id=$1 RETURNING *")
            .bind(event_id).bind(claim_id).fetch_one(&mut *tx).await?;
        project(&mut tx, event_id).await?;
        tx.commit().await?;
        Ok(Some(ClaimedCommunicationCommandResponse {
            record: record(&row)?,
            claim_id,
        }))
    }

    async fn record_communication_command_response(
        &self,
        scope: &CommunicationConversationScope,
        event_id: Uuid,
        claim_id: Uuid,
        outcome: CommunicationCommandSendOutcome,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        // A submitted provider result remains recordable after rotation/disable. It grants no send.
        let status = match outcome {
            CommunicationCommandSendOutcome::Succeeded => "succeeded",
            CommunicationCommandSendOutcome::Failed => "failed",
            CommunicationCommandSendOutcome::Unknown => "unknown",
        };
        let changed = sqlx::query("UPDATE communication_command_events SET send_status=$7 WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND conversation_id=$4 AND event_id=$5 AND send_claim_id=$6 AND send_status IN ('sending','unknown')")
            .bind(scope.workspace_id).bind(scope.instance_id).bind(&scope.external_identity).bind(scope.conversation_id).bind(event_id).bind(claim_id).bind(status).execute(&mut *tx).await?.rows_affected() == 1;
        if changed {
            project(&mut tx, event_id).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
}
