use super::*;

use crate::ports::{
    ProviderProtocolContextLocator, ProviderProtocolContextSlotId, ProviderProtocolContextValue,
};
use orchestration_runtime::execution_engine::{CodeInvocationOutput, ConsoleLogEntry};
use plugin_framework::provider_contract::ProtocolContextEnvelope;

const REDACTED_PROTOCOL_CONTEXT_LOG: &str = "[ephemeral protocol context redacted]";

#[derive(Clone)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

struct EmbeddedProtocolContext {
    path: Vec<JsonPathSegment>,
    serialized_string: bool,
}

impl<R, H> RuntimeProviderInvoker<R, H> {
    pub(super) async fn open_protocol_context_locator_value(
        &self,
        value: &Value,
    ) -> Result<Option<Value>> {
        let Some(locator) = ProviderProtocolContextLocator::parse(value)? else {
            return Ok(None);
        };
        self.open_protocol_context(&locator).await.map(Some)
    }

    pub(super) async fn expose_protocol_contexts_to_code(
        &self,
        mut input_payload: Value,
    ) -> Result<Value> {
        let mut locators = Vec::new();
        collect_locator_paths(&input_payload, &mut Vec::new(), &mut locators)?;
        for (path, locator) in locators {
            let raw_value = self.open_protocol_context(&locator).await?;
            let target = value_at_path_mut(&mut input_payload, &path)
                .ok_or_else(|| anyhow!("ephemeral_protocol_context_locator_invalid"))?;
            *target = raw_value;
        }
        Ok(input_payload)
    }

    pub(super) async fn protect_code_protocol_context_output(
        &self,
        output: &mut CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        for path in selected_output_paths {
            let path = path
                .iter()
                .cloned()
                .map(JsonPathSegment::Key)
                .collect::<Vec<_>>();
            let Some(value) = value_at_path(&output.output_payload, &path).cloned() else {
                continue;
            };
            if value.is_null() || ProviderProtocolContextLocator::parse(&value)?.is_some() {
                continue;
            }
            let locator = self.seal_protocol_context_value(value).await?;
            let target = value_at_path_mut(&mut output.output_payload, &path)
                .ok_or_else(|| anyhow!("ephemeral_protocol_context_locator_invalid"))?;
            *target = locator;
        }

        seal_embedded_protocol_contexts(self, &mut output.output_payload).await?;
        for log in &mut output.console_logs {
            protect_console_log(self, log).await?;
        }
        Ok(())
    }

    pub(super) async fn protect_code_console_logs(
        &self,
        console_logs: &mut Vec<ConsoleLogEntry>,
    ) -> Result<()> {
        for log in console_logs {
            protect_console_log(self, log).await?;
        }
        Ok(())
    }

    async fn open_protocol_context(
        &self,
        locator: &ProviderProtocolContextLocator,
    ) -> Result<Value> {
        let flow_run_id = self
            .flow_run_id
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_missing"))?;
        let store = self
            .provider_transport_store
            .as_ref()
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_missing"))?;
        let value = store
            .get_protocol_context(ProviderProtocolContextSlotId::for_locator(
                flow_run_id,
                locator,
            ))
            .await
            .map_err(|_| anyhow!("ephemeral_protocol_context_unavailable"))?
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_missing"))?;
        anyhow::ensure!(
            value.matches_locator(locator),
            "ephemeral_protocol_context_integrity_mismatch"
        );
        Ok(value.into_value())
    }

    async fn seal_protocol_context_value(&self, value: Value) -> Result<Value> {
        let flow_run_id = self
            .flow_run_id
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_store_unavailable"))?;
        let store = self
            .provider_transport_store
            .as_ref()
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_store_unavailable"))?;
        let sealed = ProviderProtocolContextValue::new(value)
            .map_err(|_| anyhow!("ephemeral_protocol_context_sealing_failed"))?;
        let locator = sealed.derived_locator();
        store
            .put_protocol_context(
                ProviderProtocolContextSlotId::for_locator(flow_run_id, &locator),
                sealed,
            )
            .await
            .map_err(|_| anyhow!("ephemeral_protocol_context_unavailable"))?;
        Ok(locator.as_value())
    }
}

async fn protect_console_log<R, H>(
    invoker: &RuntimeProviderInvoker<R, H>,
    log: &mut ConsoleLogEntry,
) -> Result<()> {
    let mut protected = false;
    for argument in &mut log.args {
        protected |= seal_embedded_protocol_contexts(invoker, argument).await?;
        if argument
            .as_str()
            .is_some_and(looks_like_protocol_context_text)
        {
            *argument = Value::String(REDACTED_PROTOCOL_CONTEXT_LOG.to_string());
            protected = true;
        }
    }
    if protected || looks_like_protocol_context_text(&log.message) {
        log.message = REDACTED_PROTOCOL_CONTEXT_LOG.to_string();
    }
    Ok(())
}

async fn seal_embedded_protocol_contexts<R, H>(
    invoker: &RuntimeProviderInvoker<R, H>,
    value: &mut Value,
) -> Result<bool> {
    let mut contexts = Vec::new();
    collect_embedded_protocol_contexts(value, &mut Vec::new(), &mut contexts);
    let protected = !contexts.is_empty();
    for context in contexts {
        let raw_value = value_at_path(value, &context.path)
            .cloned()
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_locator_invalid"))?;
        let raw_value = if context.serialized_string {
            serde_json::from_str(
                raw_value
                    .as_str()
                    .ok_or_else(|| anyhow!("ephemeral_protocol_context_locator_invalid"))?,
            )
            .map_err(|_| anyhow!("ephemeral_protocol_context_sealing_failed"))?
        } else {
            raw_value
        };
        let locator = invoker.seal_protocol_context_value(raw_value).await?;
        let replacement = if context.serialized_string {
            Value::String(locator.to_string())
        } else {
            locator
        };
        *value_at_path_mut(value, &context.path)
            .ok_or_else(|| anyhow!("ephemeral_protocol_context_locator_invalid"))? = replacement;
    }
    Ok(protected)
}

fn collect_locator_paths(
    value: &Value,
    path: &mut Vec<JsonPathSegment>,
    locators: &mut Vec<(Vec<JsonPathSegment>, ProviderProtocolContextLocator)>,
) -> Result<()> {
    if let Some(locator) = ProviderProtocolContextLocator::parse(value)? {
        locators.push((path.clone(), locator));
        return Ok(());
    }
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                path.push(JsonPathSegment::Key(key.clone()));
                collect_locator_paths(child, path, locators)?;
                path.pop();
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                path.push(JsonPathSegment::Index(index));
                collect_locator_paths(child, path, locators)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_embedded_protocol_contexts(
    value: &Value,
    path: &mut Vec<JsonPathSegment>,
    contexts: &mut Vec<EmbeddedProtocolContext>,
) {
    if serde_json::from_value::<ProtocolContextEnvelope>(value.clone()).is_ok() {
        contexts.push(EmbeddedProtocolContext {
            path: path.clone(),
            serialized_string: false,
        });
        return;
    }
    if value
        .as_str()
        .is_some_and(|text| serde_json::from_str::<ProtocolContextEnvelope>(text).is_ok())
    {
        contexts.push(EmbeddedProtocolContext {
            path: path.clone(),
            serialized_string: true,
        });
        return;
    }
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                path.push(JsonPathSegment::Key(key.clone()));
                collect_embedded_protocol_contexts(child, path, contexts);
                path.pop();
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                path.push(JsonPathSegment::Index(index));
                collect_embedded_protocol_contexts(child, path, contexts);
                path.pop();
            }
        }
        _ => {}
    }
}

fn value_at_path<'a>(value: &'a Value, path: &[JsonPathSegment]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = match segment {
            JsonPathSegment::Key(key) => current.as_object()?.get(key)?,
            JsonPathSegment::Index(index) => current.as_array()?.get(*index)?,
        };
    }
    Some(current)
}

fn value_at_path_mut<'a>(value: &'a mut Value, path: &[JsonPathSegment]) -> Option<&'a mut Value> {
    let mut current = value;
    for segment in path {
        current = match segment {
            JsonPathSegment::Key(key) => current.as_object_mut()?.get_mut(key)?,
            JsonPathSegment::Index(index) => current.as_array_mut()?.get_mut(*index)?,
        };
    }
    Some(current)
}

fn looks_like_protocol_context_text(value: &str) -> bool {
    value.contains("source_protocol")
        && (value.contains("headers") || value.contains("query") || value.contains("body"))
}
