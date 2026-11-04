use super::*;
use crate::ports::ApplicationCompileContextRepository;

impl<R> FlowService<R>
where
    R: ApplicationRepository + FlowRepository + ApplicationCompileContextRepository + Clone,
{
    /// Analyze a client draft without bootstrapping or changing persisted editor state.
    pub async fn analyze_document(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
        document: &Value,
    ) -> Result<domain::flow::FlowDocumentAnalysis> {
        self.analyze_document_for_node(actor_user_id, application_id, document, None)
            .await
    }

    /// Only a selected node needs a variable catalog; diagnostics cover the full graph.
    pub async fn analyze_document_for_node(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
        document: &Value,
        node_id: Option<&str>,
    ) -> Result<domain::flow::FlowDocumentAnalysis> {
        let application = ApplicationService::new(self.repository.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        self.analyze_document_in_workspace(
            application.workspace_id,
            application_id,
            application.application_type,
            document,
            node_id,
        )
        .await
    }

    /// Project an editor state returned by an already authorized command. Restoring
    /// a version has its own grant and must not acquire an extra view requirement.
    pub async fn analyze_editor_state(
        &self,
        actor_user_id: Uuid,
        state: &domain::FlowEditorState,
    ) -> Result<domain::flow::FlowDocumentAnalysis> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, state.flow.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        self.analyze_document_in_workspace(
            application.workspace_id,
            application.id,
            application.application_type,
            &state.draft.document,
            None,
        )
        .await
    }

    async fn analyze_document_in_workspace(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        application_type: domain::ApplicationType,
        document: &Value,
        node_id: Option<&str>,
    ) -> Result<domain::flow::FlowDocumentAnalysis> {
        validate_flow_draft_document(document)?;
        let context = self
            .repository
            .build_application_compile_context(workspace_id, application_id)
            .await?;
        Ok(match node_id {
            Some(node_id) => orchestration_runtime::compiler::analyze_flow_document_for_node(
                document,
                &context,
                application_type,
                node_id,
            ),
            None => orchestration_runtime::compiler::analyze_flow_document_for_application(
                document,
                &context,
                application_type,
            ),
        })
    }
}
