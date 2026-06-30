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
            PublishApplicationCommand,
        },
        workflow_extension::{
            CreateWorkflowExtensionRunCommand, WorkflowExtensionRequestParameters,
            WorkflowExtensionRunService,
        },
        ApplicationPublicApiTestHarness,
    },
    auth::ApiKeyService,
    ports::{
        ApplicationJsDependencySelectionRepository, FlowRepository,
        ReplaceApplicationJsDependencySelectionInput,
    },
};
use std::collections::BTreeMap;
use std::sync::Arc;
use time::Duration;
use uuid::Uuid;

mod anthropic_compat;
mod client_protocol_envelope;
mod conversations;
mod native_run;
mod openai_compat;
mod resume;
mod run_service;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn root_user_id() -> Uuid {
    Uuid::from_u128(0x33333333333333333333333333333333)
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
    assert!(editor_state
        .versions
        .iter()
        .any(|version| version.id == publication.flow_version_id && version.is_protected));
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
        stored.external_trace_id.as_deref(),
        Some("workflow-extension:open-ticket")
    );
    assert_eq!(
        stored.compatibility_mode.as_deref(),
        Some("workflow_extension_v1")
    );
}

#[tokio::test]
async fn application_public_api_publish_requires_application_edit_permission() {
    let harness =
        ApplicationPublicApiTestHarness::new_with_permissions(vec!["application.view.all"]);
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationPublicationService::new(harness.repository());

    let error = service
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
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
