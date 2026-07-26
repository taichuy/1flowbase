use super::super::event_forwarding::is_public_terminal_runtime_event;
use super::super::*;
use async_trait::async_trait;
use control_plane::{
    application_public_api::native::NativeRunStatus,
    ports::{
        AppendRuntimeEventInput, AppendTerminalIfMissingAndCloseOutcome,
        OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventClosure,
        RuntimeEventPayload, RuntimeEventStream, RuntimeEventStreamPolicy,
        RuntimeEventSubscription, RuntimeEventTrimPolicy,
    },
};
use serde_json::json;
use std::sync::Mutex;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

pub(super) struct ReplayBeforeFallbackRuntimeEventStream {
    events: Vec<RuntimeEventEnvelope>,
    subscription_replay: Vec<RuntimeEventEnvelope>,
    subscription_closure: Option<RuntimeEventClosure>,
    live_senders: Mutex<Vec<mpsc::UnboundedSender<RuntimeEventEnvelope>>>,
}

impl ReplayBeforeFallbackRuntimeEventStream {
    pub(super) fn with_subscription_replay(
        subscription_replay: Vec<RuntimeEventEnvelope>,
        events: Vec<RuntimeEventEnvelope>,
    ) -> Self {
        Self {
            events,
            subscription_replay,
            subscription_closure: None,
            live_senders: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn with_closed_subscription_replay(
        subscription_replay: Vec<RuntimeEventEnvelope>,
        events: Vec<RuntimeEventEnvelope>,
        reason: RuntimeEventCloseReason,
    ) -> Self {
        let final_sequence = subscription_replay
            .iter()
            .chain(events.iter())
            .map(|event| event.sequence)
            .max()
            .unwrap_or(0);
        Self {
            events,
            subscription_replay,
            subscription_closure: Some(RuntimeEventClosure {
                reason,
                final_sequence,
            }),
            live_senders: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn emit_live(&self, event: RuntimeEventEnvelope) {
        self.live_senders
            .lock()
            .expect("live sender lock poisoned")
            .retain(|sender| sender.send(event.clone()).is_ok());
    }

    pub(super) fn active_subscriber_count(&self) -> usize {
        let mut senders = self.live_senders.lock().expect("live sender lock poisoned");
        senders.retain(|sender| !sender.is_closed());
        senders.len()
    }
}

#[async_trait]
impl RuntimeEventStream for ReplayBeforeFallbackRuntimeEventStream {
    async fn open_run(
        &self,
        _run_id: Uuid,
        _policy: RuntimeEventStreamPolicy,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        Ok(RuntimeEventEnvelope::new(run_id, 0, event))
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        _run_id: Uuid,
        _event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("replay fixture does not support atomic terminal claims")
    }

    async fn subscribe(
        &self,
        _run_id: Uuid,
        _from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        let (sender, live_events) = mpsc::unbounded_channel();
        let (_closure_sender, closure) = tokio::sync::watch::channel(self.subscription_closure);
        if self.subscription_closure.is_none() {
            self.live_senders
                .lock()
                .expect("live sender lock poisoned")
                .push(sender);
        }
        Ok(RuntimeEventSubscription {
            replay: self.subscription_replay.clone(),
            live_events: control_plane::ports::RuntimeEventReceiver::from_unbounded(live_events),
            closure,
        })
    }

    async fn replay(
        &self,
        _run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        let from_sequence = from_sequence.unwrap_or(0);
        Ok(self
            .events
            .iter()
            .filter(|event| event.sequence > from_sequence)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn close_run(
        &self,
        _run_id: Uuid,
        _reason: RuntimeEventCloseReason,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn trim(&self, _run_id: Uuid, _policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        Ok(())
    }
}

pub(super) fn native_run() -> NativeRunResult {
    NativeRunResult {
        id: Uuid::from_u128(0x11111111111111111111111111111111),
        application_id: Uuid::from_u128(0x22222222222222222222222222222222),
        api_key_id: Uuid::from_u128(0x33333333333333333333333333333333),
        publication_version_id: Uuid::from_u128(0x44444444444444444444444444444444),
        status: NativeRunStatus::Running,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

pub(super) async fn seed_flow_run_for_compat_sse_test(state: &ApiState, run: &NativeRunResult) {
    let pool = state.store.pool();
    let user_id: Uuid = sqlx::query_scalar("select id from users where account = 'root'")
        .fetch_one(pool)
        .await
        .unwrap();
    let workspace_id: Uuid = sqlx::query_scalar("select id from workspaces limit 1")
        .fetch_one(pool)
        .await
        .unwrap();
    let flow_id = Uuid::now_v7();
    let flow_draft_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();

    sqlx::query(
        r#"
            insert into applications (
                id, workspace_id, application_type, name, description, created_by
            ) values ($1, $2, 'agent_flow', 'compat sse test', '', $3)
            "#,
    )
    .bind(run.application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(run.application_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
            "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), '1flowbase.flow/v2', '{}'::jsonb, $3, $3)",
        )
        .bind(flow_draft_id)
        .bind(flow_id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
            insert into flow_compiled_plans (
                id, flow_id, flow_draft_id, schema_version, document_hash,
                document_updated_at, plan, scope_id, created_by, updated_by
            ) values (
                $1, $2, $3, '1flowbase.flow/v2', 'compat-sse-test',
                now(), '{}'::jsonb, (select scope_id from flows where id = $2), $4, $4
            )
            "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(flow_draft_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
            insert into flow_runs (
                id, application_id, flow_id, flow_draft_id, compiled_plan_id,
                run_mode, status, input_payload, output_payload, created_by,
                started_at, debug_session_id, flow_schema_version, document_hash, title
            ) values (
                $1, $2, $3, $4, $5,
                'published_api_run', 'waiting_callback', '{}'::jsonb, '{}'::jsonb, $6,
                now(), 'compat-sse-test', '1flowbase.flow/v2', 'compat-sse-test',
                'compat sse test'
            )
            "#,
    )
    .bind(run.id)
    .bind(run.application_id)
    .bind(flow_id)
    .bind(flow_draft_id)
    .bind(compiled_plan_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

pub(super) async fn append_compat_sse_runtime_event(
    state: &ApiState,
    run_id: Uuid,
    event_type: &str,
    payload: Value,
) {
    state
        .store
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id: run_id,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            event_type: event_type.to_string(),
            layer: if is_public_terminal_runtime_event(event_type) {
                domain::RuntimeEventLayer::AgentTransition
            } else {
                domain::RuntimeEventLayer::RuntimeItem
            },
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Workspace,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
        .unwrap();
}
