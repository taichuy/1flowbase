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
    pub required: bool,
    pub value_types: Vec<String>,
    pub allowed_values: Vec<String>,
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
    pub category: String,
    pub runtime_status: ApplicationNodeRuntimeStatus,
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
    category: &'static str,
    runtime_status: ApplicationNodeRuntimeStatus,
}

const SHARED_PROCESSING_NODES: &[BuiltinNodeSpec] = &[
    ready("llm", "LLM", "generation"),
    unavailable("knowledge_retrieval", "Knowledge Retrieval", "generation"),
    unavailable("question_classifier", "Question Classifier", "control"),
    ready("if_else", "If / Else", "control"),
    ready("code", "Code", "data"),
    ready("template_transform", "Template Transform", "generation"),
    ready("http_request", "HTTP Request", "external"),
    ready("tool", "Tool", "external"),
    ready("tool_result", "Tool Result", "io"),
    ready("data_model_list", "Data Model List", "data"),
    ready("data_model_get", "Data Model Get", "data"),
    ready("data_model_create", "Data Model Create", "data"),
    ready("data_model_update", "Data Model Update", "data"),
    ready("data_model_delete", "Data Model Delete", "data"),
    ready("sql", "SQL", "data"),
    ready("variable_assigner", "Variable Assigner", "data"),
    unavailable("parameter_extractor", "Parameter Extractor", "data"),
    unavailable("iteration", "Iteration", "control"),
    unavailable("loop", "Loop", "control"),
    ready("human_input", "Human Input", "io"),
];

const fn ready(
    node_type: &'static str,
    title: &'static str,
    category: &'static str,
) -> BuiltinNodeSpec {
    BuiltinNodeSpec {
        node_type,
        title,
        category,
        runtime_status: ApplicationNodeRuntimeStatus::Ready,
    }
}

const fn unavailable(
    node_type: &'static str,
    title: &'static str,
    category: &'static str,
) -> BuiltinNodeSpec {
    BuiltinNodeSpec {
        node_type,
        title,
        category,
        runtime_status: ApplicationNodeRuntimeStatus::Unavailable,
    }
}

fn builtin_application_nodes(
    application_type: domain::ApplicationType,
) -> Vec<ApplicationNodeCatalogEntry> {
    let boundary_nodes: &[BuiltinNodeSpec] = match application_type {
        domain::ApplicationType::AgentFlow => &[
            ready("start", "Start", "io"),
            ready("answer", "Answer", "io"),
        ],
        domain::ApplicationType::Workflow => &[
            ready("workflow_start", "Workflow Start", "io"),
            ready("workflow_end", "Workflow End", "io"),
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
        category: spec.category.to_string(),
        runtime_status: spec.runtime_status,
        dependency_status: ApplicationNodeDependencyStatus::NotApplicable,
        field_contract: builtin_field_contract(spec.node_type),
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
    let category = contribution.category.clone();

    ApplicationNodeCatalogEntry {
        source_kind: ApplicationNodeSourceKind::Plugin,
        node_type: "plugin_node".to_string(),
        title,
        category,
        runtime_status,
        dependency_status,
        field_contract: ApplicationNodeFieldContract {
            config_fields: vec![field("plugin.schema_ui", true, &["object"], &[])],
            input_fields: vec![field("bindings.*", false, &["binding"], &[])],
            output_fields: vec![field(
                "plugin.output_schema_snapshot.outputs[]",
                true,
                &["array"],
                &[],
            )],
        },
        plugin: Some(contribution),
    }
}

fn builtin_field_contract(node_type: &str) -> ApplicationNodeFieldContract {
    match node_type {
        "start" => ApplicationNodeFieldContract {
            config_fields: vec![
                field("config.input_fields", true, &["array"], &[]),
                field("config.model_list", true, &["array"], &[]),
            ],
            input_fields: vec![
                field("query", false, &["string"], &[]),
                field("inputs", false, &["object"], &[]),
                field("history", false, &["array"], &[]),
                field("files", false, &["array"], &[]),
                field("model", false, &["string"], &[]),
                field("protocol_context", false, &["object"], &[]),
            ],
            output_fields: Vec::new(),
        },
        "workflow_start" => workflow_start_field_contract(),
        "workflow_end" => ApplicationNodeFieldContract {
            config_fields: vec![field("config.output_contract", false, &["array"], &[])],
            input_fields: vec![field("bindings.*", false, &["binding"], &[])],
            output_fields: vec![field("outputs[]", true, &["array"], &[])],
        },
        "answer" => ApplicationNodeFieldContract {
            config_fields: Vec::new(),
            input_fields: vec![field(
                "bindings.answer_template",
                true,
                &["templated_text"],
                &[],
            )],
            output_fields: vec![field("answer", true, &["string"], &[])],
        },
        "llm" => ApplicationNodeFieldContract {
            config_fields: vec![
                field(
                    "config.model_provider.provider_code",
                    true,
                    &["string"],
                    &[],
                ),
                field("config.model_provider.model_id", true, &["string"], &[]),
                field("config.llm_parameters", false, &["object"], &[]),
                field(
                    "config.response_format.mode",
                    false,
                    &["string"],
                    &["text", "json_schema"],
                ),
            ],
            input_fields: vec![field(
                "bindings.prompt_messages",
                true,
                &["prompt_messages"],
                &[],
            )],
            output_fields: vec![
                field("text", true, &["string"], &[]),
                field("usage", true, &["object"], &[]),
                field("structured_output", false, &["object"], &[]),
            ],
        },
        "if_else" => ApplicationNodeFieldContract {
            config_fields: vec![field("config.cases", true, &["array"], &[])],
            input_fields: vec![field("bindings.*", false, &["binding"], &[])],
            output_fields: vec![field("source_handle", true, &["string"], &[])],
        },
        "sql" => ApplicationNodeFieldContract {
            config_fields: vec![field(
                "config.data_source_instance_id",
                true,
                &["string"],
                &[],
            )],
            input_fields: vec![field("bindings.sql", true, &["templated_text"], &[])],
            output_fields: vec![field("outputs[]", false, &["array"], &[])],
        },
        _ => ApplicationNodeFieldContract {
            config_fields: vec![field("config", true, &["object"], &[])],
            input_fields: vec![field("bindings.*", false, &["binding"], &[])],
            output_fields: vec![field("outputs[]", false, &["array"], &[])],
        },
    }
}

fn workflow_start_field_contract() -> ApplicationNodeFieldContract {
    ApplicationNodeFieldContract {
        config_fields: vec![
            field("config.input_fields", true, &["array"], &[]),
            field("config.input_fields[].key", true, &["string"], &[]),
            field("config.input_fields[].label", true, &["string"], &[]),
            field(
                "config.input_fields[].inputType",
                true,
                &["string"],
                &[
                    "text",
                    "paragraph",
                    "select",
                    "number",
                    "checkbox",
                    "file",
                    "file_list",
                    "url",
                ],
            ),
            field(
                "config.input_fields[].valueType",
                true,
                &["string"],
                &["string", "number", "boolean", "json", "array[object]"],
            ),
            field("config.input_fields[].required", true, &["boolean"], &[]),
            field("config.input_fields[].placeholder", false, &["string"], &[]),
            field(
                "config.input_fields[].defaultValue",
                false,
                &["string", "number", "boolean"],
                &[],
            ),
            field("config.input_fields[].maxLength", false, &["integer"], &[]),
            field("config.input_fields[].hidden", false, &["boolean"], &[]),
            field(
                "config.input_fields[].options",
                false,
                &["array[string]"],
                &[],
            ),
            field(
                "config.input_fields[].source",
                true,
                &["string"],
                &["path", "query", "body", "form"],
            ),
            field("config.sync_timeout_ms", true, &["integer"], &[]),
        ],
        input_fields: vec![field(
            "<workflow_start_node_id>.<key>",
            false,
            &["string", "number", "boolean", "object", "array"],
            &[],
        )],
        output_fields: Vec::new(),
    }
}

fn field(
    key: &str,
    required: bool,
    value_types: &[&str],
    allowed_values: &[&str],
) -> ApplicationNodeContractField {
    ApplicationNodeContractField {
        key: key.to_string(),
        required,
        value_types: value_types
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        allowed_values: allowed_values
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}
