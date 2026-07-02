use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    application::{ApplicationService, InMemoryApplicationRepository},
    errors::ControlPlaneError,
    ports::{
        ApplicationRepository, ApplicationVisibility, CreateApplicationInput,
        CreateApplicationTagInput, DeleteApplicationInput, FlowRepository, ModelProviderRepository,
        NodeContributionRepository, UpdateApplicationInput,
    },
};

mod agentflow_template;

pub use agentflow_template::{
    load_agent_flow_template_resources, AgentFlowTemplateApplication, AgentFlowTemplateDependency,
    AgentFlowTemplateDependencyStatus, AgentFlowTemplatePackage, AgentFlowTemplatePreview,
    AgentFlowTemplateResourceSnapshot, AgentFlowTemplateUnresolvedNode,
    AGENT_FLOW_TEMPLATE_SCHEMA_VERSION, UNRESOLVED_NODE_TYPE,
};

pub struct SaveFlowDraftCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub document: serde_json::Value,
    pub change_kind: domain::FlowChangeKind,
    pub summary: String,
}

pub struct UpdateFlowVersionMetadataCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub version_id: Uuid,
    pub summary: Option<String>,
    pub summary_is_custom: Option<bool>,
    pub is_protected: Option<bool>,
}

pub struct PreviewAgentFlowTemplateCommand {
    pub actor_user_id: Uuid,
    pub template: AgentFlowTemplatePackage,
    pub resources: AgentFlowTemplateResourceSnapshot,
}

pub struct ImportAgentFlowTemplateCommand {
    pub actor_user_id: Uuid,
    pub template: AgentFlowTemplatePackage,
    pub name: Option<String>,
    pub description: Option<String>,
    pub resources: AgentFlowTemplateResourceSnapshot,
}

pub struct ImportAgentFlowTemplateResult {
    pub application: domain::ApplicationRecord,
    pub orchestration: domain::FlowEditorState,
    pub preview: AgentFlowTemplatePreview,
}

const VISIBLE_INTERNAL_LLM_TOOL_TYPE: &str = "visible_internal_llm_tool";
const FLOW_TOOL_IDENTIFIER_MAX_LENGTH: usize = 64;

pub struct FlowService<R> {
    repository: R,
}

impl<R> FlowService<R>
where
    R: ApplicationRepository + FlowRepository + Clone,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_or_create_editor_state(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        ApplicationService::new(self.repository.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;

        self.repository
            .get_or_create_editor_state(actor.current_workspace_id, application_id, actor_user_id)
            .await
    }

    pub async fn save_draft(
        &self,
        command: SaveFlowDraftCommand,
    ) -> Result<domain::FlowEditorState> {
        ApplicationService::new(self.repository.clone())
            .get_application(command.actor_user_id, command.application_id)
            .await?;
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        validate_flow_draft_document(&command.document)?;

        self.repository
            .save_draft(
                actor.current_workspace_id,
                command.application_id,
                command.actor_user_id,
                command.document,
                command.change_kind,
                &command.summary,
            )
            .await
    }

    pub async fn restore_version(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
        version_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        ApplicationService::new(self.repository.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;

        self.repository
            .restore_version(
                actor.current_workspace_id,
                application_id,
                actor_user_id,
                version_id,
            )
            .await
    }

    pub async fn update_version_metadata(
        &self,
        command: UpdateFlowVersionMetadataCommand,
    ) -> Result<domain::FlowEditorState> {
        ApplicationService::new(self.repository.clone())
            .get_application(command.actor_user_id, command.application_id)
            .await?;
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;

        self.repository
            .update_version_metadata(
                actor.current_workspace_id,
                command.application_id,
                command.actor_user_id,
                command.version_id,
                command.summary,
                command.summary_is_custom,
                command.is_protected,
            )
            .await
    }

    pub async fn export_agent_flow_template(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<AgentFlowTemplatePackage> {
        let application = ApplicationService::new(self.repository.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        if application.application_type != domain::ApplicationType::AgentFlow {
            return Err(ControlPlaneError::InvalidInput("application_type").into());
        }
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        let flow_state = self
            .repository
            .get_or_create_editor_state(actor.current_workspace_id, application_id, actor_user_id)
            .await?;

        Ok(agentflow_template::build_agent_flow_template_package(
            &application,
            &flow_state.draft.document,
        ))
    }

    pub async fn preview_agent_flow_template(
        &self,
        command: PreviewAgentFlowTemplateCommand,
    ) -> Result<AgentFlowTemplatePreview> {
        self.repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;

        agentflow_template::preview_agent_flow_template_package(
            command.template,
            &command.resources,
        )
    }

    pub async fn import_agent_flow_template(
        &self,
        command: ImportAgentFlowTemplateCommand,
    ) -> Result<ImportAgentFlowTemplateResult> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let preview = agentflow_template::preview_agent_flow_template_package(
            command.template.clone(),
            &command.resources,
        )?;
        let application_name = command
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| preview.application.name.clone());
        let application_description = command
            .description
            .unwrap_or_else(|| preview.application.description.clone());
        let application = ApplicationService::new(self.repository.clone())
            .create_application(crate::application::CreateApplicationCommand {
                actor_user_id: command.actor_user_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: application_name,
                description: application_description,
                icon: preview.application.icon.clone(),
                icon_type: preview.application.icon_type.clone(),
                icon_background: preview.application.icon_background.clone(),
            })
            .await?;
        let bootstrapped = self
            .repository
            .get_or_create_editor_state(
                actor.current_workspace_id,
                application.id,
                command.actor_user_id,
            )
            .await?;
        let (document, _) = agentflow_template::import_agent_flow_template_document(
            &command.template,
            bootstrapped.flow.id,
            &command.resources,
        )?;
        validate_flow_draft_document(&document)?;
        let orchestration = self
            .repository
            .save_draft(
                actor.current_workspace_id,
                application.id,
                command.actor_user_id,
                document,
                domain::FlowChangeKind::Logical,
                "导入 AgentFlow 模板",
            )
            .await?;

        Ok(ImportAgentFlowTemplateResult {
            application,
            orchestration,
            preview,
        })
    }
}

impl<R> FlowService<R>
where
    R: ApplicationRepository
        + FlowRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + Clone,
{
    pub async fn load_agent_flow_template_resources(
        &self,
        actor_user_id: Uuid,
    ) -> Result<AgentFlowTemplateResourceSnapshot> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;

        agentflow_template::load_agent_flow_template_resources(
            &self.repository,
            actor.current_workspace_id,
        )
        .await
    }
}

fn validate_flow_draft_document(document: &Value) -> Result<()> {
    let Some(nodes) = document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };

    for node in nodes {
        if node.get("type").and_then(Value::as_str) != Some("llm") {
            continue;
        }

        let Some(config) = node.get("config").and_then(Value::as_object) else {
            continue;
        };
        let tools_enabled = config
            .get("visible_internal_llm_tools_enabled")
            .or_else(|| config.get("visibleInternalLlmToolsEnabled"))
            .and_then(Value::as_bool)
            == Some(true);

        if !tools_enabled {
            continue;
        }

        let Some(tools) = config
            .get("visible_internal_llm_tools")
            .or_else(|| config.get("visibleInternalLlmTools"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let mut tool_names = HashSet::new();
        let mut connector_ids = HashSet::new();

        for tool in tools {
            if tool.get("type").and_then(Value::as_str) != Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE) {
                continue;
            }

            let Some(tool_name) = trimmed_string_field(tool, "tool_name", "name") else {
                return Err(ControlPlaneError::InvalidInput(
                    "visible_internal_llm_tools.tool_name",
                )
                .into());
            };

            if !is_flow_tool_identifier(tool_name) || !tool_names.insert(tool_name.to_string()) {
                return Err(ControlPlaneError::InvalidInput(
                    "visible_internal_llm_tools.tool_name",
                )
                .into());
            }

            let effective_connector_id = match string_field(tool, "connector_id", "connectorId") {
                Some(value) => value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or(ControlPlaneError::InvalidInput(
                        "visible_internal_llm_tools.connector_id",
                    ))?,
                None => tool_name,
            };

            if !is_flow_tool_identifier(effective_connector_id)
                || !connector_ids.insert(effective_connector_id.to_string())
            {
                return Err(ControlPlaneError::InvalidInput(
                    "visible_internal_llm_tools.connector_id",
                )
                .into());
            }
        }
    }

    Ok(())
}

fn trimmed_string_field<'a>(value: &'a Value, snake_key: &str, camel_key: &str) -> Option<&'a str> {
    string_field(value, snake_key, camel_key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn string_field<'a>(value: &'a Value, snake_key: &str, camel_key: &str) -> Option<&'a Value> {
    value.get(snake_key).or_else(|| value.get(camel_key))
}

fn is_flow_tool_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= FLOW_TOOL_IDENTIFIER_MAX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[derive(Default)]
struct InMemoryFlowRepositoryInner {
    editor_state_by_application_id: HashMap<Uuid, domain::FlowEditorState>,
}

#[derive(Clone)]
pub struct InMemoryFlowRepository {
    applications: InMemoryApplicationRepository,
    inner: Arc<Mutex<InMemoryFlowRepositoryInner>>,
}

impl InMemoryFlowRepository {
    pub fn with_permissions(permissions: Vec<&str>) -> Self {
        Self {
            applications: InMemoryApplicationRepository::with_permissions(permissions),
            inner: Arc::new(Mutex::new(InMemoryFlowRepositoryInner::default())),
        }
    }

    pub async fn seed_application_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::create_application(
            &self.applications,
            &CreateApplicationInput {
                actor_user_id,
                workspace_id: Uuid::nil(),
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        )
        .await
    }
}

#[async_trait]
impl ApplicationRepository for InMemoryFlowRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        ApplicationRepository::load_actor_context_for_user(&self.applications, actor_user_id).await
    }

    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: crate::ports::ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        ApplicationRepository::list_applications(
            &self.applications,
            workspace_id,
            actor_user_id,
            visibility,
        )
        .await
    }

    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::create_application(&self.applications, input).await
    }

    async fn delete_application(&self, input: &DeleteApplicationInput) -> Result<()> {
        ApplicationRepository::delete_application(&self.applications, input).await
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        ApplicationRepository::get_application(&self.applications, workspace_id, application_id)
            .await
    }

    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::update_application(&self.applications, input).await
    }

    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        ApplicationRepository::list_application_tags(
            &self.applications,
            workspace_id,
            actor_user_id,
            visibility,
        )
        .await
    }

    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        ApplicationRepository::create_application_tag(&self.applications, input).await
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        ApplicationRepository::append_audit_log(&self.applications, event).await
    }
}

#[async_trait]
impl FlowRepository for InMemoryFlowRepository {
    async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        let application_type = ApplicationRepository::get_application(
            &self.applications,
            workspace_id,
            application_id,
        )
        .await?
        .map(|application| application.application_type)
        .unwrap_or(domain::ApplicationType::AgentFlow);
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory flow repo mutex poisoned");

        Ok(inner
            .editor_state_by_application_id
            .entry(application_id)
            .or_insert_with(|| {
                bootstrap_editor_state(application_id, actor_user_id, application_type)
            })
            .clone())
    }

    async fn save_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: domain::FlowChangeKind,
        summary: &str,
    ) -> Result<domain::FlowEditorState> {
        let application_type = ApplicationRepository::get_application(
            &self.applications,
            workspace_id,
            application_id,
        )
        .await?
        .map(|application| application.application_type)
        .unwrap_or(domain::ApplicationType::AgentFlow);
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory flow repo mutex poisoned");
        let state = inner
            .editor_state_by_application_id
            .entry(application_id)
            .or_insert_with(|| {
                bootstrap_editor_state(application_id, actor_user_id, application_type)
            });

        state.flow.updated_at = OffsetDateTime::now_utc();
        state.draft.schema_version = document_schema_version(&document).to_string();
        state.draft.document = document.clone();
        state.draft.updated_at = OffsetDateTime::now_utc();

        if matches!(change_kind, domain::FlowChangeKind::Logical) {
            let sequence = state
                .versions
                .last()
                .map(|version| version.sequence + 1)
                .unwrap_or(1);
            state.versions.push(domain::FlowVersionRecord {
                id: Uuid::now_v7(),
                flow_id: state.flow.id,
                sequence,
                trigger: domain::FlowVersionTrigger::Autosave,
                change_kind: domain::FlowChangeKind::Logical,
                summary: summary.to_string(),
                summary_is_custom: false,
                is_protected: false,
                document,
                created_at: OffsetDateTime::now_utc(),
            });
            trim_versions(&mut state.versions);
        }

        Ok(state.clone())
    }

    async fn restore_version(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        let application_type = ApplicationRepository::get_application(
            &self.applications,
            workspace_id,
            application_id,
        )
        .await?
        .map(|application| application.application_type)
        .unwrap_or(domain::ApplicationType::AgentFlow);
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory flow repo mutex poisoned");
        let state = inner
            .editor_state_by_application_id
            .entry(application_id)
            .or_insert_with(|| {
                bootstrap_editor_state(application_id, actor_user_id, application_type)
            });
        let restored = state
            .versions
            .iter()
            .find(|version| version.id == version_id)
            .cloned()
            .ok_or(ControlPlaneError::NotFound("flow_version"))?;

        state.flow.updated_at = OffsetDateTime::now_utc();
        state.draft.schema_version = document_schema_version(&restored.document).to_string();
        state.draft.document = restored.document.clone();
        state.draft.updated_at = OffsetDateTime::now_utc();
        state.versions.push(domain::FlowVersionRecord {
            id: Uuid::now_v7(),
            flow_id: state.flow.id,
            sequence: state
                .versions
                .last()
                .map(|version| version.sequence + 1)
                .unwrap_or(1),
            trigger: domain::FlowVersionTrigger::Restore,
            change_kind: domain::FlowChangeKind::Logical,
            summary: format!("恢复版本 {}", restored.sequence),
            summary_is_custom: false,
            is_protected: false,
            document: restored.document,
            created_at: OffsetDateTime::now_utc(),
        });
        trim_versions(&mut state.versions);

        Ok(state.clone())
    }

    async fn update_version_metadata(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
        summary: Option<String>,
        summary_is_custom: Option<bool>,
        is_protected: Option<bool>,
    ) -> Result<domain::FlowEditorState> {
        let application_type = ApplicationRepository::get_application(
            &self.applications,
            workspace_id,
            application_id,
        )
        .await?
        .map(|application| application.application_type)
        .unwrap_or(domain::ApplicationType::AgentFlow);
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory flow repo mutex poisoned");
        let state = inner
            .editor_state_by_application_id
            .entry(application_id)
            .or_insert_with(|| {
                bootstrap_editor_state(application_id, actor_user_id, application_type)
            });
        let version = state
            .versions
            .iter_mut()
            .find(|version| version.id == version_id)
            .ok_or(ControlPlaneError::NotFound("flow_version"))?;

        if let Some(summary) = summary {
            version.summary = summary;
        }

        if let Some(summary_is_custom) = summary_is_custom {
            version.summary_is_custom = summary_is_custom;
        }

        if let Some(is_protected) = is_protected {
            version.is_protected = is_protected;
        }

        state.versions.sort_by(|left, right| {
            right
                .is_protected
                .cmp(&left.is_protected)
                .then_with(|| left.sequence.cmp(&right.sequence))
        });

        Ok(state.clone())
    }
}

fn bootstrap_editor_state(
    application_id: Uuid,
    actor_user_id: Uuid,
    application_type: domain::ApplicationType,
) -> domain::FlowEditorState {
    let flow_id = Uuid::now_v7();
    let document = domain::default_flow_document_for_application(application_type, flow_id);

    domain::FlowEditorState {
        flow: domain::FlowRecord {
            id: flow_id,
            application_id,
            created_by: actor_user_id,
            updated_at: OffsetDateTime::now_utc(),
        },
        draft: domain::FlowDraftRecord {
            id: Uuid::now_v7(),
            flow_id,
            schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
            document: document.clone(),
            updated_at: OffsetDateTime::now_utc(),
        },
        versions: vec![domain::FlowVersionRecord {
            id: Uuid::now_v7(),
            flow_id,
            sequence: 1,
            trigger: domain::FlowVersionTrigger::Autosave,
            change_kind: domain::FlowChangeKind::Logical,
            summary: "初始化默认草稿".to_string(),
            summary_is_custom: false,
            is_protected: false,
            document,
            created_at: OffsetDateTime::now_utc(),
        }],
        autosave_interval_seconds: domain::FLOW_AUTOSAVE_INTERVAL_SECONDS,
    }
}

fn document_schema_version(document: &serde_json::Value) -> &str {
    document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(domain::FLOW_SCHEMA_VERSION)
}

fn trim_versions(versions: &mut Vec<domain::FlowVersionRecord>) {
    let unprotected_count = versions
        .iter()
        .filter(|version| !version.is_protected)
        .count();

    if unprotected_count > domain::FLOW_HISTORY_LIMIT {
        let overflow = unprotected_count - domain::FLOW_HISTORY_LIMIT;
        let mut removed = 0;

        versions.retain(|version| {
            if !version.is_protected && removed < overflow {
                removed += 1;
                false
            } else {
                true
            }
        });
    }

    versions.sort_by(|left, right| {
        right
            .is_protected
            .cmp(&left.is_protected)
            .then_with(|| left.sequence.cmp(&right.sequence))
    });
}

impl FlowService<InMemoryFlowRepository> {
    pub fn for_tests() -> Self {
        Self::new(InMemoryFlowRepository::with_permissions(vec![
            "application.view.all",
            "application.create.all",
        ]))
    }

    pub fn for_tests_with_permissions(permissions: Vec<&str>) -> Self {
        Self::new(InMemoryFlowRepository::with_permissions(permissions))
    }

    pub async fn seed_application_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> Result<domain::ApplicationRecord> {
        self.seed_application_with_type_for_actor(
            actor_user_id,
            name,
            domain::ApplicationType::AgentFlow,
        )
        .await
    }

    pub async fn seed_application_with_type_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
        application_type: domain::ApplicationType,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationService::new(self.repository.clone())
            .create_application(crate::application::CreateApplicationCommand {
                actor_user_id,
                application_type,
                workflow_trigger_type: match application_type {
                    domain::ApplicationType::AgentFlow => None,
                    domain::ApplicationType::Workflow => {
                        Some(domain::WorkflowTriggerType::Extension)
                    }
                },
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            })
            .await
    }
}
