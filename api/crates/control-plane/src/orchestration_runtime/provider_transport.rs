use anyhow::{anyhow, Result};
use serde_json::json;
use time::OffsetDateTime;

use crate::{
    ports::{
        CommitFlowRunTerminalInput, CommitFlowRunTerminalResult, OrchestrationRuntimeRepository,
        ProviderTransportPayload, ProviderTransportSlotId,
    },
    state_transition::ensure_flow_run_transition,
};

use super::{
    debug_stream_events,
    stream_terminal_recovery::{resolve_terminal_commit, TerminalCommitResolution},
    OrchestrationRuntimeService,
};

const EPHEMERAL_TRANSPORT_MISSING_ERROR_CODE: &str = "ephemeral_transport_missing";
const EPHEMERAL_TRANSPORT_MISSING_ERROR_MESSAGE: &str =
    "provider transport payload is no longer available";

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: OrchestrationRuntimeRepository,
{
    pub(super) async fn resolve_provider_transport_payload(
        &self,
        flow_run: &domain::FlowRunRecord,
        slot_id: Option<ProviderTransportSlotId>,
    ) -> Result<Option<ProviderTransportPayload>> {
        let Some(slot_id) = slot_id else {
            return Ok(None);
        };
        if !matches!(
            flow_run.status,
            domain::FlowRunStatus::Queued | domain::FlowRunStatus::Running
        ) {
            self.delete_provider_transport_slot(slot_id).await;
            return Ok(None);
        }
        let payload = match &self.provider_transport_store {
            Some(store) => match store.get(slot_id).await {
                Ok(payload) => payload,
                Err(_) => {
                    tracing::warn!(
                        flow_run_id = %flow_run.id,
                        "provider transport slot lookup failed"
                    );
                    None
                }
            },
            None => None,
        };
        let Some(payload) = payload else {
            self.fail_published_run_for_missing_provider_transport(flow_run)
                .await?;
            return Err(anyhow!(EPHEMERAL_TRANSPORT_MISSING_ERROR_CODE));
        };
        // Ownership has crossed from short-lived storage into this execution segment. The
        // invoker keeps the in-memory payload across provider retries, so the storage slot
        // lifetime can end before any upstream call starts.
        self.delete_provider_transport_slot(slot_id).await;
        Ok(Some(payload))
    }

    async fn delete_provider_transport_slot(&self, slot_id: ProviderTransportSlotId) {
        let Some(store) = &self.provider_transport_store else {
            return;
        };
        if let Err(error) = store.delete(slot_id).await {
            tracing::warn!(
                flow_run_id = %slot_id.as_uuid(),
                error = %error,
                "provider transport slot cleanup deferred to retention policy"
            );
        }
    }

    async fn fail_published_run_for_missing_provider_transport(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        ensure_flow_run_transition(
            flow_run.status,
            domain::FlowRunStatus::Failed,
            "fail_published_run_for_missing_provider_transport",
        )?;
        let error_payload = json!({
            "code": EPHEMERAL_TRANSPORT_MISSING_ERROR_CODE,
            "message": EPHEMERAL_TRANSPORT_MISSING_ERROR_MESSAGE,
        });
        let terminal_event = debug_stream_events::flow_failed(flow_run.id, error_payload.clone());
        let receipt = self
            .repository
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: flow_run.id,
                expected_status: flow_run.status,
                result: CommitFlowRunTerminalResult::Failed {
                    output_payload: flow_run.output_payload.clone(),
                    error_payload: error_payload.clone(),
                },
                flow_run_event_payload: error_payload,
                terminal_event_payload: terminal_event.payload,
                finished_at: OffsetDateTime::now_utc(),
            })
            .await?;
        let winner = match resolve_terminal_commit(
            &self.repository,
            flow_run.application_id,
            flow_run.id,
            receipt,
        )
        .await?
        {
            TerminalCommitResolution::Winner(winner) | TerminalCommitResolution::Loser(winner) => {
                winner
            }
        };
        self.ensure_durable_terminal_projection(&winner).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration_runtime::test_support::{
        InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime,
    };
    use crate::ports::{CreateFlowRunInput, OrchestrationRuntimeRepository};
    use uuid::Uuid;

    #[tokio::test]
    async fn d4_ac_027_missing_ephemeral_transport_commits_explicit_failed_terminal() {
        let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(Vec::new());
        let flow_run = repository
            .create_flow_run(&CreateFlowRunInput {
                actor_user_id: Uuid::nil(),
                application_id: Uuid::now_v7(),
                flow_id: Uuid::now_v7(),
                flow_draft_id: Uuid::now_v7(),
                compiled_plan_id: Uuid::now_v7(),
                debug_session_id: String::new(),
                flow_schema_version: "1".to_string(),
                document_hash: "hash".to_string(),
                run_mode: domain::FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: "missing ephemeral transport".to_string(),
                status: domain::FlowRunStatus::Queued,
                input_payload: json!({"query": "durable input remains"}),
                started_at: OffsetDateTime::now_utc(),
                api_key_id: Some(Uuid::now_v7()),
                publication_version_id: Some(Uuid::now_v7()),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: None,
                compatibility_mode: None,
                idempotency_key: None,
            })
            .await
            .expect("published flow run should be created");
        let service = OrchestrationRuntimeService::new(
            repository.clone(),
            InMemoryProviderRuntime::default(),
            std::sync::Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
            "test-master-key",
        );

        let error = service
            .resolve_provider_transport_payload(
                &flow_run,
                Some(ProviderTransportSlotId::for_flow_run(flow_run.id)),
            )
            .await
            .expect_err("missing transport payload must fail the accepted run");

        assert!(error.to_string().contains("ephemeral_transport_missing"));
        let failed = repository
            .get_flow_run(flow_run.application_id, flow_run.id)
            .await
            .unwrap()
            .expect("failed run should remain durable");
        assert_eq!(failed.status, domain::FlowRunStatus::Failed);
        assert_eq!(
            failed.error_payload.unwrap()["code"],
            EPHEMERAL_TRANSPORT_MISSING_ERROR_CODE
        );
    }
}
