use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    application_public_api::{
        api_keys::{
            ApplicationApiKeyService, CreateApplicationApiKeyCommand,
            ListApplicationApiKeysCommand, RevokeApplicationApiKeyCommand,
        },
        mapping::{
            validate_application_api_mapping, ApplicationApiMappingConfig,
            ApplicationApiMappingInput, ApplicationApiMappingOutput, ApplicationApiMappingService,
            GetApplicationApiMappingCommand, ReplaceApplicationApiMappingCommand,
            WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
            WorkflowExtensionParameterMapping, WorkflowExtensionParameterSource,
            WorkflowExtensionResponseMode,
        },
        publications::{
            ApplicationPublicationService, LoadActiveApplicationPublicationCommand,
            PublishApplicationCommand, SetApplicationApiEnabledCommand,
        },
        workflow_extension::{
            CreateWorkflowExtensionRunCommand, WorkflowExtensionRequestParameters,
            WorkflowExtensionRunService,
        },
        workflow_schedule::{
            workflow_schedule_cron_matches, DispatchWorkflowScheduleCommand,
            GetWorkflowScheduleTriggerCommand, ReplaceWorkflowScheduleTriggerCommand,
            WorkflowScheduleDispatchOutcome, WorkflowScheduleTriggerService,
            WORKFLOW_SCHEDULE_RUN_QUEUE,
        },
        ApplicationPublicApiTestHarness,
    },
    auth::ApiKeyService,
    ports::{
        ApplicationJsDependencySelectionRepository, ClaimedTask, EphemeralInspectionCapabilities,
        FlowRepository, ReplaceApplicationJsDependencySelectionInput, TaskQueue,
    },
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use time::Duration;
use uuid::Uuid;

mod anthropic_compat;
mod client_protocol_envelope;
mod conversations;
mod native_run;
mod openai_compat;
mod resume;
mod run_service;
mod workflow_start_http_inputs;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn root_user_id() -> Uuid {
    Uuid::from_u128(0x33333333333333333333333333333333)
}

fn applications_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
    )
    .expect("applications settings feature id must be valid")
}

fn application_operation_id(value: &str) -> domain::ConsoleOperationId {
    domain::ConsoleOperationId::try_from(value).expect("application operation id must be valid")
}

fn application_console_policy(
    operations: Vec<domain::ConsoleOperationPolicy>,
) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            applications_console_group(),
            operations,
        )],
    )
}

fn application_simple_operation(
    operation_id: &str,
    enabled: bool,
) -> domain::ConsoleOperationPolicy {
    domain::ConsoleOperationPolicy::simple(application_operation_id(operation_id), enabled)
}

fn application_row_operation(
    operation_id: &str,
    scope: domain::ConsoleOperationRowScope,
) -> domain::ConsoleOperationPolicy {
    domain::ConsoleOperationPolicy::row(application_operation_id(operation_id), scope)
}

fn workflow_extension_mapping(slug: &str) -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: Some(WorkflowExtensionApiConfig {
            slug: slug.into(),
            method: WorkflowExtensionHttpMethod::Post,
            response_mode: WorkflowExtensionResponseMode::Async,
            parameters: vec![WorkflowExtensionParameterMapping {
                name: "customer_id".into(),
                source: WorkflowExtensionParameterSource::Query,
                target: "node-workflow-start.customer_id".into(),
            }],
        }),
    }
}

fn application_public_api_code_js_dependency_document(flow_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Code Imports", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-code",
                    "type": "code",
                    "alias": "Code",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": { "imports": ["zod"] },
                    "bindings": {},
                    "outputs": [{ "key": "result", "title": "Result", "valueType": "json" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-code",
                    "source": "node-start",
                    "target": "node-code",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

#[derive(Default)]
struct RecordingTaskQueue {
    enqueued: Mutex<Vec<(String, serde_json::Value, Option<String>)>>,
}

#[async_trait]
impl TaskQueue for RecordingTaskQueue {
    async fn enqueue(
        &self,
        queue: &str,
        payload: serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> Result<String> {
        let task_id = format!(
            "task-{}",
            self.enqueued
                .lock()
                .expect("recording task queue mutex poisoned")
                .len()
                + 1
        );
        self.enqueued
            .lock()
            .expect("recording task queue mutex poisoned")
            .push((
                queue.to_string(),
                payload,
                idempotency_key.map(ToOwned::to_owned),
            ));
        Ok(task_id)
    }

    async fn claim(
        &self,
        _queue: &str,
        _worker: &str,
        _visibility_timeout: Duration,
    ) -> Result<Option<ClaimedTask>> {
        Ok(None)
    }

    async fn ack(&self, _queue: &str, _task_id: &str, _worker: &str) -> Result<bool> {
        Ok(false)
    }

    async fn fail(
        &self,
        _queue: &str,
        _task_id: &str,
        _worker: &str,
        _reason: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }
}

#[tokio::test]
async fn application_public_api_key_service_requires_application_edit_permission_for_create() {
    let harness =
        ApplicationPublicApiTestHarness::new_with_permissions(vec!["application.view.all"]);
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());

    let error = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn application_public_api_create_returns_sk_token_exactly_once_and_allows_duplicate_names() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());

    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(created.token.starts_with("sk-"));
    assert!(created.api_key.token_prefix.starts_with("sk-"));
    assert_eq!(created.token.len(), 56);
    assert_eq!(created.api_key.token_prefix.len(), 15);
    assert_eq!(created.token.matches('-').count(), 2);
    assert_ne!(created.api_key.token_prefix, created.token);

    let duplicate = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(duplicate.token.starts_with("sk-"));
    assert_eq!(duplicate.token.len(), 56);
    assert_eq!(duplicate.api_key.token_prefix.len(), 15);
    assert_ne!(duplicate.api_key.id, created.api_key.id);
    assert_eq!(duplicate.api_key.name, created.api_key.name);

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|key| key.id == created.api_key.id
        && key.token_prefix == created.api_key.token_prefix
        && key.token_prefix != created.token));
    assert!(listed.iter().any(|key| key.id == duplicate.api_key.id
        && key.token_prefix == duplicate.api_key.token_prefix
        && key.token_prefix != duplicate.token));
}

#[tokio::test]
async fn application_public_api_list_only_returns_current_actor_keys_for_current_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_app = harness.seed_application(actor_user_id(), "First App");
    let second_app = harness.seed_application(actor_user_id(), "Second App");
    let service = ApplicationApiKeyService::new(harness.repository());

    let mine = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: first_app.id,
            name: "Mine".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: other_user_id(),
            application_id: first_app.id,
            name: "Other user".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: second_app.id,
            name: "Other app".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: first_app.id,
        })
        .await
        .unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, mine.api_key.id);
    assert_eq!(listed[0].application_id, Some(first_app.id));
    assert_eq!(listed[0].creator_user_id, actor_user_id());
}

#[tokio::test]
async fn application_public_api_delete_removes_key_and_makes_token_unusable() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Temporary".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .revoke_api_key(RevokeApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_key_id: created.api_key.id,
        })
        .await
        .unwrap();

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    let auth_error = service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap_err();

    assert!(!harness.repository().contains_api_key(created.api_key.id));
    assert!(listed.is_empty());
    assert!(auth_error.to_string().contains("not_authenticated"));
}

#[tokio::test]
async fn application_public_api_authentication_records_last_used_time_for_key_list() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let before_use = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    assert_eq!(before_use[0].id, created.api_key.id);
    assert!(before_use[0].last_used_at.is_none());

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();

    let after_use = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    assert_eq!(after_use[0].id, created.api_key.id);
    assert!(after_use[0].last_used_at.is_some());
}

#[tokio::test]
async fn application_public_api_last_used_write_is_throttled_for_sixty_seconds() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let cache = Arc::new(harness.last_used_cache());
    let service =
        ApplicationApiKeyService::new(harness.repository()).with_last_used_cache(cache.clone());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();
    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();

    assert_eq!(
        harness
            .repository()
            .api_key_last_used_write_count(created.api_key.id),
        1
    );
    assert_eq!(cache.last_ttl(), Some(Duration::seconds(60)));
}

#[tokio::test]
async fn application_public_api_last_used_write_failure_does_not_fail_authentication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    repository.fail_mark_api_key_used(true);
    let service = ApplicationApiKeyService::new(repository);
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();
}

#[tokio::test]
async fn application_public_api_root_has_no_global_view_every_users_key_list_path() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Owner key".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let root_visible = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert!(
        root_visible.is_empty(),
        "root may manage explicitly authorized app resources, but key list remains current-actor scoped"
    );
}

#[tokio::test]
async fn application_public_api_rejects_legacy_data_model_api_key_tokens() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let user_api_key_service = ApiKeyService::new(repository.clone());
    let application_key_service = ApplicationApiKeyService::new(repository);

    let apk = application_key_service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Application runtime".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(apk.token.starts_with("sk-"));
    application_key_service
        .authenticate_bearer_token(&apk.token)
        .await
        .unwrap();
    assert!(user_api_key_service
        .authenticate_bearer_token("dmk_legacy_token")
        .await
        .is_err());
    assert!(application_key_service
        .authenticate_bearer_token("dmk_legacy_token")
        .await
        .is_err());
    assert!(user_api_key_service
        .authenticate_bearer_token(&apk.token)
        .await
        .is_err());
    assert!(application_key_service
        .authenticate_bearer_token("apk_legacy_token")
        .await
        .is_err());
}

#[tokio::test]
async fn application_public_api_mapping_service_returns_default_then_replaces_stored_mapping() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiMappingService::new(harness.repository());

    let default_mapping = service
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    let replacement = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: Some("node-start".into()),
            history_target: Some("node-start.history".into()),
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput {
            answer_selector: Some("answer.text".into()),
            usage_selector: None,
            files_selector: None,
            error_selector: None,
        },
        extension: None,
    };
    service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: replacement.clone(),
        })
        .await
        .unwrap();
    let stored = service
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert_eq!(
        default_mapping,
        ApplicationApiMappingConfig::default_native()
    );
    assert_eq!(stored, replacement);
}

#[tokio::test]
async fn application_public_api_mapping_service_requires_edit_permission_for_replace() {
    let harness =
        ApplicationPublicApiTestHarness::new_with_permissions(vec!["application.view.all"]);
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiMappingService::new(harness.repository());

    let error = service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn application_public_api_mapping_service_rejects_duplicate_extension_slug() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first = harness.seed_application(actor_user_id(), "Support Bot A");
    let second = harness.seed_application(actor_user_id(), "Support Bot B");
    let service = ApplicationApiMappingService::new(harness.repository());

    service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: first.id,
            mapping: workflow_extension_mapping("open-ticket-conflict"),
        })
        .await
        .unwrap();
    let error = service
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: second.id,
            mapping: workflow_extension_mapping("open-ticket-conflict"),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_mapping_service_rejects_extension_slug_used_by_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let published = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let draft = harness.seed_application(actor_user_id(), "Draft Workflow");
    let repository = harness.repository();

    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: published.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table"),
            api_enabled: true,
        })
        .await
        .unwrap();
    let error = ApplicationApiMappingService::new(repository)
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: draft.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table"),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_publish_updates_current_publication_record() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let first = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let second = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig {
                input: ApplicationApiMappingInput {
                    query_target: "node-start.query".into(),
                    model_target: None,
                    inputs_target: Some("node-start".into()),
                    history_target: None,
                    attachments_target: None,
                },
                output: ApplicationApiMappingOutput::default(),
                extension: None,
            },
            api_enabled: true,
        })
        .await
        .unwrap();

    let reloaded = service
        .get_publication_version(first.id)
        .await
        .unwrap()
        .unwrap();
    let versions = service
        .list_publication_versions(application.id)
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(versions, vec![second.clone()]);
    assert_eq!(reloaded.mapping_snapshot, second.mapping_snapshot);
    assert_eq!(reloaded.compiled_plan_id, second.compiled_plan_id);
    assert!(reloaded.active);
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_is_empty_without_selection() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let reloaded = service
        .get_publication_version(publication.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(publication.dependency_snapshot, Vec::new());
    assert_eq!(reloaded.dependency_snapshot, Vec::new());
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_updates_current_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000001),
            provider_code: "fixture_js_dependency_pack_3".into(),
            plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
            plugin_version: "3.24.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "3.24.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-3.24.0".into(),
            integrity: "sha256-zod-3.24.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let first = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000002),
            provider_code: "fixture_js_dependency_pack_4".into(),
            plugin_id: "fixture_js_dependency_pack@4.0.0".into(),
            plugin_version: "4.0.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "4.0.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-4.0.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-4.0.0".into(),
            integrity: "sha256-zod-4.0.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let second = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let reloaded = service
        .get_publication_version(first.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(reloaded.dependency_snapshot.len(), 1);
    assert_eq!(reloaded.dependency_snapshot[0].alias, "zod");
    assert_eq!(reloaded.dependency_snapshot[0].package, "zod");
    assert_eq!(reloaded.dependency_snapshot[0].version, "4.0.0");
    assert_eq!(
        reloaded.dependency_snapshot[0].artifact_hash,
        "sha256-zod-4.0.0"
    );
    assert_eq!(
        reloaded.dependency_snapshot[0].permissions.network,
        "outbound_only"
    );
    assert_eq!(second.dependency_snapshot[0].version, "4.0.0");
    assert_eq!(
        second.dependency_snapshot[0].artifact_hash,
        "sha256-zod-4.0.0"
    );
}

#[tokio::test]
async fn application_public_api_js_dependency_compile_context_enables_code_imports() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();

    FlowRepository::save_draft(
        &repository,
        application.workspace_id,
        application.id,
        actor_user_id(),
        application_public_api_code_js_dependency_document(editor_state.flow.id),
        domain::FlowChangeKind::Logical,
        "Add code dependency import",
    )
    .await
    .unwrap();

    ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
        &repository,
        &ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: actor_user_id(),
            workspace_id: application.workspace_id,
            application_id: application.id,
            installation_id: Uuid::from_u128(0x90000000000000000000000000000003),
            provider_code: "fixture_js_dependency_pack_3".into(),
            plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
            plugin_version: "3.24.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "3.24.0".into(),
            target: "backend_code".into(),
            artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
            artifact_hash: "sha256-zod-3.24.0".into(),
            integrity: "sha256-zod-3.24.0".into(),
            permissions: domain::JsDependencyPermissions {
                network: "outbound_only".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        },
    )
    .await
    .unwrap();

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist a compiled plan");

    assert_eq!(
        compiled_plan.plan["compile_issues"],
        serde_json::json!([]),
        "application compile context should include selected backend_code::zod"
    );
}

#[tokio::test]
async fn application_public_api_publish_uses_real_flow_version_and_compiled_plan_records() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist a compiled plan");

    assert_eq!(publication.flow_id, editor_state.flow.id);
    assert!(
        editor_state
            .versions
            .iter()
            .any(|version| version.id == publication.flow_version_id
                && version.is_current_publication)
    );
    assert_eq!(compiled_plan.flow_id, editor_state.flow.id);
    assert_eq!(publication.document_snapshot, editor_state.draft.document);
    assert_ne!(
        publication.flow_schema_version,
        "application-public-api-placeholder-v1"
    );
    assert_ne!(
        publication.document_snapshot["source"],
        "application_public_api_placeholder"
    );
}

#[tokio::test]
async fn republishing_moves_current_publication_without_user_protecting_the_previous_version() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Republished Support Bot");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let first_publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let second_publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let editor_state = repository
        .get_or_create_editor_state(application.workspace_id, application.id, actor_user_id())
        .await
        .unwrap();
    let first_version = editor_state
        .versions
        .iter()
        .find(|version| version.id == first_publication.flow_version_id)
        .unwrap();
    let second_version = editor_state
        .versions
        .iter()
        .find(|version| version.id == second_publication.flow_version_id)
        .unwrap();

    assert!(!first_version.is_current_publication);
    assert!(!first_version.is_user_protected);
    assert!(second_version.is_current_publication);
    assert!(!second_version.is_user_protected);
}

#[tokio::test]
async fn application_public_api_publish_compiles_workflow_application_as_workflow() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();
    let service = ApplicationPublicationService::new(repository.clone());

    let publication = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("workflow application should publish through workflow compiler");
    let compiled_plan = repository
        .get_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("publish should persist workflow compiled plan");

    assert_eq!(
        compiled_plan.plan["nodes"]["node-workflow-start"]["node_type"],
        serde_json::json!("workflow_start")
    );
    assert_eq!(
        compiled_plan.plan["nodes"]["node-workflow-end"]["node_type"],
        serde_json::json!("workflow_end")
    );
}

#[tokio::test]
async fn application_public_api_publish_rejects_duplicate_extension_slug() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow A");
    let second = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow B");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: first.id,
            mapping: workflow_extension_mapping("open-ticket-publish-conflict"),
            api_enabled: true,
        })
        .await
        .unwrap();
    let error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: second.id,
            mapping: workflow_extension_mapping("open-ticket-publish-conflict"),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_publish_rejects_extension_slug_used_by_saved_mapping() {
    let harness = ApplicationPublicApiTestHarness::new();
    let draft = harness.seed_application(actor_user_id(), "Draft Workflow");
    let published = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();

    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: actor_user_id(),
            application_id: draft.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table-publish"),
        })
        .await
        .unwrap();
    let error = ApplicationPublicationService::new(repository)
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: published.id,
            mapping: workflow_extension_mapping("open-ticket-cross-table-publish"),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("extension_slug"));
}

#[tokio::test]
async fn workflow_extension_run_maps_path_query_form_and_body_parameters() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Ticket Workflow");
    let repository = harness.repository();
    let token = ApplicationApiKeyService::new(repository.clone())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Workflow extension".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token;
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: Some(WorkflowExtensionApiConfig {
            slug: "open-ticket".into(),
            method: WorkflowExtensionHttpMethod::Post,
            response_mode: WorkflowExtensionResponseMode::Async,
            parameters: vec![
                WorkflowExtensionParameterMapping {
                    name: "slug".into(),
                    source: WorkflowExtensionParameterSource::Path,
                    target: "node-workflow-start.slug".into(),
                },
                WorkflowExtensionParameterMapping {
                    name: "customer_id".into(),
                    source: WorkflowExtensionParameterSource::Query,
                    target: "node-workflow-start.customer_id".into(),
                },
                WorkflowExtensionParameterMapping {
                    name: "priority".into(),
                    source: WorkflowExtensionParameterSource::Form,
                    target: "node-workflow-start.priority".into(),
                },
                WorkflowExtensionParameterMapping {
                    name: "ticket_kind".into(),
                    source: WorkflowExtensionParameterSource::Body,
                    target: "node-workflow-start.ticket_kind".into(),
                },
            ],
        }),
    };
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap();

    let run = WorkflowExtensionRunService::new(repository.clone())
        .create_run(CreateWorkflowExtensionRunCommand {
            bearer_token: token,
            slug: "open-ticket".into(),
            method: WorkflowExtensionHttpMethod::Post,
            parameters: WorkflowExtensionRequestParameters {
                path: BTreeMap::from([("slug".to_string(), serde_json::json!("open-ticket"))]),
                query: serde_json::Map::from_iter([(
                    "customer_id".to_string(),
                    serde_json::json!("C-42"),
                )]),
                form: serde_json::Map::from_iter([(
                    "priority".to_string(),
                    serde_json::json!("urgent"),
                )]),
                body: serde_json::json!({ "ticket_kind": "billing" }),
            },
        })
        .await
        .unwrap();
    let stored = repository
        .get_flow_run(application.id, run.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(run.status, domain::FlowRunStatus::Queued);
    assert_eq!(
        stored.input_payload["node-workflow-start"],
        serde_json::json!({
            "slug": "open-ticket",
            "customer_id": "C-42",
            "priority": "urgent",
            "ticket_kind": "billing"
        })
    );
    assert_eq!(
        stored.input_payload["trigger"],
        serde_json::json!({ "type": "extension" })
    );
    assert_eq!(
        stored.external_trace_id.as_deref(),
        Some("workflow-extension:open-ticket")
    );
    assert_eq!(
        stored.compatibility_mode.as_deref(),
        Some("workflow_extension_v1")
    );
}

#[tokio::test]
async fn workflow_schedule_trigger_service_replaces_config_and_rejects_invalid_values() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Scheduled Workflow");
    let service = WorkflowScheduleTriggerService::new(harness.repository());

    let stored = service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "Asia/Shanghai".into(),
            input_payload: serde_json::json!({
                "node-workflow-start": { "customer_id": "C-42" }
            }),
        })
        .await
        .unwrap();
    let loaded = service
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap()
        .expect("schedule trigger should be stored");

    assert!(stored.enabled);
    assert_eq!(stored.cron, "0 9 * * *");
    assert_eq!(loaded.timezone, "Asia/Shanghai");

    assert!(service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "bad cron".into(),
            timezone: "Asia/Shanghai".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("cron"));
    assert!(service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "Mars/Olympus".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("timezone"));
}

#[tokio::test]
async fn workflow_schedule_trigger_dispatch_creates_traceable_async_run_and_task() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Scheduled Workflow");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Schedule);
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({
                "node-workflow-start": { "customer_id": "C-42" },
                "sys": { "user_id": "spoofed" },
                "trigger": { "type": "spoofed" }
            }),
        })
        .await
        .unwrap();
    let task_queue = RecordingTaskQueue::default();
    let scheduled_at = time::OffsetDateTime::UNIX_EPOCH + Duration::hours(9);

    let outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();
    let WorkflowScheduleDispatchOutcome::Dispatched(dispatched) = outcome else {
        panic!("enabled workflow schedule should dispatch");
    };
    let stored = repository
        .get_flow_run(application.id, dispatched.run_id)
        .await
        .unwrap()
        .expect("scheduled run should be durable");
    let enqueued = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();

    assert_eq!(dispatched.status, domain::FlowRunStatus::Queued);
    assert_eq!(dispatched.task_id.as_deref(), Some("task-1"));
    assert_eq!(stored.run_mode, domain::FlowRunMode::PublishedApiRun);
    assert_eq!(
        stored.external_trace_id.as_deref(),
        Some(format!("workflow-schedule:{}", application.id).as_str())
    );
    assert_eq!(
        stored.compatibility_mode.as_deref(),
        Some("workflow_schedule_v1")
    );
    assert_eq!(
        stored.input_payload["node-workflow-start"]["customer_id"],
        serde_json::json!("C-42")
    );
    assert_eq!(
        stored.input_payload["trigger"],
        serde_json::json!({
            "type": "schedule",
            "scheduled_at": "1970-01-01T09:00:00Z",
            "timezone": "UTC"
        })
    );
    assert!(stored.input_payload.get("sys").is_none());
    assert_eq!(enqueued.len(), 1);
    assert_eq!(enqueued[0].0, WORKFLOW_SCHEDULE_RUN_QUEUE);
    assert_eq!(
        enqueued[0].1["flow_run_id"],
        serde_json::json!(dispatched.run_id.to_string())
    );
    assert_eq!(
        enqueued[0].2.as_deref(),
        Some(
            format!(
                "workflow-schedule:{}:{}",
                application.id,
                scheduled_at.unix_timestamp()
            )
            .as_str()
        )
    );

    let duplicate = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();
    let WorkflowScheduleDispatchOutcome::Dispatched(duplicate) = duplicate else {
        panic!("duplicate schedule dispatch should return existing run");
    };
    let enqueued_after_duplicate = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();

    assert_eq!(duplicate.run_id, dispatched.run_id);
    assert_eq!(duplicate.task_id, None);
    assert_eq!(enqueued_after_duplicate.len(), 1);
}

#[test]
fn workflow_schedule_cron_matcher_covers_five_field_expressions() {
    let at = |hour: u8, minute: u8| {
        time::OffsetDateTime::UNIX_EPOCH
            .replace_time(time::Time::from_hms(hour, minute, 0).unwrap())
    };

    // 1970-01-01 is a Thursday (day-of-week 4).
    assert!(workflow_schedule_cron_matches("* * * * *", at(9, 30)));
    assert!(workflow_schedule_cron_matches("0 9 * * *", at(9, 0)));
    assert!(!workflow_schedule_cron_matches("0 9 * * *", at(9, 1)));
    assert!(workflow_schedule_cron_matches("*/15 * * * *", at(3, 45)));
    assert!(!workflow_schedule_cron_matches("*/15 * * * *", at(3, 50)));
    assert!(workflow_schedule_cron_matches("0 9-17 * * *", at(12, 0)));
    assert!(!workflow_schedule_cron_matches("0 9-17 * * *", at(18, 0)));
    assert!(workflow_schedule_cron_matches("0 9,18 * * *", at(18, 0)));
    assert!(workflow_schedule_cron_matches("0 0 1 1 *", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 0 2 1 *", at(0, 0)));
    assert!(workflow_schedule_cron_matches("0 0 * * 4", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 0 * * 5", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 9 * *", at(9, 0)));
}

#[tokio::test]
async fn workflow_schedule_tick_dispatches_only_matching_enabled_triggers() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    let publication_service = ApplicationPublicationService::new(repository.clone());
    let matching = harness.seed_workflow_application(actor_user_id(), "Matching Schedule");
    let wrong_cron = harness.seed_workflow_application(actor_user_id(), "Wrong Cron");
    let disabled = harness.seed_workflow_application(actor_user_id(), "Disabled Schedule");
    let shanghai = harness.seed_workflow_application(actor_user_id(), "Shanghai Schedule");
    let invalid_timezone = harness.seed_workflow_application(actor_user_id(), "Broken Timezone");

    for application_id in [matching.id, wrong_cron.id, shanghai.id, invalid_timezone.id] {
        publication_service
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: actor_user_id(),
                application_id,
                mapping: ApplicationApiMappingConfig::default_native(),
                api_enabled: true,
            })
            .await
            .unwrap();
    }

    let seed_trigger = |application_id, enabled, cron: &str, timezone: &str| {
        service.replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id,
            enabled,
            cron: cron.into(),
            timezone: timezone.into(),
            input_payload: serde_json::json!({}),
        })
    };
    for application_id in [matching.id, wrong_cron.id, shanghai.id, invalid_timezone.id] {
        harness.set_workflow_trigger_type(application_id, domain::WorkflowTriggerType::Schedule);
    }
    seed_trigger(matching.id, true, "0 1 * * *", "UTC")
        .await
        .unwrap();
    seed_trigger(wrong_cron.id, true, "30 12 * * *", "UTC")
        .await
        .unwrap();
    seed_trigger(disabled.id, false, "0 1 * * *", "UTC")
        .await
        .unwrap();
    // 01:00 UTC is 09:00 in Asia/Shanghai.
    seed_trigger(shanghai.id, true, "0 9 * * *", "Asia/Shanghai")
        .await
        .unwrap();
    seed_trigger(
        invalid_timezone.id,
        true,
        "0 1 * * *",
        "America/Nowhere_Fake",
    )
    .await
    .unwrap();

    let task_queue = RecordingTaskQueue::default();
    let now_utc = time::OffsetDateTime::UNIX_EPOCH + Duration::hours(1) + Duration::seconds(17);

    let entries = service
        .dispatch_due_schedules(now_utc, Some(&task_queue))
        .await
        .unwrap();

    let dispatched_ids = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                WorkflowScheduleDispatchOutcome::Dispatched(_)
            )
        })
        .map(|entry| entry.application_id)
        .collect::<Vec<_>>();
    assert!(dispatched_ids.contains(&matching.id));
    assert!(dispatched_ids.contains(&shanghai.id));
    assert_eq!(dispatched_ids.len(), 2);
    assert!(entries.iter().any(|entry| {
        entry.application_id == invalid_timezone.id
            && entry.outcome
                == WorkflowScheduleDispatchOutcome::Skipped {
                    reason: "invalid_timezone",
                }
    }));
    assert!(!entries
        .iter()
        .any(|entry| entry.application_id == wrong_cron.id || entry.application_id == disabled.id));

    let enqueued = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();
    assert_eq!(enqueued.len(), 2);

    // The same minute must not enqueue duplicates on a repeated tick.
    let repeat = service
        .dispatch_due_schedules(now_utc + Duration::seconds(20), Some(&task_queue))
        .await
        .unwrap();
    let repeat_dispatched = repeat
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                WorkflowScheduleDispatchOutcome::Dispatched(_)
            )
        })
        .count();
    assert_eq!(repeat_dispatched, 2);
    let enqueued_after_repeat = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .len();
    assert_eq!(enqueued_after_repeat, 2);
}

#[tokio::test]
async fn workflow_schedule_dispatch_skips_extension_trigger_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    // seed_workflow_application defaults to the extension trigger type.
    let application = harness.seed_workflow_application(actor_user_id(), "Extension Typed");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "* * * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap();
    let task_queue = RecordingTaskQueue::default();

    let outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();

    assert_eq!(
        outcome,
        WorkflowScheduleDispatchOutcome::Skipped {
            reason: "trigger_type_mismatch",
        }
    );
    assert!(task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .is_empty());
}

#[tokio::test]
async fn workflow_schedule_trigger_dispatch_skips_disabled_or_unpublished_applications() {
    let harness = ApplicationPublicApiTestHarness::new();
    let disabled = harness.seed_workflow_application(actor_user_id(), "Disabled Schedule");
    let unpublished = harness.seed_workflow_application(actor_user_id(), "Unpublished Schedule");
    for application_id in [disabled.id, unpublished.id] {
        harness.set_workflow_trigger_type(application_id, domain::WorkflowTriggerType::Schedule);
    }
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: disabled.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    for application in [disabled.id, unpublished.id] {
        service
            .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
                actor_user_id: actor_user_id(),
                application_id: application,
                enabled: application == unpublished.id,
                cron: "0 9 * * *".into(),
                timezone: "UTC".into(),
                input_payload: serde_json::json!({}),
            })
            .await
            .unwrap();
    }

    let disabled_outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: disabled.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();
    let unpublished_outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: unpublished.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();

    assert!(matches!(
        disabled_outcome,
        WorkflowScheduleDispatchOutcome::Skipped { reason }
            if reason == "disabled"
    ));
    assert!(matches!(
        unpublished_outcome,
        WorkflowScheduleDispatchOutcome::Skipped { reason }
            if reason == "application_not_published"
    ));
}

#[tokio::test]
async fn ac_005_ac_007_application_public_api_simple_operations_require_persisted_update_owner() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, false),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                false,
            ),
            application_row_operation(
                access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                domain::ConsoleOperationRowScope::Own,
            ),
        ]),
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
        ]),
    ]);
    let application = harness.seed_application(actor_user_id(), "Owned support bot");
    let peer_application = harness.seed_application(other_user_id(), "Peer support bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("own application must retain publish access");
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: false,
        })
        .await
        .expect("own application must retain API-status access");

    let publish_error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect_err("update own must not publish a same-workspace peer application");
    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            api_enabled: true,
        })
        .await
        .expect_err("update own must not change a same-workspace peer API status");

    assert!(publish_error.to_string().contains("permission_denied"));
    assert!(status_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_005_ac_007_application_public_api_multi_role_union_retains_update_scope_all() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
            application_row_operation(
                access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                domain::ConsoleOperationRowScope::Own,
            ),
        ]),
        application_console_policy(vec![application_row_operation(
            access_control::APPLICATIONS_UPDATE_OPERATION_ID,
            domain::ConsoleOperationRowScope::ScopeAll,
        )]),
    ]);
    let peer_application = harness.seed_application(other_user_id(), "Peer support bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .expect("multi-role scope_all must retain publish access");
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: peer_application.id,
            api_enabled: false,
        })
        .await
        .expect("multi-role scope_all must retain API-status access");
}

#[tokio::test]
async fn ac_007_application_public_api_simple_operations_do_not_read_legacy_edit_grants() {
    let harness = ApplicationPublicApiTestHarness::new_with_permissions(vec![
        "application.view.all",
        "application.edit.all",
    ]);
    let application = harness.seed_application(actor_user_id(), "Legacy only");
    let service = ApplicationPublicationService::new(harness.repository());

    let publish_error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap_err();
    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(publish_error.to_string().contains("permission_denied"));
    assert!(status_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_007_application_public_api_simple_operations_default_deny_and_honor_disabled() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                false,
            ),
        ]),
    ]);
    let application = harness.seed_application(actor_user_id(), "Disabled status");
    let service = ApplicationPublicationService::new(harness.repository());

    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(status_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_006_application_public_api_simple_operations_cannot_cross_workspace() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_simple_operation(access_control::APPLICATIONS_PUBLISH_OPERATION_ID, true),
            application_simple_operation(
                access_control::APPLICATIONS_API_SET_ENABLED_OPERATION_ID,
                true,
            ),
        ]),
    ]);
    let application =
        harness.seed_application_in_workspace(Uuid::now_v7(), actor_user_id(), "Other workspace");
    let service = ApplicationPublicationService::new(harness.repository());

    let publish_error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap_err();
    let status_error = service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(publish_error
        .to_string()
        .contains("resource not found: application"));
    assert!(status_error
        .to_string()
        .contains("resource not found: application"));
}

#[tokio::test]
async fn ac_006_application_public_api_root_bypasses_policy_but_not_workspace() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(Vec::new());
    let current = harness.seed_application(root_user_id(), "Root current workspace");
    let foreign = harness.seed_application_in_workspace(
        Uuid::now_v7(),
        root_user_id(),
        "Root other workspace",
    );
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: root_user_id(),
            application_id: current.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    service
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: root_user_id(),
            application_id: current.id,
            api_enabled: false,
        })
        .await
        .unwrap();

    for error in [
        service
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: root_user_id(),
                application_id: foreign.id,
                mapping: ApplicationApiMappingConfig::default_native(),
                api_enabled: true,
            })
            .await
            .unwrap_err(),
        service
            .set_api_enabled(SetApplicationApiEnabledCommand {
                actor_user_id: root_user_id(),
                application_id: foreign.id,
                api_enabled: true,
            })
            .await
            .unwrap_err(),
    ] {
        assert!(error
            .to_string()
            .contains("resource not found: application"));
    }
}

#[tokio::test]
async fn ac_007_publish_operation_does_not_bypass_mapping_domain_validation() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![application_simple_operation(
            access_control::APPLICATIONS_PUBLISH_OPERATION_ID,
            true,
        )]),
    ]);
    let application = harness.seed_application(actor_user_id(), "Invalid mapping");
    let mut mapping = ApplicationApiMappingConfig::default_native();
    mapping.input.query_target.clear();

    let error = ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping,
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("invalid input: query_target"));
}

#[tokio::test]
async fn application_public_api_only_one_current_publication_exists_per_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let latest = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();

    let versions = service
        .list_publication_versions(application.id)
        .await
        .unwrap();
    assert_eq!(versions, vec![latest]);
}

#[tokio::test]
async fn application_public_api_public_lookup_returns_application_not_published_without_active_publication(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let error = service
        .load_active_publication(LoadActiveApplicationPublicationCommand {
            application_id: application.id,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("application_not_published"));
}

#[test]
fn application_public_api_mapping_validation_rejects_missing_query_target_and_invalid_selector() {
    let missing_query_target = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };
    let invalid_selector = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "start.messages[0].content".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    assert!(validate_application_api_mapping(&missing_query_target)
        .unwrap_err()
        .to_string()
        .contains("query_target"));
    assert!(validate_application_api_mapping(&invalid_selector)
        .unwrap_err()
        .to_string()
        .contains("selector"));
}

#[test]
fn application_public_api_mapping_validation_accepts_null_model_target() {
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    validate_application_api_mapping(&mapping).unwrap();
}

#[test]
fn application_public_api_mapping_validation_rejects_invalid_workflow_extension_config() {
    let mut invalid_slug = workflow_extension_mapping("OpenTicket");
    assert!(validate_application_api_mapping(&invalid_slug)
        .unwrap_err()
        .to_string()
        .contains("extension.slug"));

    invalid_slug = workflow_extension_mapping("open-ticket");
    invalid_slug
        .extension
        .as_mut()
        .unwrap()
        .parameters
        .push(WorkflowExtensionParameterMapping {
            name: "customer_id".into(),
            source: WorkflowExtensionParameterSource::Query,
            target: "node-workflow-start.duplicate_customer_id".into(),
        });
    assert!(validate_application_api_mapping(&invalid_slug)
        .unwrap_err()
        .to_string()
        .contains("parameter"));
}
