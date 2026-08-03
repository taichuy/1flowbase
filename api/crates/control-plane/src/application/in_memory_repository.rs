use super::*;

#[derive(Default)]
pub(super) struct InMemoryApplicationRepositoryInner {
    applications: HashMap<Uuid, domain::ApplicationRecord>,
    environment_variables: HashMap<Uuid, Vec<domain::ApplicationEnvironmentVariable>>,
    pub(super) js_dependencies: Vec<domain::JsDependencyRegistryEntry>,
    js_dependency_selections:
        HashMap<(Uuid, String, String), domain::ApplicationJsDependencySelection>,
    tags: HashMap<Uuid, domain::ApplicationTagCatalogEntry>,
    permissions: Vec<String>,
    pub(super) console_policies: Vec<domain::RoleConsolePolicy>,
    pub(super) actor_is_root: bool,
    workspace_id: Uuid,
    tenant_id: Uuid,
    pub(super) audit_events: Vec<String>,
}

#[derive(Clone)]
pub struct InMemoryApplicationRepository {
    pub(super) inner: Arc<Mutex<InMemoryApplicationRepositoryInner>>,
}

impl InMemoryApplicationRepository {
    pub fn with_permissions(permissions: Vec<&str>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InMemoryApplicationRepositoryInner {
                applications: HashMap::new(),
                environment_variables: HashMap::new(),
                js_dependencies: Vec::new(),
                js_dependency_selections: HashMap::new(),
                tags: HashMap::new(),
                permissions: permissions.into_iter().map(str::to_string).collect(),
                console_policies: Vec::new(),
                actor_is_root: false,
                workspace_id: Uuid::nil(),
                tenant_id: Uuid::nil(),
                audit_events: Vec::new(),
            })),
        }
    }

    pub fn with_console_policies(policies: Vec<domain::RoleConsolePolicy>) -> Self {
        let repository = Self::with_permissions(Vec::new());
        repository
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .console_policies = policies;
        repository
    }

    pub(super) fn insert_application(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = build_application_record(
            Uuid::now_v7(),
            CreateApplicationInput {
                workflow_trigger_config: None,
                actor_user_id,
                workspace_id: inner.workspace_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        );
        inner
            .applications
            .insert(application.id, application.clone());
        application
    }

    pub(super) fn insert_application_in_workspace(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        name: &str,
    ) -> domain::ApplicationRecord {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = build_application_record(
            Uuid::now_v7(),
            CreateApplicationInput {
                workflow_trigger_config: None,
                actor_user_id,
                workspace_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        );
        inner
            .applications
            .insert(application.id, application.clone());
        application
    }
}

#[async_trait]
impl JsDependencyRepository for InMemoryApplicationRepository {
    async fn replace_installation_js_dependencies(
        &self,
        input: &ReplaceInstallationJsDependenciesInput,
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        inner
            .js_dependencies
            .retain(|entry| entry.installation_id != input.installation_id);
        inner
            .js_dependencies
            .extend(
                input
                    .entries
                    .iter()
                    .map(|entry| domain::JsDependencyRegistryEntry {
                        installation_id: input.installation_id,
                        provider_code: input.provider_code.clone(),
                        plugin_id: input.plugin_id.clone(),
                        plugin_version: input.plugin_version.clone(),
                        alias: entry.alias.clone(),
                        package: entry.package.clone(),
                        version: entry.version.clone(),
                        target: entry.target.clone(),
                        artifact_path: entry.artifact_path.clone(),
                        integrity: entry.integrity.clone(),
                        permissions: entry.permissions.clone(),
                    }),
            );
        Ok(())
    }

    async fn list_workspace_js_dependencies(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::JsDependencyRegistryEntry>> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependencies
            .clone())
    }
}

#[async_trait]
impl crate::ports::ApplicationJsDependencySelectionRepository for InMemoryApplicationRepository {
    async fn list_application_js_dependency_selections(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationJsDependencySelection>> {
        let mut selections = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependency_selections
            .values()
            .filter(|selection| {
                selection.workspace_id == workspace_id && selection.application_id == application_id
            })
            .cloned()
            .collect::<Vec<_>>();
        selections.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then(left.target.cmp(&right.target))
        });

        Ok(selections)
    }

    async fn replace_application_js_dependency_selection(
        &self,
        input: &ReplaceApplicationJsDependencySelectionInput,
    ) -> Result<domain::ApplicationJsDependencySelection> {
        let selection = domain::ApplicationJsDependencySelection {
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            installation_id: input.installation_id,
            provider_code: input.provider_code.clone(),
            plugin_id: input.plugin_id.clone(),
            plugin_version: input.plugin_version.clone(),
            alias: input.alias.clone(),
            package: input.package.clone(),
            version: input.version.clone(),
            target: input.target.clone(),
            artifact_path: input.artifact_path.clone(),
            artifact_hash: input.artifact_hash.clone(),
            integrity: input.integrity.clone(),
            permissions: input.permissions.clone(),
        };
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .js_dependency_selections
            .insert(
                (
                    input.application_id,
                    input.alias.clone(),
                    input.target.clone(),
                ),
                selection.clone(),
            );

        Ok(selection)
    }
}

#[async_trait]
impl ApplicationRepository for InMemoryApplicationRepository {
    async fn settle_application_archive_releases(
        &self,
        workspace_id: Uuid,
        digests: &[ApplicationArchiveReleaseDigest],
    ) -> Result<Vec<ApplicationArchiveRelease>> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        if digests.iter().any(|digest| {
            inner
                .applications
                .get(&digest.application_id)
                .is_none_or(|application| application.workspace_id != workspace_id)
        }) {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(digests
            .iter()
            .map(|digest| {
                let application = inner
                    .applications
                    .get_mut(&digest.application_id)
                    .expect("application existence checked before atomic settlement");
                if application.release_digest.as_deref() != Some(&digest.release_digest) {
                    application.release_version += 1;
                    application.release_digest = Some(digest.release_digest.clone());
                }
                ApplicationArchiveRelease {
                    application_id: application.id,
                    release_version: application.release_version,
                    release_digest: digest.release_digest.clone(),
                }
            })
            .collect())
    }

    async fn record_application_extension_source(
        &self,
        _workspace_id: Uuid,
        _application_id: Uuid,
        _extension_installation_id: Uuid,
        _actor_user_id: Uuid,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn has_application_extension_source(
        &self,
        _workspace_id: Uuid,
        _extension_installation_id: Uuid,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");

        if inner.actor_is_root {
            Ok(domain::ActorContext::root_in_scope(
                actor_user_id,
                inner.tenant_id,
                inner.workspace_id,
                "root",
            ))
        } else {
            Ok(domain::ActorContext::scoped_in_scope(
                actor_user_id,
                inner.tenant_id,
                inner.workspace_id,
                "member",
                inner.permissions.iter().cloned(),
            ))
        }
    }

    async fn load_role_console_policies_for_user(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .console_policies
            .clone())
    }

    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        let mut applications = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .values()
            .filter(|application| application.workspace_id == workspace_id)
            .filter(|application| {
                matches!(visibility, ApplicationVisibility::All)
                    || application.created_by == actor_user_id
            })
            .cloned()
            .collect::<Vec<_>>();
        applications.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then(right.id.cmp(&left.id))
        });

        Ok(applications)
    }

    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let application = build_application_record(Uuid::now_v7(), input.clone());
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .insert(application.id, application.clone());

        Ok(application)
    }

    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let tags = input
            .tag_ids
            .iter()
            .map(|tag_id| inner.tags.get(tag_id).cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or(ControlPlaneError::InvalidInput("tag_ids"))?
            .into_iter()
            .map(|tag| domain::ApplicationTag {
                id: tag.id,
                name: tag.name,
            })
            .collect::<Vec<_>>();
        let application = inner
            .applications
            .get_mut(&input.application_id)
            .ok_or(ControlPlaneError::NotFound("application"))?;
        application.name = input.name.clone();
        application.description = input.description.clone();
        if let Some(icon) = &input.icon {
            application.icon = icon.clone();
        }
        if let Some(icon_type) = &input.icon_type {
            application.icon_type = icon_type.clone();
        }
        if let Some(icon_background) = &input.icon_background {
            application.icon_background = icon_background.clone();
        }
        application.updated_at = time::OffsetDateTime::now_utc();
        application.tags = tags;

        Ok(application.clone())
    }

    async fn delete_application(&self, input: &DeleteApplicationInput) -> Result<()> {
        let deleted = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .remove(&input.application_id);

        if deleted.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(())
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        let application = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .get(&application_id)
            .cloned()
            .filter(|application| application.workspace_id == workspace_id);

        Ok(application)
    }

    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Option<domain::ApplicationRecord>> {
        let application = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .applications
            .get(&application_id)
            .cloned()
            .filter(|application| application.workspace_id == workspace_id)
            .filter(|application| {
                matches!(visibility, ApplicationVisibility::All)
                    || application.created_by == actor_user_id
            });
        Ok(application)
    }

    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let mut tags = inner.tags.values().cloned().collect::<Vec<_>>();
        for tag in &mut tags {
            tag.application_count = inner
                .applications
                .values()
                .filter(|application| application.workspace_id == workspace_id)
                .filter(|application| {
                    matches!(visibility, ApplicationVisibility::All)
                        || application.created_by == actor_user_id
                })
                .filter(|application| application.tags.iter().any(|item| item.id == tag.id))
                .count() as i64;
        }
        tags.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));

        Ok(tags)
    }

    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        if let Some(existing) = inner
            .tags
            .values()
            .find(|tag| tag.name.eq_ignore_ascii_case(&input.name))
            .cloned()
        {
            return Ok(existing);
        }

        let tag = domain::ApplicationTagCatalogEntry {
            id: Uuid::now_v7(),
            name: input.name.clone(),
            application_count: 0,
        };
        inner.tags.insert(tag.id, tag.clone());

        Ok(tag)
    }

    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = inner
            .applications
            .get(&application_id)
            .filter(|application| application.workspace_id == workspace_id);

        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(inner
            .environment_variables
            .get(&application_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn replace_application_environment_variables(
        &self,
        input: &ReplaceApplicationEnvironmentVariablesInput,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let mut inner = self
            .inner
            .lock()
            .expect("in-memory app repo mutex poisoned");
        let application = inner
            .applications
            .get(&input.application_id)
            .filter(|application| application.workspace_id == input.workspace_id);

        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        let updated_at = time::OffsetDateTime::now_utc();
        let variables = input
            .variables
            .iter()
            .map(|variable| domain::ApplicationEnvironmentVariable {
                application_id: input.application_id,
                name: variable.name.clone(),
                value_type: variable.value_type.clone(),
                value: variable.value.clone(),
                description: variable.description.clone(),
                updated_at,
            })
            .collect::<Vec<_>>();
        inner
            .environment_variables
            .insert(input.application_id, variables.clone());

        Ok(variables)
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        self.inner
            .lock()
            .expect("in-memory app repo mutex poisoned")
            .audit_events
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl ApplicationManagementRepository for InMemoryApplicationRepository {
    async fn list_application_management(
        &self,
        _workspace_id: Uuid,
        query: &ApplicationManagementQuery,
    ) -> Result<ApplicationManagementPage> {
        Ok(ApplicationManagementPage {
            items: Vec::new(),
            total: 0,
            page: query.page,
            page_size: query.page_size,
        })
    }
}

fn build_application_record(id: Uuid, input: CreateApplicationInput) -> domain::ApplicationRecord {
    domain::ApplicationRecord {
        id,
        workspace_id: input.workspace_id,
        application_type: input.application_type,
        workflow_trigger_type: input.workflow_trigger_type,
        name: input.name,
        description: input.description,
        icon: input.icon,
        icon_type: input.icon_type,
        icon_background: input.icon_background,
        created_by: input.actor_user_id,
        updated_at: time::OffsetDateTime::now_utc(),
        release_version: 0,
        release_digest: None,
        tags: Vec::new(),
        sections: planned_sections(input.application_type, input.workflow_trigger_type),
    }
}

fn create_application_workflow_trigger_type(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
) -> Option<domain::WorkflowTriggerType> {
    match application_type {
        domain::ApplicationType::AgentFlow => None,
        domain::ApplicationType::Workflow => {
            Some(workflow_trigger_type.unwrap_or(domain::WorkflowTriggerType::Extension))
        }
    }
}

fn planned_sections(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
) -> domain::ApplicationSections {
    let mut sections = domain::ApplicationSections {
        orchestration: domain::ApplicationOrchestrationSection {
            status: "planned".to_string(),
            subject_kind: application_type.as_str().to_string(),
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
    };
    sections.api = product_api_section(application_type, workflow_trigger_type, sections.api);
    sections
}

fn with_product_capability_sections(
    mut application: domain::ApplicationRecord,
) -> domain::ApplicationRecord {
    application.sections.api = product_api_section(
        application.application_type,
        application.workflow_trigger_type,
        application.sections.api,
    );
    application
}

fn product_api_section(
    application_type: domain::ApplicationType,
    workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    agent_flow_api: domain::ApplicationApiSection,
) -> domain::ApplicationApiSection {
    match (application_type, workflow_trigger_type) {
        (domain::ApplicationType::AgentFlow, _) => agent_flow_api,
        (domain::ApplicationType::Workflow, Some(domain::WorkflowTriggerType::Extension)) => {
            domain::ApplicationApiSection {
                status: "available".to_string(),
                credential_kind: "user_or_public".to_string(),
                invoke_routing_mode: "published_workflow_operation".to_string(),
                invoke_path_template: Some("/api/ex/{operation}".to_string()),
                api_capability_status: "available".to_string(),
                credentials_status: "not_required".to_string(),
            }
        }
        (domain::ApplicationType::Workflow, _) => domain::ApplicationApiSection {
            status: "unavailable".to_string(),
            credential_kind: "not_applicable".to_string(),
            invoke_routing_mode: "not_available".to_string(),
            invoke_path_template: None,
            api_capability_status: "unavailable".to_string(),
            credentials_status: "not_applicable".to_string(),
        },
    }
}
