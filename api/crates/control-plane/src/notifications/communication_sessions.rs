//! Trusted host commands for durable sessions, continuation checkpoints and reply reservation.
use super::*;
use crate::ports::*;
use domain::*;

impl<R> NotificationService<R>
where
    R: NotificationRepository
        + PluginRepository
        + RoleConsolePolicyReader
        + CommunicationRepository,
{
    pub async fn scan_pending_admissions(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        limit: u32,
    ) -> Result<Vec<CommunicationPendingAdmission>> {
        self.repository
            .scan_communication_admissions(workspace_id, instance_id, limit)
            .await
    }
    pub async fn pending_admissions(
        &self,
        scope: &CommunicationConversationScope,
    ) -> Result<Vec<Uuid>> {
        self.repository
            .pending_communication_admissions(scope)
            .await
    }
    pub async fn admit_input(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
    ) -> Result<Option<CommunicationAdmission>> {
        self.repository
            .admit_communication_input(scope, input_id)
            .await
    }
    pub async fn exclude_input(
        &self,
        scope: &CommunicationConversationScope,
        input_id: Uuid,
    ) -> Result<bool> {
        self.repository
            .exclude_communication_input(scope, input_id)
            .await
    }
    pub async fn active_session(
        &self,
        scope: &CommunicationConversationScope,
    ) -> Result<Option<CommunicationSession>> {
        self.repository.active_communication_session(scope).await
    }
    /// The command owner validates application allow-list and caller authorization before
    /// selecting a model. This chat override does not mutate an already claimed run.
    pub async fn select_model(
        &self,
        scope: &CommunicationConversationScope,
        model: Option<&str>,
    ) -> Result<bool> {
        if model.is_some_and(|model| model.trim().is_empty()) {
            return Err(Error::InvalidInput("communication_model").into());
        }
        self.repository
            .select_communication_model(scope, model)
            .await
    }
    /// Caller must cancel and join returned executions before acknowledging /new or
    /// allowing the new session to dispatch. Receipt replay repeats no rotation.
    pub async fn rotate_session(
        &self,
        scope: &CommunicationConversationScope,
        receipt_key: &str,
    ) -> Result<Option<CommunicationSessionRotation>> {
        if receipt_key.trim().is_empty() {
            return Err(Error::InvalidInput("communication_rotation_receipt").into());
        }
        self.repository
            .rotate_communication_session(scope, receipt_key)
            .await
    }
    pub async fn claim_pending_turn(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
    ) -> Result<Option<ClaimedCommunicationTurn>> {
        self.repository
            .claim_communication_turn(workspace_id, instance_id, lease_seconds, &self.master_key)
            .await
    }
    pub async fn renew_turn_claim(
        &self,
        claim: &CommunicationTurnClaim,
        lease_seconds: u32,
    ) -> Result<bool> {
        self.repository
            .renew_communication_turn_claim(claim, lease_seconds)
            .await
    }
    pub async fn turn_inputs(
        &self,
        claim: &CommunicationTurnClaim,
    ) -> Result<Option<CommunicationTurnInputs>> {
        self.repository
            .communication_turn_inputs(claim, &self.master_key)
            .await
    }
    pub async fn record_turn_attached_file(
        &self,
        claim: &CommunicationTurnClaim,
        update: CommunicationAttachmentUpdate,
    ) -> Result<bool> {
        self.repository
            .record_communication_turn_attachment(claim, update)
            .await
    }
    /// false is a fence, never permission to dispatch another run. Reclaimed turns carry
    /// their original run_id and runtime continuation for observation/resume.
    pub async fn link_turn_run(
        &self,
        claim: &CommunicationTurnClaim,
        run_id: Uuid,
    ) -> Result<bool> {
        if run_id.is_nil() {
            return Err(Error::InvalidInput("communication_run").into());
        }
        self.repository
            .link_communication_turn_run(claim, run_id)
            .await
    }
    pub async fn checkpoint_turn(
        &self,
        claim: &CommunicationTurnClaim,
        checkpoint: CommunicationTurnCheckpoint,
    ) -> Result<bool> {
        if checkpoint.checkpoint.is_null()
            || checkpoint.consumed_watermark < 0
            || checkpoint.expected_checkpoint_revision < 0
        {
            return Err(Error::InvalidInput("communication_checkpoint").into());
        }
        self.repository
            .checkpoint_communication_turn(claim, checkpoint)
            .await
    }
    /// Reserved grants exactly one provider submission. Pending must resume this turn,
    /// preserving completed tool/node effects in the runtime-owned checkpoint.
    pub async fn seal_turn(
        &self,
        claim: &CommunicationTurnClaim,
        candidate: CommunicationTurnFinalCandidate,
    ) -> Result<CommunicationTurnSeal> {
        if candidate.final_reply.trim().is_empty() {
            return Err(Error::InvalidInput("communication_final_reply").into());
        }
        self.repository
            .seal_communication_turn(claim, candidate, &self.master_key)
            .await
    }
    pub async fn reserve_turn_send(
        &self,
        claim: &CommunicationTurnClaim,
    ) -> Result<Option<String>> {
        self.repository.reserve_communication_turn_send(claim).await
    }
    pub async fn record_turn_progress(
        &self,
        claim: &CommunicationTurnClaim,
        progress: CommunicationTurnProgress,
    ) -> Result<bool> {
        self.repository
            .record_communication_turn_progress(claim, progress)
            .await
    }
}
