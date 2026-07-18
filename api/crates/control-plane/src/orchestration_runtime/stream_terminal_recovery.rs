use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{
        CommitFlowRunTerminalInput, CommitFlowRunTerminalReceipt, CommitFlowRunTerminalResult,
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

pub(super) enum TerminalCommitResolution {
    Winner(domain::FlowRunRecord),
    Loser(domain::FlowRunRecord),
}

pub(super) async fn resolve_terminal_commit<R>(
    repository: &R,
    application_id: Uuid,
    flow_run_id: Uuid,
    receipt: CommitFlowRunTerminalReceipt,
) -> Result<TerminalCommitResolution>
where
    R: OrchestrationRuntimeRepository,
{
    match receipt {
        CommitFlowRunTerminalReceipt::Winner(flow_run) => {
            Ok(TerminalCommitResolution::Winner(flow_run))
        }
        CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(flow_run) => {
            tracing::warn!(
                flow_run_id = %flow_run.id,
                application_id = %flow_run.application_id,
                "terminal commit won with a post-commit projection warning"
            );
            Ok(TerminalCommitResolution::Winner(flow_run))
        }
        CommitFlowRunTerminalReceipt::Loser => {
            let winner = repository
                .get_flow_run(application_id, flow_run_id)
                .await?
                .ok_or_else(|| anyhow!("flow run disappeared after terminal commit CAS"))?;
            Ok(TerminalCommitResolution::Loser(winner))
        }
    }
}

pub(super) fn terminal_event_from_flow_run(
    flow_run: &domain::FlowRunRecord,
) -> Option<crate::ports::RuntimeEventPayload> {
    match flow_run.status {
        domain::FlowRunStatus::Succeeded => Some(debug_stream_events::flow_finished(
            flow_run.id,
            flow_run.output_payload.clone(),
        )),
        domain::FlowRunStatus::Incomplete => Some(debug_stream_events::flow_incomplete(
            flow_run.id,
            flow_run.output_payload.clone(),
        )),
        domain::FlowRunStatus::Failed => Some(debug_stream_events::flow_failed(
            flow_run.id,
            flow_run.error_payload.clone().unwrap_or_else(
                || json!({ "message": "flow run failed without an error payload" }),
            ),
        )),
        domain::FlowRunStatus::Cancelled => Some(debug_stream_events::flow_cancelled(flow_run.id)),
        domain::FlowRunStatus::Queued
        | domain::FlowRunStatus::Running
        | domain::FlowRunStatus::WaitingCallback
        | domain::FlowRunStatus::WaitingHuman
        | domain::FlowRunStatus::Paused => None,
    }
}

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
                self.ensure_durable_terminal_projection(&flow_run).await?;
                return Ok(flow_run);
            }
            domain::FlowRunStatus::Succeeded
            | domain::FlowRunStatus::Incomplete
            | domain::FlowRunStatus::Failed
            | domain::FlowRunStatus::Cancelled => {
                self.ensure_durable_terminal_projection(&flow_run).await?;
                return Ok(flow_run);
            }
            domain::FlowRunStatus::WaitingCallback | domain::FlowRunStatus::WaitingHuman => {
                return Ok(flow_run)
            }
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
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: flow_run.id,
                expected_status,
                result: CommitFlowRunTerminalResult::Failed {
                    output_payload: flow_run.output_payload.clone(),
                    error_payload: error_payload.clone(),
                },
                flow_run_event_payload: error_payload,
                terminal_event_payload: terminal_event.payload.clone(),
                finished_at: OffsetDateTime::now_utc(),
            })
            .await?;
        let winner = match resolve_terminal_commit(
            &self.repository,
            command.application_id,
            command.flow_run_id,
            persistence_outcome,
        )
        .await?
        {
            TerminalCommitResolution::Winner(winner) | TerminalCommitResolution::Loser(winner) => {
                winner
            }
        };
        self.ensure_durable_terminal_projection(&winner).await?;
        Ok(winner)
    }

    async fn ensure_durable_terminal_projection(
        &self,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<()> {
        let Some(stream) = &self.runtime_event_stream else {
            return Ok(());
        };
        let Some(mut terminal_event) = terminal_event_from_flow_run(flow_run) else {
            return Ok(());
        };
        terminal_event.persist_required = false;
        terminal_event.durability = RuntimeEventDurability::Ephemeral;
        match stream
            .append_terminal_if_missing_and_close(flow_run.id, terminal_event)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if is_expected_runtime_event_stream_closed_error(&error) => Err(anyhow!(
                "runtime event stream closed before durable terminal could project: {error}"
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
