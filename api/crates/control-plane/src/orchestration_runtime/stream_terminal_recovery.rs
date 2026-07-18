use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{
        FinalizePublishedRunMissingStreamTerminalPersistenceInput,
        FinalizePublishedRunMissingStreamTerminalPersistenceOutcome,
        OrchestrationRuntimeRepository, RuntimeEventDurability,
    },
    state_transition::ensure_flow_run_transition,
};

use super::{
    debug_stream_events, is_expected_runtime_event_stream_closed_error, OrchestrationRuntimeService,
};

pub struct FinalizePublishedRunMissingStreamTerminalCommand {
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
}

const STREAM_TERMINAL_MISSING_ERROR_CODE: &str = "stream_terminal_missing";
const STREAM_TERMINAL_MISSING_ERROR_MESSAGE: &str =
    "runtime event stream ended without a terminal event";

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: OrchestrationRuntimeRepository,
{
    pub async fn finalize_published_run_missing_stream_terminal(
        &self,
        command: FinalizePublishedRunMissingStreamTerminalCommand,
    ) -> Result<domain::FlowRunRecord> {
        let flow_run = self
            .repository
            .get_flow_run(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        if flow_run.run_mode != domain::FlowRunMode::PublishedApiRun {
            return Err(anyhow!(
                "stream terminal recovery only accepts published API runs"
            ));
        }

        let expected_status = match flow_run.status {
            domain::FlowRunStatus::Queued | domain::FlowRunStatus::Running => flow_run.status,
            domain::FlowRunStatus::Failed if is_stream_terminal_missing_failure(&flow_run) => {
                let error_payload = flow_run
                    .error_payload
                    .clone()
                    .expect("stable stream terminal failure must retain its error payload");
                self.ensure_recovered_terminal_event(flow_run.id, error_payload)
                    .await?;
                return Ok(flow_run);
            }
            domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Incomplete
            | domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled
            | domain::FlowRunStatus::WaitingCallback
            | domain::FlowRunStatus::WaitingHuman => return Ok(flow_run),
            domain::FlowRunStatus::Paused => {
                return Err(anyhow!(
                    "stream terminal recovery cannot finalize a paused published API run"
                ));
            }
        };
        ensure_flow_run_transition(
            expected_status,
            domain::FlowRunStatus::Failed,
            "finalize_published_run_missing_stream_terminal",
        )?;

        let error_payload = stream_terminal_missing_error_payload();
        let terminal_event = debug_stream_events::flow_failed(flow_run.id, error_payload.clone());
        let persistence_outcome = self
            .repository
            .finalize_published_run_missing_stream_terminal(
                &FinalizePublishedRunMissingStreamTerminalPersistenceInput {
                    flow_run_id: flow_run.id,
                    expected_status,
                    output_payload: flow_run.output_payload.clone(),
                    error_payload: error_payload.clone(),
                    terminal_event_payload: terminal_event.payload.clone(),
                    finished_at: OffsetDateTime::now_utc(),
                },
            )
            .await?;
        let failed_run = match persistence_outcome {
            FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::Finalized(flow_run) => {
                flow_run
            }
            FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::FinalizedWithPostCommitProjectionWarning(flow_run) => {
                tracing::warn!(
                    flow_run_id = %flow_run.id,
                    application_id = %flow_run.application_id,
                    "published stream EOF recovery committed canonical terminal with a post-commit projection warning"
                );
                flow_run
            }
            FinalizePublishedRunMissingStreamTerminalPersistenceOutcome::CasMiss => {
                let winner = self
                    .repository
                    .get_flow_run(command.application_id, command.flow_run_id)
                    .await?
                    .ok_or_else(|| anyhow!("flow run disappeared after terminal recovery CAS"))?;
                if is_stream_terminal_missing_failure(&winner) {
                    let error_payload = winner
                        .error_payload
                        .clone()
                        .expect("stable stream terminal failure must retain its error payload");
                    self.ensure_recovered_terminal_event(winner.id, error_payload)
                        .await?;
                }
                return Ok(winner);
            }
        };

        self.ensure_recovered_terminal_event(failed_run.id, error_payload)
            .await?;

        Ok(failed_run)
    }

    async fn ensure_recovered_terminal_event(
        &self,
        flow_run_id: Uuid,
        error_payload: Value,
    ) -> Result<()> {
        let Some(stream) = &self.runtime_event_stream else {
            return Ok(());
        };
        let mut terminal_event = debug_stream_events::flow_failed(flow_run_id, error_payload);
        terminal_event.persist_required = false;
        terminal_event.durability = RuntimeEventDurability::Ephemeral;
        match stream
            .append_terminal_if_missing_and_close(flow_run_id, terminal_event)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if is_expected_runtime_event_stream_closed_error(&error) => Err(anyhow!(
                "runtime event stream closed before recovered terminal could publish: {error}"
            )),
            Err(error) => Err(error),
        }
    }
}

fn is_stream_terminal_missing_failure(flow_run: &domain::FlowRunRecord) -> bool {
    let Some(error_payload) = flow_run.error_payload.as_ref() else {
        return false;
    };
    error_payload.get("code").and_then(Value::as_str) == Some(STREAM_TERMINAL_MISSING_ERROR_CODE)
        && error_payload.get("message").and_then(Value::as_str)
            == Some(STREAM_TERMINAL_MISSING_ERROR_MESSAGE)
}

fn stream_terminal_missing_error_payload() -> serde_json::Value {
    json!({
        "code": STREAM_TERMINAL_MISSING_ERROR_CODE,
        "message": STREAM_TERMINAL_MISSING_ERROR_MESSAGE,
    })
}
