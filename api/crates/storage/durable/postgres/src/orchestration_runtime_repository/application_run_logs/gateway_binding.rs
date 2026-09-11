use super::*;

impl PgControlPlaneStore {
    pub(super) async fn bind_gateway_log_invocation(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        run: &domain::FlowRunRecord,
        context: &control_plane_contracts::gateway_logs::GatewayLogContext,
    ) -> Result<()> {
        let Some(api_key_id) = run.api_key_id else {
            return Err(anyhow!(
                "gateway log binding requires an authenticated API key"
            ));
        };
        let scope_id: Uuid =
            sqlx::query_scalar("select workspace_id from applications where id=$1")
                .bind(run.application_id)
                .fetch_one(&mut **tx)
                .await?;
        Self::lock_gateway_projection(tx, run.application_id).await?;
        let mut conversation_id = None;
        let mut turn_id = None;
        if matches!(
            context.identity_status.as_str(),
            "identified" | "unknown_turn"
        ) {
            if let Some(thread_id) = context.thread_id.as_deref() {
                let id:Uuid=sqlx::query_scalar("insert into gateway_log_conversations(id,scope_id,application_id,api_key_id,external_user,protocol,thread_id,created_at,updated_at) values($1,$2,$3,$4,$5,'openai_responses',$6,$7,$7) on conflict(application_id,api_key_id,external_user,protocol,thread_id) do update set updated_at=greatest(gateway_log_conversations.updated_at,excluded.updated_at) returning id")
                    .bind(Uuid::now_v7()).bind(scope_id).bind(run.application_id).bind(api_key_id)
                    .bind(run.external_user.as_deref().unwrap_or("")).bind(thread_id).bind(run.started_at)
                    .fetch_one(&mut **tx).await?;
                conversation_id = Some(id);
                if let Some(client_turn_id) = context.turn_id.as_deref() {
                    turn_id=Some(sqlx::query_scalar::<_,Uuid>("insert into gateway_log_turns(id,conversation_id,client_turn_id,created_at,updated_at) values($1,$2,$3,$4,$4) on conflict(conversation_id,client_turn_id) do update set updated_at=greatest(gateway_log_turns.updated_at,excluded.updated_at) returning id")
                        .bind(Uuid::now_v7()).bind(id).bind(client_turn_id).bind(run.started_at).fetch_one(&mut **tx).await?);
                }
            }
        }
        if let Some(task_id) = turn_id {
            // The turn upsert holds its row lock. Only the first invocation may
            // bind a parent, and only a previously committed task can be a parent.
            // Later requests cannot rewrite the edge or introduce a back-edge.
            let first: bool = sqlx::query_scalar(
                "select not exists(select 1 from gateway_log_invocations where turn_id=$1)",
            )
            .bind(task_id)
            .fetch_one(&mut **tx)
            .await?;
            if first
                && (context.parent_thread_id.is_some()
                    || context.parent_turn_id.is_some()
                    || context.forked_from_thread_id.is_some())
            {
                let parent_thread = context
                    .parent_thread_id
                    .as_deref()
                    .or(context.forked_from_thread_id.as_deref())
                    .or(context.thread_id.as_deref());
                let parent=sqlx::query("select c.id as conversation_id,t.id as task_id from gateway_log_conversations c left join gateway_log_turns t on t.conversation_id=c.id and t.client_turn_id=$6 and t.id<>$7 where c.application_id=$1 and c.api_key_id=$2 and c.external_user=$3 and c.protocol='openai_responses' and c.thread_id=$4 and c.scope_id=$5 and exists(select 1 from gateway_log_invocations g where g.conversation_id=c.id and ($6::text is null or g.turn_id=t.id))")
                    .bind(run.application_id).bind(api_key_id).bind(run.external_user.as_deref().unwrap_or(""))
                    .bind(parent_thread).bind(scope_id).bind(context.parent_turn_id.as_deref()).bind(task_id).fetch_optional(&mut **tx).await?;
                let parent_conversation = parent
                    .as_ref()
                    .map(|r| r.try_get::<Uuid, _>("conversation_id"))
                    .transpose()?;
                let parent_task = parent
                    .as_ref()
                    .map(|r| r.try_get::<Option<Uuid>, _>("task_id"))
                    .transpose()?
                    .flatten();
                let relation = if parent_task.is_some() {
                    "resolved_parent"
                } else if parent_conversation.is_some() && context.parent_turn_id.is_none() {
                    "resolved_thread_reference"
                } else {
                    "declared_parent_unresolved"
                };
                sqlx::query("update gateway_log_turns set parent_task_id=$2,parent_conversation_id=$3,relation_status=$4 where id=$1")
                    .bind(task_id).bind(parent_task).bind(parent_conversation).bind(relation).execute(&mut **tx).await?;
            }
        }
        // Only an already-existing run in the same authorization domain can be
        // a cause. New IDs cannot create back-edges or cycles, including on replay.
        let predecessor = context
            .previous_response_id
            .as_deref()
            .and_then(|id| id.strip_prefix("resp_"))
            .and_then(|id| Uuid::parse_str(id).ok());
        let caused_by_run_id = if let Some(id) = predecessor {
            sqlx::query_scalar::<_,Uuid>("select id from flow_runs where id=$1 and id<>$2 and application_id=$3 and api_key_id=$4 and external_user is not distinct from $5")
                .bind(id).bind(run.id).bind(run.application_id).bind(api_key_id).bind(run.external_user.as_deref()).fetch_optional(&mut **tx).await?
        } else {
            None
        };
        sqlx::query("insert into gateway_log_invocations(flow_run_id,scope_id,application_id,api_key_id,conversation_id,turn_id,caused_by_run_id,identity_status,context,created_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(run.id).bind(scope_id).bind(run.application_id).bind(api_key_id).bind(conversation_id).bind(turn_id).bind(caused_by_run_id)
            .bind(&context.identity_status).bind(serde_json::to_value(context)?).bind(run.started_at).execute(&mut **tx).await?;
        if let Some(id) = conversation_id {
            Self::project_gateway_tool_results(
                tx,
                id,
                run.id,
                run.started_at,
                &context.tool_results,
            )
            .await?;
        }
        Ok(())
    }
}
