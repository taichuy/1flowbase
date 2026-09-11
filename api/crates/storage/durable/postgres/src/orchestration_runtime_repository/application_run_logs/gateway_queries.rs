use super::*;
use control_plane_contracts::gateway_logs::*;

#[path = "gateway_metrics.rs"]
mod metrics;
#[path = "gateway_rebuild.rs"]
mod rebuild;

const INVOCATION_COLUMNS: &str = "f.id,'invocation'::text as kind,f.title,c.thread_id,t.client_turn_id,f.id as flow_run_id,coalesce(g.context,'{}'::jsonb)||jsonb_build_object('parent_task_id',t.parent_task_id,'parent_conversation_id',t.parent_conversation_id,'relation_status',t.relation_status) as context,f.started_at,f.finished_at,f.status,g.caused_by_run_id,coalesce(g.identity_status,'legacy_missing_identity') as identity_status";
const INVOCATION_JOINS: &str = "from flow_runs f join applications a on a.id=f.application_id left join gateway_log_invocations g on g.flow_run_id=f.id left join gateway_log_conversations c on c.id=g.conversation_id left join gateway_log_turns t on t.id=g.turn_id";

#[async_trait]
impl GatewayLogRepository for PgControlPlaneStore {
    async fn rebuild_gateway_log_conversation(
        &self,
        scope: &GatewayLogScope,
        conversation_id: Uuid,
    ) -> Result<bool> {
        self.rebuild_gateway_projection(scope, conversation_id)
            .await
    }
    async fn list_gateway_log_page(
        &self,
        scope: &GatewayLogScope,
        query: &GatewayLogQuery,
    ) -> Result<GatewayLogPage> {
        let selectors = [query.conversation_id, query.turn_id, query.flow_run_id]
            .iter()
            .filter(|v| v.is_some())
            .count();
        if selectors > 1 {
            return Err(anyhow!("gateway logs accept exactly one parent selector"));
        }
        let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 50);
        if let Some(run_id) = query.flow_run_id {
            return self
                .gateway_attempt_page(scope, run_id, page, page_size)
                .await;
        }
        let (base, parent) = if let Some(turn_id) = query.turn_id {
            (
                format!(
                    "select {INVOCATION_COLUMNS} {INVOCATION_JOINS} where a.workspace_id=$1 and f.application_id=$2 and ($3::uuid is null or f.api_key_id=$3) and g.turn_id=$4"
                ),
                Some(turn_id),
            )
        } else if let Some(conversation_id) = query.conversation_id {
            (
                format!(
                    "select t.id,'turn'::text as kind,coalesce(first_run.title,t.client_turn_id) as title,c.thread_id,t.client_turn_id,null::uuid as flow_run_id,coalesce(first_run.context,'{{}}'::jsonb)||jsonb_build_object('parent_task_id',t.parent_task_id,'parent_conversation_id',t.parent_conversation_id,'relation_status',t.relation_status) as context,t.created_at as started_at,null::timestamptz as finished_at,null::text as status,null::uuid as caused_by_run_id,'identified'::text as identity_status from gateway_log_turns t join gateway_log_conversations c on c.id=t.conversation_id join lateral(select f.title,g.context from gateway_log_invocations g join flow_runs f on f.id=g.flow_run_id where g.turn_id=t.id order by (g.context->'prompt' is null),g.created_at,g.flow_run_id limit 1) first_run on true where c.scope_id=$1 and c.application_id=$2 and ($3::uuid is null or c.api_key_id=$3) and c.id=$4 union all select {INVOCATION_COLUMNS} {INVOCATION_JOINS} where a.workspace_id=$1 and f.application_id=$2 and ($3::uuid is null or f.api_key_id=$3) and g.conversation_id=$4 and g.turn_id is null"
                ),
                Some(conversation_id),
            )
        } else {
            (
                format!(
                    "select c.id,'conversation'::text as kind,coalesce(first_run.title,c.thread_id) as title,c.thread_id,null::text as client_turn_id,null::uuid as flow_run_id,'{{}}'::jsonb as context,c.created_at as started_at,null::timestamptz as finished_at,null::text as status,null::uuid as caused_by_run_id,'identified'::text as identity_status from gateway_log_conversations c join lateral(select f.title from gateway_log_invocations g join flow_runs f on f.id=g.flow_run_id where g.conversation_id=c.id order by g.created_at,g.flow_run_id limit 1) first_run on true where c.scope_id=$1 and c.application_id=$2 and ($3::uuid is null or c.api_key_id=$3) and $4::uuid is null union all select {INVOCATION_COLUMNS} {INVOCATION_JOINS} where a.workspace_id=$1 and f.application_id=$2 and ($3::uuid is null or f.api_key_id=$3) and f.run_mode='published_api_run' and g.conversation_id is null and $4::uuid is null"
                ),
                None,
            )
        };
        let total: i64 = sqlx::query_scalar(&format!("select count(*) from ({base}) entries"))
            .bind(scope.scope_id)
            .bind(scope.application_id)
            .bind(scope.api_key_id)
            .bind(parent)
            .fetch_one(self.pool())
            .await?;
        let order = if parent.is_none() { "desc" } else { "asc" };
        let rows=sqlx::query(&format!("select * from ({base}) entries order by started_at {order},id {order} limit $5 offset $6"))
            .bind(scope.scope_id).bind(scope.application_id).bind(scope.api_key_id).bind(parent).bind(page_size).bind((page-1)*page_size).fetch_all(self.pool()).await?;
        let mut items = Vec::new();
        for row in rows {
            let context: Value = row.try_get("context")?;
            let mut entry = GatewayLogEntry {
                id: row.try_get("id")?,
                kind: row.try_get("kind")?,
                title: row.try_get("title")?,
                identity_status: row.try_get("identity_status")?,
                thread_id: row.try_get("thread_id")?,
                client_turn_id: row.try_get("client_turn_id")?,
                request_kind: string(&context, "request_kind"),
                parent_thread_id: string(&context, "parent_thread_id"),
                parent_turn_id: string(&context, "parent_turn_id"),
                relation_status: "no_parent_declared".into(),
                parent_task_id: string(&context, "parent_task_id")
                    .and_then(|v| Uuid::parse_str(&v).ok()),
                parent_conversation_id: string(&context, "parent_conversation_id")
                    .and_then(|v| Uuid::parse_str(&v).ok()),
                forked_from_thread_id: string(&context, "forked_from_thread_id"),
                identity_sources: context
                    .get("identity_sources")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                completion_status: "unknown".into(),
                observations: vec![],
                flow_run_id: row.try_get("flow_run_id")?,
                caused_by_run_id: row.try_get("caused_by_run_id")?,
                started_at: timestamp(row.try_get("started_at")?),
                finished_at: row
                    .try_get::<Option<OffsetDateTime>, _>("finished_at")?
                    .map(timestamp),
                status: row.try_get("status")?,
                attempt_index: None,
                is_retry: None,
                error_code: None,
                metrics: GatewayLogMetrics::default(),
                messages: vec![],
                messages_has_more: false,
            };
            if entry.parent_thread_id.is_some() || entry.parent_turn_id.is_some() {
                // A declaration is not an authorized edge. Cross-thread links
                // remain unresolved instead of constructing a possibly cyclic DAG.
                entry.relation_status = if entry.parent_thread_id == entry.thread_id
                    && entry.parent_turn_id == entry.client_turn_id
                {
                    "conflicting_parent"
                } else {
                    "declared_parent_unresolved"
                }
                .into();
            }
            if let Some(relation) = string(&context, "relation_status") {
                entry.relation_status = relation;
            }
            if entry.kind == "turn" {
                if let Some(prompt) = context.get("prompt").filter(|v| !v.is_null()) {
                    if let Some(text) = item_text(prompt) {
                        entry.title = text;
                    }
                }
            }
            let (metrics, observations) =
                self.gateway_metrics(scope, &entry.kind, entry.id).await?;
            entry.metrics = metrics;
            entry.observations = observations;
            if entry.kind == "invocation" {
                let (messages, has_more) = self.gateway_messages(scope, entry.id).await?;
                entry.messages = messages;
                entry.messages_has_more = has_more;
            }
            items.push(entry);
        }
        Ok(GatewayLogPage {
            items,
            total,
            page,
            page_size,
        })
    }
}

impl PgControlPlaneStore {
    async fn gateway_messages(
        &self,
        scope: &GatewayLogScope,
        run_id: Uuid,
    ) -> Result<(Vec<GatewayLogMessage>, bool)> {
        let rows=sqlx::query("select o.*,r.result,r.conflicting as result_conflicting from gateway_log_output_items o join gateway_log_invocations g on g.flow_run_id=o.flow_run_id left join gateway_log_tool_results r on r.conversation_id=o.conversation_id and r.call_id=o.item->>'call_id' where o.flow_run_id=$1 and g.scope_id=$2 and g.application_id=$3 and ($4::uuid is null or g.api_key_id=$4) order by o.sequence,o.item_key limit 101")
            .bind(run_id).bind(scope.scope_id).bind(scope.application_id).bind(scope.api_key_id).fetch_all(self.pool()).await?;
        let has_more = rows.len() > 100;
        let messages = rows
            .into_iter()
            .take(100)
            .map(|row| -> Result<GatewayLogMessage> {
                let item: Value = row.try_get("item")?;
                let result: Option<Value> = row.try_get("result")?;
                let conflicting: bool = row.try_get("conflicting")?;
                let result_conflicting: Option<bool> = row.try_get("result_conflicting")?;
                let expected = match item.get("type").and_then(Value::as_str) {
                    Some("custom_tool_call") => Some("custom_tool_call_output"),
                    Some("function_call") => Some("function_call_output"),
                    _ => None,
                };
                let valid_result = !conflicting
                    && result
                        .as_ref()
                        .is_some_and(|r| r.get("type").and_then(Value::as_str) == expected)
                    && result_conflicting != Some(true);
                Ok(GatewayLogMessage {
                    id: row.try_get("item_key")?,
                    kind: string(&item, "type").unwrap_or("unknown".into()),
                    phase: string(&item, "phase"),
                    text: item_text(&item),
                    call_id: string(&item, "call_id"),
                    tool_name: string(&item, "name"),
                    tool_input: string(&item, "input").or_else(|| string(&item, "arguments")),
                    tool_result: result
                        .as_ref()
                        .and_then(|r| r.get("output"))
                        .map(display_value),
                    result_received: valid_result,
                    execution_verified: false,
                    flow_run_id: run_id,
                    sequence: row.try_get("sequence")?,
                    identity_status: if conflicting || result_conflicting == Some(true) {
                        "conflicting_item"
                    } else {
                        "identified"
                    }
                    .into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((messages, has_more))
    }

    async fn gateway_attempt_page(
        &self,
        scope: &GatewayLogScope,
        run_id: Uuid,
        page: i64,
        page_size: i64,
    ) -> Result<GatewayLogPage> {
        let filter = "from model_provider_request_logs l join flow_runs f on f.id=l.flow_run_id join applications a on a.id=f.application_id where l.scope_id=$1 and a.workspace_id=$1 and f.application_id=$2 and ($3::uuid is null or f.api_key_id=$3) and f.id=$4";
        let total = sqlx::query_scalar::<_, i64>(&format!("select count(*) {filter}"))
            .bind(scope.scope_id)
            .bind(scope.application_id)
            .bind(scope.api_key_id)
            .bind(run_id)
            .fetch_one(self.pool())
            .await?;
        let rows=sqlx::query(&format!("select l.*,l.total_cost::text as cost_amount {filter} order by l.started_at,l.attempt_id limit $5 offset $6"))
            .bind(scope.scope_id).bind(scope.application_id).bind(scope.api_key_id).bind(run_id).bind(page_size).bind((page-1)*page_size).fetch_all(self.pool()).await?;
        let mut items = vec![];
        for row in rows {
            let cost: Option<String> = row.try_get("cost_amount")?;
            let currency: Option<String> = row.try_get("currency_code")?;
            let costs = match (cost, currency) {
                (Some(amount), Some(currency_code)) => vec![GatewayLogCost {
                    amount,
                    currency_code,
                }],
                _ => vec![],
            };
            let unknown_cost_attempts = if costs.is_empty() { 1 } else { 0 };
            let status: String = row.try_get("status")?;
            items.push(GatewayLogEntry {
                id: row.try_get("attempt_id")?,
                kind: "attempt".into(),
                title: row.try_get("upstream_model_id")?,
                identity_status: "identified".into(),
                thread_id: None,
                client_turn_id: None,
                request_kind: None,
                parent_thread_id: None,
                parent_turn_id: None,
                relation_status: "invocation_attempt".into(),
                parent_task_id: None,
                parent_conversation_id: None,
                forked_from_thread_id: None,
                identity_sources: vec![],
                completion_status: "unknown".into(),
                observations: vec![],
                flow_run_id: Some(run_id),
                caused_by_run_id: None,
                started_at: timestamp(row.try_get("started_at")?),
                finished_at: row
                    .try_get::<Option<OffsetDateTime>, _>("finished_at")?
                    .map(timestamp),
                status: Some(status.clone()),
                attempt_index: Some(row.try_get("attempt_index")?),
                is_retry: Some(row.try_get("is_retry")?),
                error_code: row.try_get("error_code")?,
                metrics: GatewayLogMetrics {
                    attempt_count: 1,
                    failed_attempt_count: if status == "failed" { 1 } else { 0 },
                    input_tokens: row.try_get("input_tokens")?,
                    output_tokens: row.try_get("output_tokens")?,
                    total_tokens: row.try_get("total_tokens")?,
                    model_duration_ms: row.try_get("total_duration_ms")?,
                    costs,
                    unknown_cost_attempts,
                    ..Default::default()
                },
                messages: vec![],
                messages_has_more: false,
            });
        }
        Ok(GatewayLogPage {
            items,
            total,
            page,
            page_size,
        })
    }
}
fn timestamp(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .expect("stored timestamp")
}
fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}
fn display_value(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn item_text(item: &Value) -> Option<String> {
    if let Some(content) = item.get("content").and_then(Value::as_str) {
        return Some(content.to_owned());
    }
    let content = item
        .get("content")
        .and_then(Value::as_array)
        .or_else(|| item.get("summary").and_then(Value::as_array))?;
    let text = content
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}
