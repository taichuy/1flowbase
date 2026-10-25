//! Channel configuration/history and host worker state transitions. No SQL or provider I/O.
use super::*;
use crate::ports::{
    ClaimedCommunicationInput, CommitCommunicationReceiveInput, CommunicationAttachmentUpdate,
    CommunicationClaim, CommunicationProgress, CommunicationRepository,
};
use domain::{
    CommunicationAttachment, CommunicationBindingInput, CommunicationBindingRecord,
    CommunicationConversation, CommunicationHistoryScope, CommunicationMessage, CommunicationPage,
};

impl<R> NotificationService<R>
where
    R: NotificationRepository
        + PluginRepository
        + RoleConsolePolicyReader
        + CommunicationRepository,
{
    pub async fn management_conversations(
        &self,
        actor: &ActorContext,
        query: &domain::CommunicationManagementQuery,
    ) -> Result<domain::CommunicationManagementPage<domain::CommunicationConversationListItem>>
    {
        self.manage(actor, "communications.management.conversations.list")
            .await?;
        self.repository
            .communication_management_conversations(actor.current_workspace_id, query)
            .await
    }
    pub async fn management_messages(
        &self,
        actor: &ActorContext,
        query: &domain::CommunicationManagementQuery,
    ) -> Result<domain::CommunicationManagementPage<domain::CommunicationMessageListItem>> {
        self.manage(actor, "communications.management.messages.list")
            .await?;
        self.repository
            .communication_management_messages(actor.current_workspace_id, query)
            .await
    }
    pub async fn management_message(
        &self,
        actor: &ActorContext,
        message_id: Uuid,
    ) -> Result<domain::CommunicationMessageDetail> {
        self.manage(actor, "communications.management.messages.get")
            .await?;
        self.repository
            .communication_management_message(actor.current_workspace_id, message_id)
            .await?
            .ok_or_else(|| Error::NotFound("communication_message").into())
    }
    pub async fn get_binding(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
    ) -> Result<Option<CommunicationBindingRecord>> {
        self.manage(actor, "communications.binding.get").await?;
        self.repository
            .communication_binding(actor.current_workspace_id, instance_id)
            .await
    }
    /// Host supplies its provisioned plugin MCP ID; caller JSON cannot set execution identity.
    /// Application/model/MCP/file access are revalidated by the execution owner for every run.
    pub async fn save_binding(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        mut input: CommunicationBindingInput,
        plugin_mcp_instance_id: String,
    ) -> Result<CommunicationBindingRecord> {
        self.manage(actor, "communications.binding.update").await?;
        let instance = self.record(actor, instance_id).await?;
        if instance.type_code == "system" {
            return Err(Error::InvalidInput("communication_binding_unsupported").into());
        }
        if plugin_mcp_instance_id.trim().is_empty()
            || input.application_id.is_nil()
            || input.mcp_instance_ids.iter().any(|v| v.trim().is_empty())
        {
            return Err(Error::InvalidInput("communication_binding").into());
        }
        let prior = self
            .repository
            .communication_binding(actor.current_workspace_id, instance_id)
            .await?;
        if prior.is_none() && !input.mcp_instance_ids.contains(&plugin_mcp_instance_id) {
            input
                .mcp_instance_ids
                .insert(0, plugin_mcp_instance_id.clone());
        }
        let mut seen = BTreeSet::new();
        input.mcp_instance_ids.retain(|id| seen.insert(id.clone()));
        let actor_user_id = prior.map_or(actor.user_id, |binding| binding.actor_user_id);
        self.repository
            .save_communication_binding(CommunicationBindingRecord {
                workspace_id: actor.current_workspace_id,
                instance_id,
                actor_user_id,
                config: input,
                plugin_mcp_instance_id,
                revision: 0,
            })
            .await
    }
    pub async fn conversations(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationConversation>> {
        self.manage(actor, "communications.conversations.list")
            .await?;
        self.repository
            .communication_conversations(
                actor.current_workspace_id,
                instance_id,
                None,
                cursor,
                limit,
            )
            .await
    }
    async fn conversation_scope(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<CommunicationHistoryScope> {
        let c = self
            .repository
            .communication_conversation(actor.current_workspace_id, instance_id, conversation_id)
            .await?
            .ok_or(Error::NotFound("communication_conversation"))?;
        Ok(CommunicationHistoryScope {
            workspace_id: c.workspace_id,
            instance_id: c.instance_id,
            external_identity: c.external_identity,
        })
    }
    pub async fn message_history(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationMessage>> {
        self.manage(actor, "communications.messages.list").await?;
        let scope = self
            .conversation_scope(actor, instance_id, conversation_id)
            .await?;
        self.scoped_message_history(&scope, conversation_id, cursor, limit)
            .await
    }
    pub async fn files(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationAttachment>> {
        self.manage(actor, "communications.files.list").await?;
        let scope = self
            .conversation_scope(actor, instance_id, conversation_id)
            .await?;
        self.scoped_files(&scope, conversation_id, cursor, limit)
            .await
    }
    pub async fn file(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        attachment_id: Uuid,
    ) -> Result<CommunicationAttachment> {
        self.manage(actor, "communications.files.get").await?;
        self.repository
            .communication_file(actor.current_workspace_id, instance_id, None, attachment_id)
            .await?
            .ok_or_else(|| Error::NotFound("communication_attachment").into())
    }
    /// Only the host may construct scope from its authenticated execution. No model filter is authority.
    pub async fn scoped_conversations(
        &self,
        scope: &CommunicationHistoryScope,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationConversation>> {
        self.repository
            .communication_conversations(
                scope.workspace_id,
                scope.instance_id,
                Some(&scope.external_identity),
                cursor,
                limit,
            )
            .await
    }
    pub async fn scoped_message_history(
        &self,
        scope: &CommunicationHistoryScope,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationMessage>> {
        self.repository
            .communication_messages(scope, conversation_id, cursor, limit)
            .await
    }
    pub async fn scoped_files(
        &self,
        scope: &CommunicationHistoryScope,
        conversation_id: Uuid,
        cursor: Option<Uuid>,
        limit: u32,
    ) -> Result<CommunicationPage<CommunicationAttachment>> {
        self.repository
            .communication_files(scope, conversation_id, cursor, limit)
            .await
    }
    pub async fn scoped_file(
        &self,
        scope: &CommunicationHistoryScope,
        attachment_id: Uuid,
    ) -> Result<CommunicationAttachment> {
        self.repository
            .communication_file(
                scope.workspace_id,
                scope.instance_id,
                Some(&scope.external_identity),
                attachment_id,
            )
            .await?
            .ok_or_else(|| Error::NotFound("communication_attachment").into())
    }
    pub async fn retry_message(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool> {
        self.manage(actor, "communications.messages.retry").await?;
        self.repository
            .retry_communication_message(actor.current_workspace_id, instance_id, message_id)
            .await
    }
    pub async fn enabled_bound_channels(&self) -> Result<Vec<CommunicationBindingRecord>> {
        self.repository.enabled_communication_bindings().await
    }
    pub async fn receive_cursor(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<Value>> {
        self.repository
            .communication_receive_cursor(workspace_id, instance_id, &self.master_key)
            .await
    }
    pub async fn commit_receive(
        &self,
        mut input: CommitCommunicationReceiveInput,
    ) -> Result<Option<i64>> {
        input.authorization.master_key = self.master_key.clone();
        self.repository.commit_communication_receive(input).await
    }
    pub async fn claim_pending_input(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        lease_seconds: u32,
    ) -> Result<Option<ClaimedCommunicationInput>> {
        self.repository
            .claim_communication_input(workspace_id, instance_id, lease_seconds, &self.master_key)
            .await
    }
    pub async fn renew_claim(
        &self,
        claim: &CommunicationClaim,
        lease_seconds: u32,
    ) -> Result<bool> {
        self.repository
            .renew_communication_claim(claim, lease_seconds)
            .await
    }
    pub async fn record_attached_file(
        &self,
        claim: &CommunicationClaim,
        update: CommunicationAttachmentUpdate,
    ) -> Result<bool> {
        self.repository
            .record_communication_attachment(claim, update)
            .await
    }
    /// Reserve a run ID durably before starting execution, and mark send-start before provider I/O.
    /// false means stale/expired claim or an invalid state transition: host must stop processing.
    pub async fn record_progress(
        &self,
        claim: &CommunicationClaim,
        progress: CommunicationProgress,
    ) -> Result<bool> {
        self.repository
            .record_communication_progress(claim, progress)
            .await
    }
}
