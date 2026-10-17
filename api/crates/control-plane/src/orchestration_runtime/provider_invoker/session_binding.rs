use super::*;

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository + PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(in crate::orchestration_runtime) fn with_provider_session_scope(
        &self,
        run: &domain::FlowRunRecord,
    ) -> Self {
        let mut invoker = self.clone();
        let identity = run
            .input_payload
            .pointer("/sys/provider_session_identity")
            .and_then(Value::as_str)
            .filter(|identity| !identity.trim().is_empty())
            .or(run
                .external_conversation_id
                .as_deref()
                .filter(|id| !id.trim().is_empty()))
            .or_else(|| {
                run.input_payload
                    .pointer("/sys/conversation_id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
            });
        invoker.provider_session_scope = identity.map(|identity| {
            session_scope_digest(&json!([
                "provider-session/v1",
                self.workspace_id,
                run.application_id,
                run.flow_id,
                run.created_by,
                run.api_key_id,
                identity
            ]))
        });
        invoker
    }

    pub(in crate::orchestration_runtime) fn session_provider_scope(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        node_id: &str,
    ) -> Option<String> {
        let session = self.provider_session_scope.as_deref()?;
        if node_id.is_empty() {
            return None;
        }
        // Mutable rule/config revisions and candidate ordering do not change the original owner.
        let queue = runtime
            .routing
            .as_ref()
            .and_then(|routing| routing.queue_template_id.as_deref());
        let pool = if runtime.targets_main_instance() {
            json!(["main"])
        } else if let Some(queue) = queue {
            json!(["queue", queue])
        } else {
            json!(["fixed", runtime.provider_instance_id])
        };
        Some(session_scope_digest(&json!([
            session,
            node_id,
            pool,
            runtime.provider_code,
            runtime.model
        ])))
    }

    pub(super) async fn session_provider_owner(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        node_id: &str,
    ) -> Result<Option<String>> {
        let Some(scope) = self.session_provider_scope(runtime, node_id) else {
            return Ok(None);
        };
        Ok(self
            .repository
            .get_model_provider_session_binding(self.workspace_id, &scope)
            .await?
            .map(|id| id.to_string()))
    }
}

fn session_scope_digest(value: &Value) -> String {
    format!(
        "{:x}",
        base16ct::HexDisplay(&Sha256::digest(
            serde_json::to_vec(value).expect("session scope serializes")
        ))
    )
}
