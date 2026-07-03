use super::{conversations, mapping, native, publications, run_service, workflow_schedule};
use crate::errors::ControlPlaneError;

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::{
    ApiKeyRepository, AppendRunEventInput, ApplicationApiMappingRepository,
    ApplicationCompileContextRepository, ApplicationCompiledPlanRepository,
    ApplicationJsDependencySelectionRepository, ApplicationPublicationRepository,
    ApplicationRepository, ApplicationVisibility, AuthRepository, CacheStore, CreateApiKeyInput,
    CreateApplicationInput, CreateApplicationPublicationVersionInput, CreateApplicationTagInput,
    CreateFlowRunInput, DeleteApplicationInput, FlowRepository, ReplaceApplicationApiMappingInput,
    ReplaceApplicationEnvironmentVariablesInput, ReplaceApplicationJsDependencySelectionInput,
    ReplaceWorkflowScheduleTriggerInput, SetApplicationApiEnabledInput, UpdateApplicationInput,
    UpdateProfileInput, UpsertCompiledPlanInput, WorkflowScheduleTriggerRepository,
};

const TEST_TENANT_ID: Uuid = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
const TEST_WORKSPACE_ID: Uuid = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
const TEST_ROOT_USER_ID: Uuid = Uuid::from_u128(0x33333333333333333333333333333333);

#[derive(Default)]
struct ApplicationPublicApiTestRepositoryInner {
    applications: HashMap<Uuid, domain::ApplicationRecord>,
    api_keys: HashMap<Uuid, domain::ApiKeyRecord>,
    conversations:
        HashMap<(Uuid, Uuid, String, String), conversations::ApplicationPublicConversationRecord>,
    run_conversations: HashMap<Uuid, Uuid>,
    actor_permissions: Vec<String>,
    mappings: HashMap<Uuid, mapping::ApplicationApiMappingConfig>,
    editor_states: HashMap<Uuid, domain::FlowEditorState>,
    compiled_plans: HashMap<Uuid, domain::CompiledPlanRecord>,
    publications: HashMap<Uuid, publications::ApplicationPublicationVersionRecord>,
    workflow_schedule_triggers: HashMap<Uuid, workflow_schedule::WorkflowScheduleTriggerRecord>,
    js_dependency_selections:
        HashMap<(Uuid, String, String), domain::ApplicationJsDependencySelection>,
    application_api_enabled: HashMap<Uuid, bool>,
    native_runs: HashMap<Uuid, native::NativeRunResult>,
    flow_runs: HashMap<Uuid, domain::FlowRunRecord>,
    node_runs: HashMap<Uuid, domain::NodeRunRecord>,
    callback_tasks: HashMap<Uuid, domain::CallbackTaskRecord>,
    callback_resume_attempts: HashMap<String, domain::FlowRunCallbackResumeAttemptRecord>,
    run_events: HashMap<Uuid, Vec<domain::RunEventRecord>>,
    application_environment_variables: HashMap<Uuid, Vec<domain::ApplicationEnvironmentVariable>>,
    editor_state_read_count: usize,
    next_flow_ordinal: u128,
    next_flow_version_sequence: i64,
    next_compiled_plan_ordinal: u128,
    next_publication_ordinal: u128,
    next_workflow_schedule_ordinal: u128,
    api_key_last_used_write_counts: HashMap<Uuid, usize>,
    fail_mark_api_key_used: bool,
}

#[derive(Clone, Default)]
pub struct ApplicationPublicApiTestRepository {
    inner: Arc<Mutex<ApplicationPublicApiTestRepositoryInner>>,
}

impl ApplicationPublicApiTestRepository {
    fn with_permissions(permissions: Vec<&str>) -> Self {
        let repository = Self::default();
        repository
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .actor_permissions = permissions.into_iter().map(str::to_string).collect();
        repository
    }

    fn set_workflow_trigger_type(
        &self,
        application_id: Uuid,
        trigger_type: domain::WorkflowTriggerType,
    ) {
        if let Some(application) = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .applications
            .get_mut(&application_id)
        {
            application.workflow_trigger_type = Some(trigger_type);
        }
    }

    fn seed_application_with_type(
        &self,
        actor_user_id: Uuid,
        name: &str,
        application_type: domain::ApplicationType,
    ) -> domain::ApplicationRecord {
        let subject_kind = application_type.as_str().to_string();
        let application = domain::ApplicationRecord {
            id: Uuid::now_v7(),
            workspace_id: TEST_WORKSPACE_ID,
            application_type,
            workflow_trigger_type: match application_type {
                domain::ApplicationType::AgentFlow => None,
                domain::ApplicationType::Workflow => Some(domain::WorkflowTriggerType::Extension),
            },
            name: name.to_string(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
            created_by: actor_user_id,
            updated_at: OffsetDateTime::now_utc(),
            tags: Vec::new(),
            sections: domain::ApplicationSections {
                orchestration: domain::ApplicationOrchestrationSection {
                    status: "planned".to_string(),
                    subject_kind,
                    subject_status: "unconfigured".to_string(),
                    current_subject_id: None,
                    current_draft_id: None,
                },
                api: domain::ApplicationApiSection {
                    status: "planned".to_string(),
                    credential_kind: "application_api_key".to_string(),
                    invoke_routing_mode: "api_key_bound_application".to_string(),
                    invoke_path_template: Some("/api/agent/v1/runs".to_string()),
                    api_capability_status: "not_published".to_string(),
                    credentials_status: "missing".to_string(),
                },
                logs: domain::ApplicationLogsSection {
                    status: "planned".to_string(),
                    runs_capability_status: "planned".to_string(),
                    run_object_kind: "application_run".to_string(),
                    log_retention_status: "planned".to_string(),
                },
                monitoring: domain::ApplicationMonitoringSection {
                    status: "planned".to_string(),
                    metrics_capability_status: "planned".to_string(),
                    metrics_object_kind: "application_metrics".to_string(),
                    tracing_config_status: "planned".to_string(),
                },
            },
        };
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .applications
            .insert(application.id, application.clone());
        application
    }

    fn seed_application(&self, actor_user_id: Uuid, name: &str) -> domain::ApplicationRecord {
        self.seed_application_with_type(actor_user_id, name, domain::ApplicationType::AgentFlow)
    }

    pub fn contains_api_key(&self, api_key_id: Uuid) -> bool {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .api_keys
            .contains_key(&api_key_id)
    }

    pub fn api_key_last_used_write_count(&self, api_key_id: Uuid) -> usize {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .api_key_last_used_write_counts
            .get(&api_key_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn fail_mark_api_key_used(&self, fail: bool) {
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .fail_mark_api_key_used = fail;
    }
}

pub struct ApplicationPublicApiTestHarness {
    repository: ApplicationPublicApiTestRepository,
}

impl ApplicationPublicApiTestHarness {
    pub fn new() -> Self {
        Self {
            repository: ApplicationPublicApiTestRepository::with_permissions(vec![
                "state_model.manage.all",
                "application.view.all",
                "application.edit.all",
            ]),
        }
    }

    pub fn new_with_permissions(permissions: Vec<&str>) -> Self {
        Self {
            repository: ApplicationPublicApiTestRepository::with_permissions(permissions),
        }
    }

    pub fn repository(&self) -> ApplicationPublicApiTestRepository {
        self.repository.clone()
    }

    pub fn last_used_cache(&self) -> ApplicationPublicApiTestCache {
        ApplicationPublicApiTestCache::default()
    }

    pub fn seed_application(&self, actor_user_id: Uuid, name: &str) -> domain::ApplicationRecord {
        self.repository.seed_application(actor_user_id, name)
    }

    pub fn seed_workflow_application(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        self.repository.seed_application_with_type(
            actor_user_id,
            name,
            domain::ApplicationType::Workflow,
        )
    }

    pub fn set_workflow_trigger_type(
        &self,
        application_id: Uuid,
        trigger_type: domain::WorkflowTriggerType,
    ) {
        self.repository
            .set_workflow_trigger_type(application_id, trigger_type);
    }
}

#[derive(Clone, Default)]
pub struct ApplicationPublicApiTestCache {
    inner: Arc<Mutex<ApplicationPublicApiTestCacheInner>>,
}

#[derive(Default)]
struct ApplicationPublicApiTestCacheInner {
    keys: HashMap<String, serde_json::Value>,
    last_ttl: Option<time::Duration>,
}

impl ApplicationPublicApiTestCache {
    pub fn last_ttl(&self) -> Option<time::Duration> {
        self.inner
            .lock()
            .expect("application public api test cache mutex poisoned")
            .last_ttl
    }
}

#[async_trait]
impl CacheStore for ApplicationPublicApiTestCache {
    async fn get_json(&self, key: &str) -> Result<Option<serde_json::Value>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test cache mutex poisoned")
            .keys
            .get(key)
            .cloned())
    }

    async fn set_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test cache mutex poisoned");
        inner.last_ttl = ttl;
        inner.keys.insert(key.to_string(), value);
        Ok(())
    }

    async fn set_if_absent_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test cache mutex poisoned");
        inner.last_ttl = ttl;
        if inner.keys.contains_key(key) {
            return Ok(false);
        }
        inner.keys.insert(key.to_string(), value);
        Ok(true)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.inner
            .lock()
            .expect("application public api test cache mutex poisoned")
            .keys
            .remove(key);
        Ok(())
    }

    async fn touch(&self, key: &str, ttl: time::Duration) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test cache mutex poisoned");
        inner.last_ttl = Some(ttl);
        Ok(inner.keys.contains_key(key))
    }
}

impl Default for ApplicationPublicApiTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkflowScheduleTriggerRepository for ApplicationPublicApiTestRepository {
    async fn get_workflow_schedule_trigger(
        &self,
        application_id: Uuid,
    ) -> Result<Option<workflow_schedule::WorkflowScheduleTriggerRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .workflow_schedule_triggers
            .get(&application_id)
            .cloned())
    }

    async fn list_enabled_workflow_schedule_triggers(
        &self,
    ) -> Result<Vec<workflow_schedule::WorkflowScheduleTriggerRecord>> {
        let mut triggers = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .workflow_schedule_triggers
            .values()
            .filter(|trigger| trigger.enabled)
            .cloned()
            .collect::<Vec<_>>();
        triggers.sort_by_key(|trigger| trigger.application_id);
        Ok(triggers)
    }

    async fn replace_workflow_schedule_trigger(
        &self,
        input: &ReplaceWorkflowScheduleTriggerInput,
    ) -> Result<workflow_schedule::WorkflowScheduleTriggerRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if !inner.applications.contains_key(&input.application_id) {
            return Err(ControlPlaneError::NotFound("application").into());
        }
        let existing = inner
            .workflow_schedule_triggers
            .get(&input.application_id)
            .cloned();
        inner.next_workflow_schedule_ordinal += 1;
        let now = OffsetDateTime::UNIX_EPOCH
            + time::Duration::seconds(inner.next_workflow_schedule_ordinal as i64);
        let record = workflow_schedule::WorkflowScheduleTriggerRecord {
            id: existing
                .as_ref()
                .map(|record| record.id)
                .unwrap_or_else(|| {
                    deterministic_test_id(
                        0x77777777777777770000000000000000,
                        inner.next_workflow_schedule_ordinal,
                    )
                }),
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            enabled: input.enabled,
            cron: input.cron.clone(),
            timezone: input.timezone.clone(),
            input_payload: input.input_payload.clone(),
            created_by: existing
                .as_ref()
                .map(|record| record.created_by)
                .unwrap_or(input.actor_user_id),
            updated_by: input.actor_user_id,
            created_at: existing
                .as_ref()
                .map(|record| record.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        inner
            .workflow_schedule_triggers
            .insert(input.application_id, record.clone());
        Ok(record)
    }
}

pub(super) fn deterministic_test_id(prefix: u128, ordinal: u128) -> Uuid {
    Uuid::from_u128(prefix | ordinal)
}

mod api_key_repository;
mod application_repository;
mod auth_repository;
mod compiled_plan_repository;
mod flow_repository;
mod publication_repository;
mod repository_seeders;
mod run_repositories;
