//! Communication adapter and final-success gate. File authorization/import stays in the host.
use super::*;
use crate::{
    notifications::NotificationService,
    ports::{
        CommunicationRepository, CommunicationTurnCheckpoint, CommunicationTurnClaim,
        CommunicationTurnFinalCandidate, CommunicationTurnInput, CommunicationTurnSeal,
        NotificationRepository, RoleConsolePolicyReader,
    },
};
use orchestration_runtime::execution_engine::communication::{
    CommunicationExecution, CommunicationExecutionActivity, CommunicationExecutionInput,
    CommunicationExecutionInputs, CommunicationExecutionLease, CommunicationExecutionPort,
    CommunicationExecutionSeal,
};

#[derive(Clone)]
pub struct CommunicationExecutionBinding {
    pub port: Arc<dyn CommunicationExecutionPort>,
    pub initial_prompt_watermark: i64,
}

/// Shared by freshly constructed initial, callback and human-resume services. The host
/// retains/reclaims the durable turn claim; a missing registered owner must not dispatch.
#[async_trait]
pub trait CommunicationExecutionResolver: Send + Sync {
    async fn resolve(
        &self,
        run: &domain::FlowRunRecord,
    ) -> Result<Option<CommunicationExecutionBinding>>;
}

#[async_trait]
pub trait CommunicationInputPreparer: Send + Sync {
    /// Preserve IDs/order. Import pending files through the existing file owner and P01's
    /// record_turn_attached_file before returning only safe user messages to the runtime.
    async fn prepare_inputs(
        &self,
        claim: &CommunicationTurnClaim,
        inputs: Vec<CommunicationTurnInput>,
    ) -> Result<Vec<CommunicationExecutionInput>>;
}

#[derive(Clone, Debug)]
pub struct CommunicationReservedReply {
    pub turn_id: Uuid,
    pub outbound_message_id: Uuid,
    pub run_id: Uuid,
    pub final_reply: String,
}

#[derive(Default)]
struct BridgeState {
    checkpoint_revision: Option<i64>,
    received_watermark: i64,
    consumed_watermark: i64,
    reserved: Option<CommunicationReservedReply>,
}

pub struct CommunicationTurnExecutionBridge<R> {
    service: NotificationService<R>,
    claim: CommunicationTurnClaim,
    preparer: Arc<dyn CommunicationInputPreparer>,
    state: tokio::sync::Mutex<BridgeState>,
    activity: Option<Arc<dyn CommunicationExecutionActivity>>,
}

impl<R> CommunicationTurnExecutionBridge<R> {
    pub fn new(
        service: NotificationService<R>,
        claim: CommunicationTurnClaim,
        preparer: Arc<dyn CommunicationInputPreparer>,
    ) -> Self {
        Self {
            service,
            claim,
            preparer,
            state: tokio::sync::Mutex::new(BridgeState::default()),
            activity: None,
        }
    }

    pub fn with_activity(mut self, activity: Arc<dyn CommunicationExecutionActivity>) -> Self {
        self.activity = Some(activity);
        self
    }
}

impl<R> CommunicationTurnExecutionBridge<R>
where
    R: NotificationRepository
        + PluginRepository
        + RoleConsolePolicyReader
        + CommunicationRepository
        + Send
        + Sync,
{
    /// Only the dispatch owner takes this receipt, after the same run persisted success.
    /// Recheck rotation/lease fencing; the host also retains cancellation ownership around
    /// provider submission and records its terminal progress through the original claim.
    pub async fn take_reserved_reply(&self) -> Result<Option<CommunicationReservedReply>> {
        self.check_active().await?;
        Ok(self.state.lock().await.reserved.take())
    }
}

#[async_trait]
impl<R> CommunicationExecutionPort for CommunicationTurnExecutionBridge<R>
where
    R: NotificationRepository
        + PluginRepository
        + RoleConsolePolicyReader
        + CommunicationRepository
        + Send
        + Sync,
{
    async fn enter_execution(&self) -> Result<Box<dyn CommunicationExecutionLease>> {
        match &self.activity {
            Some(activity) => {
                activity
                    .enter(self.claim.session_id, self.claim.session_epoch)
                    .await
            }
            None => {
                self.check_active().await?;
                Ok(Box::new(()))
            }
        }
    }

    async fn cancelled(&self) {
        if let Some(activity) = &self.activity {
            activity
                .cancelled(self.claim.session_id, self.claim.session_epoch)
                .await;
        } else {
            loop {
                if self.check_active().await.is_err() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
    async fn inspect(&self) -> Result<CommunicationExecutionInputs> {
        let mut state = self.state.lock().await;
        let pending = self
            .service
            .turn_inputs(&self.claim)
            .await?
            .ok_or_else(|| anyhow!("communication_turn_fenced"))?;
        if state
            .checkpoint_revision
            .is_some_and(|revision| revision != pending.turn.checkpoint_revision)
        {
            return Err(anyhow!("communication_checkpoint_owner_changed"));
        }
        let identities = pending
            .inputs
            .iter()
            .map(|input| (input.input_id, input.input_sequence))
            .collect::<Vec<_>>();
        let inputs = self
            .preparer
            .prepare_inputs(&self.claim, pending.inputs)
            .await?;
        if inputs
            .iter()
            .map(|input| (input.input_id, input.input_sequence))
            .collect::<Vec<_>>()
            != identities
        {
            return Err(anyhow!("communication_prepared_input_membership_changed"));
        }
        state.checkpoint_revision = Some(pending.turn.checkpoint_revision);
        state.received_watermark = pending.turn.received_watermark;
        state.consumed_watermark = pending.turn.consumed_watermark;
        Ok(CommunicationExecutionInputs {
            received_watermark: pending.turn.received_watermark,
            consumed_watermark: pending.turn.consumed_watermark,
            checkpoint: pending.checkpoint,
            inputs,
            pending_admission: !pending.pending_admission_input_ids.is_empty(),
        })
    }

    async fn checkpoint(&self, consumed_watermark: i64, continuation: Value) -> Result<()> {
        let mut state = self.state.lock().await;
        let revision = state
            .checkpoint_revision
            .ok_or_else(|| anyhow!("communication_checkpoint_before_input_inspection"))?;
        if !self
            .service
            .checkpoint_turn(
                &self.claim,
                CommunicationTurnCheckpoint {
                    expected_checkpoint_revision: revision,
                    consumed_watermark,
                    checkpoint: continuation,
                },
            )
            .await?
        {
            return Err(anyhow!("communication_checkpoint_fenced"));
        }
        state.checkpoint_revision = Some(revision + 1);
        state.consumed_watermark = consumed_watermark;
        Ok(())
    }

    async fn check_active(&self) -> Result<()> {
        if !self.service.renew_turn_claim(&self.claim, 60).await? {
            return Err(anyhow!("communication_turn_fenced"));
        }
        Ok(())
    }

    async fn seal(&self, final_reply: &str) -> Result<CommunicationExecutionSeal> {
        let mut state = self.state.lock().await;
        let revision = state
            .checkpoint_revision
            .ok_or_else(|| anyhow!("communication_seal_without_checkpoint"))?;
        match self
            .service
            .seal_turn(
                &self.claim,
                CommunicationTurnFinalCandidate {
                    expected_received_watermark: state.received_watermark,
                    expected_checkpoint_revision: revision,
                    final_reply: final_reply.to_owned(),
                },
            )
            .await?
        {
            CommunicationTurnSeal::Reserved { turn, final_reply } => {
                state.reserved = Some(CommunicationReservedReply {
                    turn_id: turn.id,
                    outbound_message_id: turn
                        .outbound_message_id
                        .ok_or_else(|| anyhow!("communication_outbound_receipt_missing"))?,
                    run_id: turn
                        .run_id
                        .ok_or_else(|| anyhow!("communication_run_receipt_missing"))?,
                    final_reply,
                });
                Ok(CommunicationExecutionSeal::Reserved)
            }
            CommunicationTurnSeal::Pending(pending) => {
                if pending.turn.checkpoint_revision != revision {
                    return Err(anyhow!("communication_checkpoint_owner_changed"));
                }
                state.received_watermark = pending.turn.received_watermark;
                Ok(CommunicationExecutionSeal::Pending)
            }
            CommunicationTurnSeal::Fenced => Err(anyhow!("communication_turn_fenced")),
        }
    }
}

impl<R, H> OrchestrationRuntimeService<R, H> {
    pub fn with_communication_execution_resolver(
        mut self,
        resolver: Arc<dyn CommunicationExecutionResolver>,
    ) -> Self {
        self.communication_execution_resolver = Some(resolver);
        self
    }
    pub fn with_communication_execution(
        mut self,
        flow_run_id: Uuid,
        port: Arc<dyn CommunicationExecutionPort>,
        initial_prompt_watermark: i64,
    ) -> Result<Self> {
        self.communication_execution = Some((
            flow_run_id,
            Arc::new(CommunicationExecution::new(port, initial_prompt_watermark)?),
        ));
        Ok(self)
    }

    pub(super) async fn attach_communication_execution(
        &self,
        run: &domain::FlowRunRecord,
        context: orchestration_runtime::execution_engine::ExecutionRuntimeContext,
    ) -> Result<orchestration_runtime::execution_engine::ExecutionRuntimeContext> {
        let execution = if let Some((id, execution)) = &self.communication_execution {
            if *id != run.id {
                return Err(anyhow!("communication_execution_run_scope_invalid"));
            }
            execution.clone()
        } else if let Some(resolver) = &self.communication_execution_resolver {
            if run.compatibility_mode.as_deref() != Some("communication") {
                return Ok(context);
            }
            let binding = match resolver.resolve(run).await? {
                Some(binding) => binding,
                None if run
                    .input_payload
                    .pointer("/sys/provider_session_identity")
                    .is_none() =>
                {
                    return Ok(context)
                }
                None => return Err(anyhow!("communication_execution_binding_missing")),
            };
            Arc::new(CommunicationExecution::new(
                binding.port,
                binding.initial_prompt_watermark,
            )?)
        } else {
            return Ok(context);
        };
        if run.compatibility_mode.as_deref() != Some("communication")
            || run.run_mode != domain::FlowRunMode::AssistantExecution
            || !matches!(context.operation(), domain::AiNativeOperation::Generate(_))
        {
            return Err(anyhow!("communication_execution_run_scope_invalid"));
        }
        Ok(context.with_communication_execution(execution))
    }
}

pub(super) async fn finish_communication_execution<I>(
    plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    mut outcome: orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    context: &orchestration_runtime::execution_engine::ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn orchestration_runtime::execution_engine::ExecutionLifecycle,
) -> Result<orchestration_runtime::execution_state::FlowDebugExecutionOutcome>
where
    I: orchestration_runtime::execution_engine::ProviderInvoker
        + orchestration_runtime::execution_engine::CapabilityInvoker
        + orchestration_runtime::execution_engine::CodeInvoker
        + ?Sized,
{
    use orchestration_runtime::execution_state::ExecutionStopReason;
    let Some(communication) = context.communication_execution() else {
        return Ok(outcome);
    };
    if matches!(outcome.stop_reason, ExecutionStopReason::Completed) {
        if let Err(error) = communication
            .restore_retained_traces(plan, &mut outcome)
            .await
        {
            return Ok(rejected_reply(outcome, error));
        }
    }
    while matches!(outcome.stop_reason, ExecutionStopReason::Completed) {
        if let Err(error) = communication.checkpoint_graph(&outcome).await {
            return Ok(rejected_reply(outcome, error));
        }
        let output = match super::persistence::communication_final_output(plan, &outcome) {
            Ok(output) => output,
            Err(error) => return Ok(rejected_reply(outcome, error)),
        };
        let Some(reply) = output
            .get("answer")
            .or_else(|| output.get("text"))
            .and_then(Value::as_str)
            .filter(|reply| !reply.trim().is_empty())
        else {
            return Ok(rejected_reply(
                outcome,
                anyhow!("communication_final_reply_missing"),
            ));
        };
        match communication.seal(reply).await {
            Ok(CommunicationExecutionSeal::Reserved) => return Ok(outcome),
            Ok(CommunicationExecutionSeal::Pending) => {
                let retained = outcome.clone();
                outcome = match orchestration_runtime::execution_engine::communication::continue_communication_reply(plan, outcome, context, invoker, lifecycle).await {
                    Ok(next) => next,
                    Err(error) => return Ok(rejected_reply(retained, error)),
                };
            }
            Err(error) => return Ok(rejected_reply(outcome, error)),
        }
    }
    Ok(outcome)
}

fn rejected_reply(
    mut outcome: orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    error: anyhow::Error,
) -> orchestration_runtime::execution_state::FlowDebugExecutionOutcome {
    // Carry a precise closed failure into the existing terminal CAS, including human/
    // callback resume claims. An exception must not strand a running reply after resume.
    let reason = error.to_string();
    let code = if reason.starts_with("communication_")
        && reason
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    {
        reason.as_str()
    } else {
        "communication_continuation_failed"
    };
    let payload =
        json!({"code":code,"message":"Communication reply could not complete its durable turn"});
    let (node_id, node_alias) = match outcome
        .node_traces
        .iter_mut()
        .rev()
        .find(|trace| trace.node_type == "llm")
    {
        Some(trace) => {
            trace.error_payload = Some(payload.clone());
            (trace.node_id.clone(), trace.node_alias.clone())
        }
        None => outcome
            .node_traces
            .last()
            .map(|trace| (trace.node_id.clone(), trace.node_alias.clone()))
            .unwrap_or_else(|| ("communication_reply".into(), "Communication reply".into())),
    };
    outcome.stop_reason = orchestration_runtime::execution_state::ExecutionStopReason::Failed(
        orchestration_runtime::execution_state::NodeExecutionFailure {
            node_id,
            node_alias,
            error_payload: payload,
        },
    );
    outcome
}
