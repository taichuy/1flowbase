use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use extension_contracts::provider_contract::ProviderStreamEvent;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::compiled_plan::CompiledOutput;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RawNodeExecutionResult {
    #[serde(default)]
    pub executor_output: Map<String, Value>,
    #[serde(default)]
    pub metrics_facts: Map<String, Value>,
    #[serde(default)]
    pub error_facts: Map<String, Value>,
    #[serde(default)]
    pub debug_facts: Map<String, Value>,
    #[serde(default)]
    pub provider_events: Vec<ProviderStreamEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuiltNodePayloads {
    pub output_payload: Value,
    pub metrics_payload: Value,
    pub error_payload: Value,
    pub debug_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicOutputContract {
    output_selectors: BTreeMap<String, Vec<String>>,
    allows_structured_expansion: bool,
}

impl PublicOutputContract {
    pub fn from_compiled_outputs(outputs: &[CompiledOutput]) -> Result<Self> {
        for output in outputs {
            if output.key.starts_with("__") {
                return Err(anyhow!(
                    "internal public output key `{}` cannot be declared by this node output contract",
                    output.key
                ));
            }
        }

        Ok(Self {
            output_selectors: outputs
                .iter()
                .map(|output| {
                    let selector = if output.selector.is_empty() {
                        vec![output.key.clone()]
                    } else {
                        output.selector.clone()
                    };
                    (output.key.clone(), selector)
                })
                .collect(),
            allows_structured_expansion: false,
        })
    }

    pub fn with_structured_expansion(mut self, allows_structured_expansion: bool) -> Self {
        self.allows_structured_expansion = allows_structured_expansion;
        self
    }

    pub fn allows_structured_expansion(&self) -> bool {
        self.allows_structured_expansion
    }

    pub fn contains_output_key(&self, key: &str) -> bool {
        self.output_selectors.contains_key(key)
    }

    pub fn project_variable_payload(&self, output_payload: &Value) -> Result<Value> {
        let payload = output_payload
            .as_object()
            .ok_or_else(|| anyhow!("node output payload must be an object"))?;
        let mut variable_payload = Map::new();

        for (key, selector) in &self.output_selectors {
            if let Some(value) = read_output_selector(payload, selector) {
                variable_payload.insert(key.clone(), value.clone());
            }
        }

        Ok(Value::Object(variable_payload))
    }

    pub fn build_node_payloads(&self, raw: RawNodeExecutionResult) -> Result<BuiltNodePayloads> {
        let mut output_payload = Map::new();
        let metrics_payload = raw.metrics_facts.clone();
        let error_payload = raw.error_facts.clone();
        let mut debug_payload = raw.debug_facts.clone();

        merge_output_facts(&mut output_payload, raw.executor_output)?;

        if !raw.provider_events.is_empty() {
            insert_unique(
                &mut debug_payload,
                "provider_events".to_string(),
                serde_json::to_value(raw.provider_events)?,
                "debug",
            )?;
        }

        Ok(BuiltNodePayloads {
            output_payload: Value::Object(output_payload),
            metrics_payload: Value::Object(metrics_payload),
            error_payload: Value::Object(error_payload),
            debug_payload: Value::Object(debug_payload),
        })
    }
}

fn read_output_selector<'a>(
    output_payload: &'a Map<String, Value>,
    selector: &[String],
) -> Option<&'a Value> {
    let (first, rest) = selector.split_first()?;
    let mut current = output_payload.get(first)?;

    for segment in rest {
        current = current.as_object()?.get(segment)?;
    }

    Some(current)
}

pub fn is_reserved_payload_key(key: &str) -> bool {
    key.starts_with("__")
}

fn merge_output_facts(target: &mut Map<String, Value>, facts: Map<String, Value>) -> Result<()> {
    for (key, value) in facts {
        insert_unique(target, key, value, "output")?;
    }
    Ok(())
}

fn insert_unique(
    target: &mut Map<String, Value>,
    key: String,
    value: Value,
    bucket_name: &'static str,
) -> Result<()> {
    if target.get(&key) == Some(&value) {
        return Ok(());
    }

    if target.contains_key(&key) {
        return Err(anyhow!(
            "duplicate key `{key}` in {bucket_name} payload bucket"
        ));
    }
    target.insert(key, value);
    Ok(())
}
