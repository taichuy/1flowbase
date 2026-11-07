//! User rules bind to declared canonical interface facts, never a free-form schema.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceEventPhase {
    Before,
    Completion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceEventTerminal {
    Completed,
    Failed,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InterfaceEventBinding {
    pub interface_id: String,
    pub phase: InterfaceEventPhase,
    /// Empty selects all terminal results; Before must keep this empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<InterfaceEventTerminal>,
}
impl InterfaceEventBinding {
    pub fn matches(
        &self,
        interface_id: &str,
        phase: InterfaceEventPhase,
        terminal: Option<InterfaceEventTerminal>,
    ) -> bool {
        self.interface_id == interface_id
            && self.phase == phase
            && (self.results.is_empty()
                || terminal.is_some_and(|value| self.results.contains(&value)))
    }
}

/// A stable envelope around explicitly declared business fields. Values are supplied per call.
pub fn interface_event_schema(phase: InterfaceEventPhase, input: Value, output: Value) -> Value {
    let base = json!({
        "interface_id":{"type":"string"}, "invocation_id":{"type":"string"},
        "phase":{"const":match phase { InterfaceEventPhase::Before=>"before", InterfaceEventPhase::Completion=>"completion" }}
    });
    let mut properties = base.as_object().cloned().expect("object literal");
    let required;
    match phase {
        InterfaceEventPhase::Before => {
            properties.insert("input".into(), input);
            required = vec!["interface_id", "invocation_id", "phase", "input"];
        }
        InterfaceEventPhase::Completion => {
            properties.insert(
                "terminal".into(),
                json!({"enum":["completed","failed","rejected","cancelled"]}),
            );
            properties.insert("output".into(), output);
            properties.insert("classification".into(), json!({"type":"string"}));
            required = vec!["interface_id", "invocation_id", "phase", "terminal"];
        }
    }
    let mut schema = json!({"type":"object","additionalProperties":false,"properties":properties,"required":required});
    if phase == InterfaceEventPhase::Completion {
        schema["allOf"] = json!([
            {"if":{"properties":{"terminal":{"const":"completed"}}},"then":{"required":["output"]}},
            {"if":{"properties":{"terminal":{"enum":["failed","rejected","cancelled"]}}},"then":{"not":{"required":["output"]}}}
        ]);
    }
    schema
}
