use super::*;

#[async_trait]
impl ApplicationPublishedFlowRunRepository for PgControlPlaneStore {
    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult> {
        PgControlPlaneStore::create_published_flow_run(self, input).await
    }

    async fn create_published_flow_run_superseding_callback_predecessors(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<CreatePublishedFlowRunResult> {
        PgControlPlaneStore::create_published_flow_run_superseding_callback_predecessors(
            self, input,
        )
        .await
    }

    async fn list_local_summary_superseded_flow_run_ids(
        &self,
        successor_flow_run_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        PgControlPlaneStore::list_local_summary_superseded_flow_run_ids(self, successor_flow_run_id)
            .await
    }

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Option<Uuid>,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>> {
        PgControlPlaneStore::find_published_flow_run_by_idempotency_key(
            self,
            application_id,
            api_key_id,
            idempotency_key,
        )
        .await
    }

    async fn append_published_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        PgControlPlaneStore::append_run_event(self, input).await
    }

    async fn create_assistant_conversation(
        &self,
        input: &control_plane_contracts::application_public_runtime::CreateAssistantConversationInput,
    ) -> Result<control_plane_contracts::application_public_runtime::AssistantConversationRecord>
    {
        PgControlPlaneStore::create_assistant_conversation(self, input).await
    }

    async fn get_assistant_conversation(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<
        Option<control_plane_contracts::application_public_runtime::AssistantConversationRecord>,
    > {
        PgControlPlaneStore::get_assistant_conversation(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn list_assistant_conversations(
        &self,
        input: &control_plane_contracts::application_public_runtime::ListAssistantConversationsInput,
    ) -> Result<control_plane_contracts::application_public_runtime::AssistantConversationPage>
    {
        PgControlPlaneStore::list_assistant_conversations(self, input).await
    }

    async fn get_assistant_conversation_summary(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<
        Option<control_plane_contracts::application_public_runtime::AssistantConversationSummary>,
    > {
        PgControlPlaneStore::get_assistant_conversation_summary(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn has_active_assistant_conversation_run(&self, conversation_id: Uuid) -> Result<bool> {
        PgControlPlaneStore::has_active_assistant_conversation_run(self, conversation_id).await
    }

    async fn list_assistant_conversation_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<
        Vec<control_plane_contracts::application_public_runtime::AssistantConversationMessage>,
    > {
        PgControlPlaneStore::list_assistant_conversation_messages(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn list_assistant_conversation_native_history(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<
        Vec<
            control_plane_contracts::application_public_runtime::AssistantConversationNativeMessage,
        >,
    > {
        PgControlPlaneStore::list_assistant_conversation_native_history(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            conversation_id,
        )
        .await
    }

    async fn list_assistant_legacy_snapshot_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<
        Vec<control_plane_contracts::application_public_runtime::AssistantConversationMessage>,
    > {
        PgControlPlaneStore::list_assistant_legacy_snapshot_messages(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            flow_run_id,
        )
        .await
    }

    async fn is_assistant_run_visible(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<bool> {
        PgControlPlaneStore::is_assistant_run_visible(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            flow_run_id,
        )
        .await
    }
}
