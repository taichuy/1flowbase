//! User rules select documented operations; existing typed bindings remain distinct.
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
pub struct LegacyInterfaceEventBinding {
    pub interface_id: String,
    pub phase: InterfaceEventPhase,
    /// Empty selects all terminal results; Before must keep this empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<InterfaceEventTerminal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApiInterfaceEventBinding {
    /// Stable operation identifier from the shared API documentation catalog.
    pub operation_id: String,
    pub phase: InterfaceEventPhase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<InterfaceEventTerminal>,
}

/// The variants have different identities and payload contracts; they are not field aliases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum InterfaceEventBinding {
    Api(ApiInterfaceEventBinding),
    // @field-contract-compat source=canonical_interface_event.interface_id alias=InterfaceEventBinding::Legacy remove_by=2027-02-07
    Legacy(LegacyInterfaceEventBinding),
}
impl InterfaceEventBinding {
    pub fn legacy(interface_id: impl Into<String>, phase: InterfaceEventPhase) -> Self {
        Self::Legacy(LegacyInterfaceEventBinding {
            interface_id: interface_id.into(),
            phase,
            results: Vec::new(),
        })
    }

    pub fn api(operation_id: impl Into<String>, phase: InterfaceEventPhase) -> Self {
        Self::Api(ApiInterfaceEventBinding {
            operation_id: operation_id.into(),
            phase,
            results: Vec::new(),
        })
    }

    pub fn phase(&self) -> InterfaceEventPhase {
        match self {
            Self::Api(value) => value.phase,
            Self::Legacy(value) => value.phase,
        }
    }

    pub fn results(&self) -> &[InterfaceEventTerminal] {
        match self {
            Self::Api(value) => &value.results,
            Self::Legacy(value) => &value.results,
        }
    }

    pub fn with_results(mut self, results: Vec<InterfaceEventTerminal>) -> Self {
        match &mut self {
            Self::Api(value) => value.results = results,
            Self::Legacy(value) => value.results = results,
        }
        self
    }

    pub fn target_id(&self) -> &str {
        match self {
            Self::Api(value) => &value.operation_id,
            Self::Legacy(value) => &value.interface_id,
        }
    }

    pub fn operation_id(&self) -> Option<&str> {
        match self {
            Self::Api(value) => Some(&value.operation_id),
            Self::Legacy(_) => None,
        }
    }

    pub fn legacy_interface_id(&self) -> Option<&str> {
        match self {
            Self::Legacy(value) => Some(&value.interface_id),
            Self::Api(_) => None,
        }
    }

    pub fn same_target(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && self.target_id() == other.target_id()
            && self.phase() == other.phase()
    }

    fn matches_phase_result(
        &self,
        phase: InterfaceEventPhase,
        terminal: Option<InterfaceEventTerminal>,
    ) -> bool {
        self.phase() == phase
            && (self.results().is_empty()
                || terminal.is_some_and(|value| self.results().contains(&value)))
    }

    pub fn matches(
        &self,
        interface_id: &str,
        phase: InterfaceEventPhase,
        terminal: Option<InterfaceEventTerminal>,
    ) -> bool {
        self.legacy_interface_id() == Some(interface_id)
            && self.matches_phase_result(phase, terminal)
    }

    pub fn matches_operation(
        &self,
        operation_id: &str,
        phase: InterfaceEventPhase,
        terminal: Option<InterfaceEventTerminal>,
    ) -> bool {
        self.operation_id() == Some(operation_id) && self.matches_phase_result(phase, terminal)
    }
}

/// A stable envelope around explicitly declared business fields. Values are supplied per call.
pub fn interface_event_schema(phase: InterfaceEventPhase, input: Value, output: Value) -> Value {
    event_schema("interface_id", phase, input, output)
}

/// API operation envelopes deliberately identify the documented operation, not a shared handler.
pub fn api_operation_event_schema(
    phase: InterfaceEventPhase,
    input: Value,
    output: Value,
) -> Value {
    event_schema("operation_id", phase, input, output)
}

fn event_schema(
    target_key: &str,
    phase: InterfaceEventPhase,
    input: Value,
    output: Value,
) -> Value {
    let base = json!({
        "invocation_id":{"type":"string"},
        "phase":{"const":match phase { InterfaceEventPhase::Before=>"before", InterfaceEventPhase::Completion=>"completion" }}
    });
    let mut properties = base.as_object().cloned().expect("object literal");
    properties.insert(target_key.into(), json!({"type":"string"}));
    let required;
    match phase {
        InterfaceEventPhase::Before => {
            properties.insert("input".into(), input);
            required = vec![target_key, "invocation_id", "phase", "input"];
        }
        InterfaceEventPhase::Completion => {
            properties.insert(
                "terminal".into(),
                json!({"enum":["completed","failed","rejected","cancelled"]}),
            );
            properties.insert("output".into(), output);
            properties.insert("classification".into(), json!({"type":"string"}));
            required = vec![target_key, "invocation_id", "phase", "terminal"];
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
