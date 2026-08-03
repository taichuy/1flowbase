use anyhow::Result;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, NodeContributionRepository, RoleConsolePolicyReader},
};

const NODE_CONTRIBUTIONS_VIEW_OPERATION_ID: &str = "node_contributions.view";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNodeRuntimeStatus {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNodeSourceKind {
    Builtin,
    Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNodeDependencyStatus {
    NotApplicable,
    Ready,
    MissingPlugin,
    VersionMismatch,
    DisabledPlugin,
}

impl ApplicationNodeDependencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Ready => "ready",
            Self::MissingPlugin => "missing_plugin",
            Self::VersionMismatch => "version_mismatch",
            Self::DisabledPlugin => "disabled_plugin",
        }
    }
}

impl From<domain::NodeContributionDependencyStatus> for ApplicationNodeDependencyStatus {
    fn from(value: domain::NodeContributionDependencyStatus) -> Self {
        match value {
            domain::NodeContributionDependencyStatus::Ready => Self::Ready,
            domain::NodeContributionDependencyStatus::MissingPlugin => Self::MissingPlugin,
            domain::NodeContributionDependencyStatus::VersionMismatch => Self::VersionMismatch,
            domain::NodeContributionDependencyStatus::DisabledPlugin => Self::DisabledPlugin,
        }
    }
}

impl ApplicationNodeRuntimeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationNodeContractField {
    pub key: String,
    pub description: String,
    pub required: bool,
    pub value_types: Vec<String>,
    pub allowed_values: Vec<String>,
    pub applicability: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationNodeFieldContract {
    pub config_fields: Vec<ApplicationNodeContractField>,
    pub input_fields: Vec<ApplicationNodeContractField>,
    pub output_fields: Vec<ApplicationNodeContractField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationNodeCatalogEntry {
    pub source_kind: ApplicationNodeSourceKind,
    pub node_type: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub runtime_status: ApplicationNodeRuntimeStatus,
    pub runtime_status_description: String,
    pub dependency_status: ApplicationNodeDependencyStatus,
    pub field_contract: ApplicationNodeFieldContract,
    pub plugin: Option<domain::NodeContributionRegistryEntry>,
}

pub struct ListApplicationNodesQuery {
    pub actor_user_id: Uuid,
    pub application_type: domain::ApplicationType,
}

#[derive(Debug, Clone)]
pub struct ApplicationNodeCatalogView {
    pub nodes: Vec<ApplicationNodeCatalogEntry>,
}

pub struct ApplicationNodeCatalogService<R> {
    repository: R,
}

impl<R> ApplicationNodeCatalogService<R>
where
    R: AuthRepository + NodeContributionRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_application_nodes(
        &self,
        query: ListApplicationNodesQuery,
    ) -> Result<ApplicationNodeCatalogView> {
        let actor = self
            .repository
            .load_actor_context_for_user(query.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
                .await?;
            let group = domain::ConsolePolicyGroup::other("other.node-contributions")
                .expect("compiled node contribution policy group must be valid");
            let operation_id =
                domain::ConsoleOperationId::try_from(NODE_CONTRIBUTIONS_VIEW_OPERATION_ID)
                    .expect("compiled node contribution operation id must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        let mut nodes = builtin_application_nodes(query.application_type);
        nodes.extend(
            self.repository
                .list_node_contributions(actor.current_workspace_id)
                .await?
                .into_iter()
                .map(plugin_application_node),
        );

        Ok(ApplicationNodeCatalogView { nodes })
    }
}

#[derive(Clone, Copy)]
struct BuiltinNodeSpec {
    node_type: &'static str,
    title: &'static str,
    description: &'static str,
    category: &'static str,
    runtime_status: ApplicationNodeRuntimeStatus,
}

const SHARED_PROCESSING_NODES: &[BuiltinNodeSpec] = &[
    ready("llm", "LLM", "Calls a configured language model and returns generated text, usage, and optional structured output.", "generation"),
    unavailable("knowledge_retrieval", "Knowledge Retrieval", "Retrieves documents from a knowledge source. The authoring contract is reserved, but runtime execution is not implemented.", "generation"),
    unavailable("question_classifier", "Question Classifier", "Classifies input into configured branches. The authoring contract is reserved, but runtime execution is not implemented.", "control"),
    ready("if_else", "If / Else", "Evaluates ordered conditions and activates the matching branch.", "control"),
    ready("code", "Code", "Executes configured isolated code and returns structured outputs.", "data"),
    ready("template_transform", "Template Transform", "Renders a templated value from upstream variables.", "generation"),
    ready("http_request", "HTTP Request", "Calls an external HTTP endpoint and exposes its response.", "external"),
    ready("tool", "Tool", "Pauses for an external tool callback and resumes with the tool result.", "external"),
    ready("tool_result", "Tool Result", "Projects tool callback content into flow outputs.", "io"),
    ready("data_model_list", "Data Model List", "Lists records from a platform data model.", "data"),
    ready("data_model_get", "Data Model Get", "Loads one record from a platform data model.", "data"),
    ready("data_model_create", "Data Model Create", "Creates one record in a platform data model.", "data"),
    ready("data_model_update", "Data Model Update", "Updates one record in a platform data model.", "data"),
    ready("data_model_delete", "Data Model Delete", "Deletes one record from a platform data model.", "data"),
    ready("sql", "SQL", "Executes SQL against the selected native data source and returns the provider result.", "data"),
    ready("variable_assigner", "Variable Assigner", "Writes configured values into the run or conversation variable pool.", "data"),
    unavailable("parameter_extractor", "Parameter Extractor", "Extracts structured parameters from text. The authoring contract is reserved, but runtime execution is not implemented.", "data"),
    unavailable("iteration", "Iteration", "Runs a child graph for collection items. The authoring contract is reserved, but runtime execution is not implemented.", "control"),
    unavailable("loop", "Loop", "Repeats a child graph while a condition holds. The authoring contract is reserved, but runtime execution is not implemented.", "control"),
    ready("human_input", "Human Input", "Pauses execution until a user supplies the requested input.", "io"),
];

const fn ready(
    node_type: &'static str,
    title: &'static str,
    description: &'static str,
    category: &'static str,
) -> BuiltinNodeSpec {
    BuiltinNodeSpec {
        node_type,
        title,
        description,
        category,
        runtime_status: ApplicationNodeRuntimeStatus::Ready,
    }
}

const fn unavailable(
    node_type: &'static str,
    title: &'static str,
    description: &'static str,
    category: &'static str,
) -> BuiltinNodeSpec {
    BuiltinNodeSpec {
        node_type,
        title,
        description,
        category,
        runtime_status: ApplicationNodeRuntimeStatus::Unavailable,
    }
}

fn builtin_application_nodes(
    application_type: domain::ApplicationType,
) -> Vec<ApplicationNodeCatalogEntry> {
    let boundary_nodes: &[BuiltinNodeSpec] = match application_type {
        domain::ApplicationType::AgentFlow => &[
            ready("start", "Start", "Defines Agent Flow user inputs, model choices, history, files, and protocol context.", "io"),
            ready("answer", "Answer", "Returns the conversational Agent Flow response.", "io"),
        ],
        domain::ApplicationType::Workflow => &[
            ready("workflow_start", "Workflow Start", "Defines Workflow input fields consumed by extension or schedule invocation.", "io"),
            ready("workflow_end", "Workflow End", "Projects the Workflow return field contract.", "io"),
        ],
    };

    boundary_nodes
        .iter()
        .chain(SHARED_PROCESSING_NODES)
        .copied()
        .map(builtin_application_node)
        .collect()
}

fn builtin_application_node(spec: BuiltinNodeSpec) -> ApplicationNodeCatalogEntry {
    ApplicationNodeCatalogEntry {
        source_kind: ApplicationNodeSourceKind::Builtin,
        node_type: spec.node_type.to_string(),
        title: spec.title.to_string(),
        description: spec.description.to_string(),
        category: spec.category.to_string(),
        runtime_status: spec.runtime_status,
        runtime_status_description: match spec.runtime_status {
            ApplicationNodeRuntimeStatus::Ready => {
                "Executable by the current orchestration runtime.".to_string()
            }
            ApplicationNodeRuntimeStatus::Unavailable => {
                "Known authoring contract; current orchestration runtime does not execute this node type."
                    .to_string()
            }
        },
        dependency_status: ApplicationNodeDependencyStatus::NotApplicable,
        field_contract: builtin_field_contract(spec.node_type, spec.title),
        plugin: None,
    }
}

fn plugin_application_node(
    contribution: domain::NodeContributionRegistryEntry,
) -> ApplicationNodeCatalogEntry {
    let runtime_status =
        if contribution.dependency_status == domain::NodeContributionDependencyStatus::Ready {
            ApplicationNodeRuntimeStatus::Ready
        } else {
            ApplicationNodeRuntimeStatus::Unavailable
        };
    let dependency_status = ApplicationNodeDependencyStatus::from(contribution.dependency_status);
    let title = contribution.title.clone();
    let description = contribution.description.clone();
    let category = contribution.category.clone();

    ApplicationNodeCatalogEntry {
        source_kind: ApplicationNodeSourceKind::Plugin,
        node_type: "plugin_node".to_string(),
        title,
        description,
        category,
        runtime_status,
        runtime_status_description: if runtime_status == ApplicationNodeRuntimeStatus::Ready {
            "Executable by the assigned workspace capability plugin.".to_string()
        } else {
            format!(
                "Plugin contribution is unavailable because dependency status is {}.",
                dependency_status.as_str()
            )
        },
        dependency_status,
        field_contract: ApplicationNodeFieldContract {
            config_fields: vec![field(
                "plugin.schema_ui",
                "Plugin-owned configuration form contract. Read the nested plugin.schema_ui object for exact fields and validation.",
                true,
                &["object"],
                &[],
                None,
            )],
            input_fields: vec![field(
                "bindings.*",
                "Bindings accepted by the plugin node configuration contract.",
                false,
                &["binding"],
                &[],
                Some("Keys are defined by plugin.schema_ui."),
            )],
            output_fields: vec![field(
                "plugin.output_schema_snapshot.outputs[]",
                "Immutable published output contract for the plugin contribution.",
                true,
                &["array"],
                &[],
                None,
            )],
        },
        plugin: Some(contribution),
    }
}

fn builtin_field_contract(node_type: &str, title: &str) -> ApplicationNodeFieldContract {
    match node_type {
        "start" => ApplicationNodeFieldContract {
            config_fields: vec![
                field("config.input_fields", "Agent Flow input field definitions persisted in the flow document.", true, &["array"], &[], None),
                field("config.model_list", "Models that callers may select for this Agent Flow.", true, &["array"], &[], None),
            ],
            input_fields: vec![
                field("query", "Primary conversational user input.", false, &["string"], &[], None),
                field("inputs", "Additional named Agent Flow input values.", false, &["object"], &[], None),
                field("history", "Prior conversation messages.", false, &["array"], &[], None),
                field("files", "File descriptors supplied to the run.", false, &["array"], &[], None),
                field("model", "Requested model identifier from config.model_list.", false, &["string"], &[], None),
                field("protocol_context", "AI Gateway protocol and operation context injected by the backend.", false, &["object"], &[], None),
            ],
            output_fields: Vec::new(),
        },
        "workflow_start" => workflow_start_field_contract(),
        "workflow_end" => ApplicationNodeFieldContract {
            config_fields: vec![field("config.output_contract", "Workflow return field definitions.", false, &["array"], &[], None)],
            input_fields: vec![field("bindings.*", "Values projected into Workflow return fields.", false, &["binding"], &[], None)],
            output_fields: vec![field("outputs[]", "Returned fields using key, title, valueType, and selector.", true, &["array"], &[], None)],
        },
        "answer" => ApplicationNodeFieldContract {
            config_fields: Vec::new(),
            input_fields: vec![field("bindings.answer_template", "Templated conversational response.", true, &["templated_text"], &[], None)],
            output_fields: vec![field("answer", "Final Agent Flow response text.", true, &["string"], &[], None)],
        },
        "llm" => ApplicationNodeFieldContract {
            config_fields: vec![
                field("config.model_provider.provider_code", "Configured model provider family.", true, &["string"], &[], None),
                field("config.model_provider.model_id", "Provider model identifier.", true, &["string"], &[], None),
                field("config.llm_parameters", "Provider-supported generation parameters.", false, &["object"], &[], None),
                field("config.response_format.mode", "Model output mode.", false, &["string"], &["text", "json_schema"], None),
            ],
            input_fields: vec![field("bindings.prompt_messages", "Ordered system, user, and assistant prompt messages with templated content.", true, &["prompt_messages"], &[], None)],
            output_fields: vec![
                field("text", "Generated model text.", true, &["string"], &[], None),
                field("usage", "Provider token usage metadata.", true, &["object"], &[], None),
                field("structured_output", "Validated structured result.", false, &["object"], &[], Some("Present when config.response_format.mode is json_schema.")),
            ],
        },
        "if_else" => ApplicationNodeFieldContract {
            config_fields: vec![field("config.cases", "Ordered condition groups and their branch source handles.", true, &["array"], &[], None)],
            input_fields: vec![field("bindings.*", "Values referenced by case conditions.", false, &["binding"], &[], None)],
            output_fields: vec![field("source_handle", "The selected branch handle; branch selection controls graph activation rather than producing a normal value.", true, &["string"], &[], None)],
        },
        "sql" => ApplicationNodeFieldContract {
            config_fields: vec![field("config.data_source_instance_id", "Native data source instance: main for the primary PostgreSQL source or a data-source UUID.", true, &["string"], &[], None)],
            input_fields: vec![field("bindings.sql", "SQL as a templated_text binding resolved from upstream variables.", true, &["templated_text"], &[], None)],
            output_fields: vec![field("outputs[]", "Caller-declared projection of the native SQL result.", false, &["array"], &[], None)],
        },
        _ => ApplicationNodeFieldContract {
            config_fields: vec![field("config", &format!("{title} node configuration object."), true, &["object"], &[], None)],
            input_fields: vec![field("bindings.*", "Named bindings resolved from literals, templates, selectors, or node-specific binding kinds.", false, &["binding"], &[], None)],
            output_fields: vec![field("outputs[]", "Declared node outputs using key, title, valueType, and selector where applicable.", false, &["array"], &[], None)],
        },
    }
}

fn workflow_start_field_contract() -> ApplicationNodeFieldContract {
    ApplicationNodeFieldContract {
        config_fields: vec![
            field("config.input_fields", "Workflow input field definitions. Extension requests read them by source; schedules provide keyed defaults.", true, &["array"], &[], None),
            field("config.input_fields[].key", "Stable input key used in the request and Workflow Start variable namespace; keys must be non-empty and unique.", true, &["string"], &[], None),
            field("config.input_fields[].label", "Human-facing field label; it does not rename the runtime key.", true, &["string"], &[], None),
            field("config.input_fields[].inputType", "Authoring control type. It determines valueType and which optional properties apply.", true, &["string"], &["text", "paragraph", "select", "number", "checkbox", "file", "file_list", "url"], None),
            field("config.input_fields[].valueType", "Persisted runtime value type: text, paragraph, select, and url use string; number uses number; checkbox uses boolean; file uses json; file_list uses array[object].", true, &["string"], &["string", "number", "boolean", "json", "array[object]"], None),
            field("config.input_fields[].required", "When true, invocation must supply a value unless defaultValue is present. Path fields are always required by the published route.", true, &["boolean"], &[], None),
            field("config.input_fields[].placeholder", "Optional authoring hint for text-like controls; it is not runtime input.", false, &["string"], &[], Some("Applies to text, paragraph, number, and url controls.")),
            field("config.input_fields[].defaultValue", "Optional fallback whose JSON type must match valueType; file and file_list do not accept defaults.", false, &["string", "number", "boolean"], &[], Some("Applies to text, paragraph, select, number, checkbox, and url.")),
            field("config.input_fields[].maxLength", "Optional positive integer length limit for text-like authoring controls.", false, &["integer"], &[], Some("Applies to text, paragraph, and url.")),
            field("config.input_fields[].hidden", "Whether the authoring or generated input UI hides the field; the runtime key remains part of the contract.", false, &["boolean"], &[], None),
            field("config.input_fields[].options", "Allowed string choices for a select control.", false, &["array[string]"], &[], Some("Applies only when inputType is select.")),
            field("config.input_fields[].source", "Extension request location. form is an input source alongside path, query, and body; it is not a Workflow trigger. Schedule defaults ignore HTTP source.", true, &["string"], &["path", "query", "body", "form"], Some("Required for extension publication; ignored when schedule defaults are projected.")),
            field("config.sync_timeout_ms", "Maximum synchronous extension wait before the API returns an accepted run.", true, &["integer"], &[], None),
        ],
        input_fields: vec![field("<workflow_start_node_id>.<key>", "Runtime input variables produced from the configured input_fields contract.", false, &["string", "number", "boolean", "object", "array"], &[], None)],
        output_fields: Vec::new(),
    }
}

fn field(
    key: &str,
    description: &str,
    required: bool,
    value_types: &[&str],
    allowed_values: &[&str],
    applicability: Option<&str>,
) -> ApplicationNodeContractField {
    ApplicationNodeContractField {
        key: key.to_string(),
        description: description.to_string(),
        required,
        value_types: value_types
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        allowed_values: allowed_values
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        applicability: applicability.map(str::to_string),
    }
}
