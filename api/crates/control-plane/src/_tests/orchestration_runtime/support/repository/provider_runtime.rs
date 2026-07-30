use super::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(crate) struct InMemoryProviderRuntime {
    invoke_delay: Option<std::time::Duration>,
    provider_events: Option<Vec<ProviderStreamEvent>>,
    provider_result: Option<ProviderInvocationResult>,
    provider_results: Option<Arc<Mutex<VecDeque<ProviderInvocationResult>>>>,
    provider_outputs: Option<Arc<Mutex<VecDeque<crate::ports::ProviderRuntimeInvocationOutput>>>>,
    live_events_then_error: Option<Vec<ProviderStreamEvent>>,
    fail_before_token_models: Vec<String>,
    captured_inputs: Option<Arc<Mutex<Vec<ProviderInvocationInput>>>>,
}

impl InMemoryProviderRuntime {
    pub(crate) fn with_invoke_delay(invoke_delay: std::time::Duration) -> Self {
        Self {
            invoke_delay: Some(invoke_delay),
            provider_events: None,
            provider_result: None,
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_provider_events(provider_events: Vec<ProviderStreamEvent>) -> Self {
        Self {
            invoke_delay: None,
            provider_events: Some(provider_events),
            provider_result: None,
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_provider_result(provider_result: ProviderInvocationResult) -> Self {
        Self {
            invoke_delay: None,
            provider_events: None,
            provider_result: Some(provider_result),
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_provider_events_and_result(
        provider_events: Vec<ProviderStreamEvent>,
        provider_result: ProviderInvocationResult,
    ) -> Self {
        Self {
            invoke_delay: None,
            provider_events: Some(provider_events),
            provider_result: Some(provider_result),
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_provider_results(provider_results: Vec<ProviderInvocationResult>) -> Self {
        Self {
            invoke_delay: None,
            provider_events: None,
            provider_result: None,
            provider_results: Some(Arc::new(Mutex::new(provider_results.into()))),
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_provider_outputs(
        provider_outputs: Vec<crate::ports::ProviderRuntimeInvocationOutput>,
    ) -> Self {
        Self {
            invoke_delay: None,
            provider_events: None,
            provider_result: None,
            provider_results: None,
            provider_outputs: Some(Arc::new(Mutex::new(provider_outputs.into()))),
            live_events_then_error: None,
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_live_events_then_error(live_events: Vec<ProviderStreamEvent>) -> Self {
        Self {
            invoke_delay: None,
            provider_events: None,
            provider_result: None,
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: Some(live_events),
            fail_before_token_models: Vec::new(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_fail_before_token_models(models: Vec<&str>) -> Self {
        Self {
            invoke_delay: None,
            provider_events: None,
            provider_result: None,
            provider_results: None,
            provider_outputs: None,
            live_events_then_error: None,
            fail_before_token_models: models.into_iter().map(str::to_string).collect(),
            captured_inputs: None,
        }
    }

    pub(crate) fn with_invocation_capture() -> (Self, Arc<Mutex<Vec<ProviderInvocationInput>>>) {
        let captured_inputs = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                captured_inputs: Some(captured_inputs.clone()),
                ..Self::default()
            },
            captured_inputs,
        )
    }
}

#[async_trait]
impl ProviderRuntimePort for InMemoryProviderRuntime {
    async fn ensure_loaded(&self, _installation: &domain::PluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Value> {
        Ok(json!({ "ok": true }))
    }

    async fn list_models(
        &self,
        _installation: &domain::PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<plugin_framework::provider_contract::ProviderModelDescriptor>> {
        Ok(vec![])
    }

    async fn invoke_stream(
        &self,
        _installation: &domain::PluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        if let Some(captured_inputs) = &self.captured_inputs {
            captured_inputs
                .lock()
                .expect("provider input capture mutex should not be poisoned")
                .push(input.clone());
        }
        if self
            .fail_before_token_models
            .iter()
            .any(|model| model == &input.model)
        {
            anyhow::bail!("provider unavailable before first token");
        }
        if let Some(delay) = self.invoke_delay {
            tokio::time::sleep(delay).await;
        }

        if let Some(output) = self
            .provider_outputs
            .as_ref()
            .and_then(|provider_outputs| provider_outputs.lock().ok()?.pop_front())
        {
            return Ok(output);
        }

        let prompt = input
            .messages
            .first()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        let default_events = vec![
            ProviderStreamEvent::TextDelta {
                delta: format!("echo:{}:{}", input.model, prompt),
            },
            ProviderStreamEvent::UsageSnapshot {
                usage: plugin_framework::provider_contract::ProviderUsage {
                    input_tokens: Some(5),
                    output_tokens: Some(7),
                    total_tokens: Some(12),
                    ..plugin_framework::provider_contract::ProviderUsage::default()
                },
            },
            ProviderStreamEvent::Finish {
                reason: plugin_framework::provider_contract::ProviderFinishReason::Stop,
            },
        ];
        let default_result = plugin_framework::provider_contract::ProviderInvocationResult {
            final_content: Some(format!("echo:{}:{}", input.model, prompt)),
            usage: plugin_framework::provider_contract::ProviderUsage {
                input_tokens: Some(5),
                output_tokens: Some(7),
                total_tokens: Some(12),
                ..plugin_framework::provider_contract::ProviderUsage::default()
            },
            finish_reason: Some(plugin_framework::provider_contract::ProviderFinishReason::Stop),
            ..plugin_framework::provider_contract::ProviderInvocationResult::default()
        };
        let queued_result = self
            .provider_results
            .as_ref()
            .and_then(|provider_results| provider_results.lock().ok()?.pop_front());

        Ok(crate::ports::ProviderRuntimeInvocationOutput {
            events: self.provider_events.clone().unwrap_or(default_events),
            result: queued_result
                .or_else(|| self.provider_result.clone())
                .unwrap_or(default_result),
        })
    }

    async fn invoke_stream_with_live_events(
        &self,
        installation: &domain::PluginInstallationRecord,
        input: ProviderInvocationInput,
        live_events: Option<crate::ports::ProviderLiveEventSenders>,
    ) -> Result<crate::ports::ProviderRuntimeInvocationOutput> {
        if let Some(events) = &self.live_events_then_error {
            if let Some(live_events) = live_events {
                for event in events.iter().cloned() {
                    let sender = if matches!(event, ProviderStreamEvent::NativeEvent { .. }) {
                        &live_events.diagnostic
                    } else {
                        &live_events.required
                    };
                    sender.send(event).await.unwrap();
                }
            }
            anyhow::bail!("provider failed after live events");
        }
        let output = self.invoke_stream(installation, input).await?;
        if let Some(live_events) = live_events {
            for event in output.events.iter().cloned() {
                let sender = if matches!(event, ProviderStreamEvent::NativeEvent { .. }) {
                    &live_events.diagnostic
                } else {
                    &live_events.required
                };
                sender.send(event).await.unwrap();
            }
        }
        Ok(output)
    }
}

#[async_trait]
impl CapabilityPluginRuntimePort for InMemoryProviderRuntime {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> Result<Value> {
        Ok(json!({
            "installation_id": input.installation.id,
            "plugin_id": input.installation.plugin_id,
            "contribution_code": input.contribution_code,
            "config_payload": input.config_payload,
        }))
    }

    async fn resolve_dynamic_options(&self, input: ResolveCapabilityOptionsInput) -> Result<Value> {
        Ok(json!({
            "installation_id": input.installation.id,
            "plugin_id": input.installation.plugin_id,
            "contribution_code": input.contribution_code,
            "config_payload": input.config_payload,
        }))
    }

    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> Result<Value> {
        Ok(json!({
            "installation_id": input.installation.id,
            "plugin_id": input.installation.plugin_id,
            "contribution_code": input.contribution_code,
            "config_payload": input.config_payload,
        }))
    }

    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> Result<CapabilityExecutionOutput> {
        let answer = input
            .input_payload
            .get("query")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(CapabilityExecutionOutput {
            output_payload: json!({
                "answer": answer,
            }),
        })
    }
}
