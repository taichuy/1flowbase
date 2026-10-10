mod support;
use control_plane::{
    portable_template::*,
    ports::{ApplicationRepository, FrontstagePageRepository, ModelDefinitionRepository},
};
use control_plane_contracts::ports::{
    PortableTemplateBaselineRepository, PortableTemplateIdentityRepository,
    PortableTemplateTransactionRepository,
};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn fixture() -> PortableTemplatePackage {
    let model_id = Uuid::new_v4();
    let page_id = Uuid::new_v4();
    let tab_id = Uuid::new_v4();
    let block_id = Uuid::new_v4().to_string();
    PortableTemplatePackage { i18n_entries: vec![], release: None,schema_version:PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),plugins:vec![],mcp_bundle:None,applications:vec![],
        data_models:vec![PortableDataModel {id:model_id,code:"portable_orders".into(),title:"Portable orders".into(),description:None,scope_kind:domain::DataModelScopeKind::Workspace,template_provider:"core".into(),template_code:"general".into(),template_version:"v1".into(),status:domain::DataModelStatus::Published,builtin:false,fields:vec![PortableModelField {id:Uuid::new_v4(),code:"label".into(),title:"Label".into(),description:None,field_kind:domain::ModelFieldKind::String,is_system:false,is_required:false,api_required:false,is_unique:false,default_value:None,display_interface:None,display_options:json!({"page_id":page_id}),relation_target_model_id:None,relation_options:json!({})}]}],
        pages:vec![PortablePage {id:page_id,parent_id:None,kind:domain::FrontstagePageKind::Page,title:Some("Portable".into()),icon:None,tooltip:None,is_hidden:false,placement:domain::frontstage::FrontstageNavigationPlacement::Topbar,content_presentation:domain::frontstage::FrontstagePageContentPresentation::Single,slug:Some("portable".into()),rank:"a".into(),visibility_rules:vec![],tabs:vec![PortableTab {id:tab_id,title:Some("Default".into()),rank:"a".into(),is_default:true,route_segment:None,document_root_uid:format!("source-tab-{tab_id}"),document_payload:json!({"root":format!("source-tab-{tab_id}"),"model":model_id,"page":page_id,"tab":tab_id}),blocks:vec![PortableBlock {block_id:block_id.clone(),parent_block_id:None,rank:"a".into(),presentation:domain::frontstage::FrontstageBlockPresentation::Inline,title:Some("Orders".into()),description:None,code_ref:format!("frontstage.block.{block_id}"),schema_version:1,input_mapping:BTreeMap::new(),output_mapping:BTreeMap::new(),runtime_descriptor:json!({"props":{"model_id":model_id}}),source_code:format!("export default function Component() {{ return '{model_id}:{page_id}:{tab_id}:{block_id}'; }}")}]}]}]}
}

#[tokio::test]
async fn user_deleted_tracked_model_is_preserved_without_recreation() {
    let (store, workspace, actor) = support::seed_store().await;
    let mut package = fixture();
    package.pages.clear();
    package.data_models[0].fields.clear();
    let source_id = package.data_models[0].id;
    let stale_target_id = Uuid::now_v7();
    store
        .record_portable_template_identity(
            workspace.id,
            "data_model",
            &source_id.to_string(),
            &stale_target_id.to_string(),
        )
        .await
        .unwrap();
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package)
        .await
        .unwrap();
    assert!(result.complete, "{:?}", result.failures);
    assert!(result.created.is_empty());
    assert!(result
        .skipped
        .iter()
        .any(|item| item.kind == "data_model" && item.reason == "user_deleted"));
    assert!(
        ModelDefinitionRepository::get_model_definition(&store, workspace.id, stale_target_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .load_portable_template_identity_map(workspace.id)
            .await
            .unwrap()[&source_id.to_string()],
        stale_target_id.to_string()
    );
}

#[tokio::test]
async fn installs_main_source_table_and_rewrites_page_without_duplicate_default_tab() {
    let (store, workspace, actor) = support::seed_store().await;
    let mut package = fixture();
    let mut group = package.pages[0].clone();
    group.id = Uuid::new_v4();
    group.kind = domain::FrontstagePageKind::Group;
    group.slug = Some("portable-group".into());
    group.tabs.clear();
    package.pages[0].parent_id = Some(group.id);
    package.pages[0].slug = None;
    package.pages.push(group.clone());
    let source_model = package.data_models[0].id;
    let source_page = package.pages[0].id;
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(result.complete, "{:?}", result.failures);
    let group_id = result.id_map[&group.id.to_string()]
        .parse::<Uuid>()
        .unwrap();
    let restored_group =
        FrontstagePageRepository::get_frontstage_page(&store, workspace.id, group_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(restored_group.kind, domain::FrontstagePageKind::Group);
    assert_eq!(restored_group.slug, group.slug);
    let restored_page = FrontstagePageRepository::get_frontstage_page(
        &store,
        workspace.id,
        result.id_map[&source_page.to_string()]
            .parse::<Uuid>()
            .unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(restored_page.parent_id, Some(group_id));
    let model_id: Uuid = result.id_map[&source_model.to_string()].parse().unwrap();
    let model = ModelDefinitionRepository::get_model_definition(&store, workspace.id, model_id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(model_id, source_model);
    assert_eq!(model.scope_id, workspace.id);
    let columns:Vec<String>=sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema=current_schema() AND table_name=$1").bind(&model.physical_table_name).fetch_all(store.pool()).await.unwrap();
    assert!(
        columns.iter().any(|c| c == "label"),
        "physical custom column missing: {columns:?}"
    );
    let page_id: Uuid = result.id_map[&source_page.to_string()].parse().unwrap();
    assert_eq!(
        model
            .fields
            .iter()
            .find(|f| f.code == "label")
            .unwrap()
            .display_options["page_id"],
        page_id.to_string()
    );
    let tabs = FrontstagePageRepository::list_frontstage_page_tabs(&store, workspace.id, page_id)
        .await
        .unwrap();
    assert_eq!(tabs.len(), 1);
    assert!(tabs[0].is_default);
    let snapshot = store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap();
    let installed = snapshot.pages.iter().find(|p| p.id == page_id).unwrap();
    assert_eq!(
        installed.tabs[0].document_payload["model"],
        model_id.to_string()
    );
    let code = &installed.tabs[0].blocks[0].source_code;
    for old in [
        source_model.to_string(),
        source_page.to_string(),
        package.pages[0].tabs[0].id.to_string(),
        package.pages[0].tabs[0].blocks[0].block_id.clone(),
    ] {
        assert!(!code.contains(&old));
    }
    assert!(code.contains(&model_id.to_string()));
    let exported = PortableTemplateService::new(store.clone())
        .export(
            actor.id,
            PortableTemplateSelection {
                page_ids: vec![page_id],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(exported.pages.len(), 2);
    assert!(exported.pages.iter().any(|page| page.id == group_id));
    assert_eq!(
        exported
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .unwrap()
            .parent_id,
        Some(group_id)
    );
    let serialized = serde_json::to_value(&exported).unwrap();
    // Field names such as "created_by" are schema definitions, not ownership values.
    for resources in ["pages", "applications", "data_models"] {
        for resource in serialized[resources].as_array().unwrap() {
            for forbidden in [
                "physical_table_name",
                "created_by",
                "workspace_id",
                "password_hash",
            ] {
                assert!(
                    resource.get(forbidden).is_none(),
                    "unexpected resource property {resources}.{forbidden}"
                );
            }
        }
    }
    assert!(exported.data_models[0]
        .fields
        .iter()
        .any(|field| field.code == "created_by" && field.is_system));
    let repeated = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package)
        .await
        .unwrap();
    assert!(repeated.complete, "{:?}", repeated.failures);
    assert!(repeated.created.is_empty());
    assert_eq!(
        FrontstagePageRepository::list_frontstage_page_tabs(&store, workspace.id, page_id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn owner_failure_rolls_back_native_resources_and_baselines_then_retries() {
    let (store, workspace, actor) = support::seed_store().await;
    let valid = fixture();
    let mut invalid = valid.clone();
    invalid.data_models[0].fields[0].code = "id".into();
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, invalid)
        .await
        .unwrap();
    assert!(!result.complete);
    assert!(!result.failures.is_empty());
    assert!(result.created.is_empty());
    assert!(result.updated.is_empty());
    assert!(store
        .load_template_baselines(&template_baseline_scope(workspace.id, &valid))
        .await
        .unwrap()
        .is_empty());
    assert!(!store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap()
        .data_models
        .iter()
        .any(|m| m.code == "portable_orders"));
    let retry = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, valid.clone())
        .await
        .unwrap();
    assert!(retry.complete, "{:?}", retry.failures);
    let baselines = store
        .load_template_baselines(&template_baseline_scope(workspace.id, &valid))
        .await
        .unwrap();
    assert!(!baselines.is_empty());
    assert!(baselines
        .iter()
        .all(|b| b.applied_fingerprint.is_some() && b.pending.is_none()));
}

fn workflow_document(flow_id: Uuid, name: &str) -> serde_json::Value {
    json!({"schemaVersion":"1flowbase.flow/v2","meta":{"flowId":flow_id,"name":name,"description":"","tags":[]},"graph":{"nodes":[
        {"id":"start","type":"workflow_start","alias":"Start","description":"","containerId":null,"position":{"x":0,"y":0},"configVersion":1,"config":{"input_fields":[],"sync_timeout_ms":30000},"bindings":{},"outputs":[]},
        {"id":"end","type":"workflow_end","alias":"End","description":"","containerId":null,"position":{"x":240,"y":0},"configVersion":1,"config":{},"bindings":{},"outputs":[]}
    ],"edges":[{"id":"start-end","source":"start","target":"end","sourceHandle":null,"targetHandle":null,"containerId":null,"points":[]}]},"editor":{"viewport":{"x":0,"y":0,"zoom":1},"annotations":[],"activeContainerPath":[]}})
}

#[tokio::test]
async fn published_workflow_is_compiled_and_draft_remains_independent() {
    use control_plane::{
        application_public_api::mapping::{
            ApplicationApiMappingConfig, WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
            WorkflowExtensionResponseMode,
        },
        flow::FlowService,
        ports::{ApplicationCompiledPlanRepository, ApplicationPublicationRepository},
    };
    let (store, workspace, actor) = support::seed_store().await;
    let source_application = Uuid::new_v4();
    let source_flow = Uuid::new_v4();
    let mut mapping = ApplicationApiMappingConfig::default_native();
    mapping.extension = Some(WorkflowExtensionApiConfig {
        slug: "portable-workflow".into(),
        method: WorkflowExtensionHttpMethod::Get,
        response_mode: WorkflowExtensionResponseMode::Sync,
    });
    let mut package = PortableTemplatePackage {
        i18n_entries: Vec::new(),
        release: None,
        mcp_bundle: None,
        schema_version: PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),
        pages: vec![],
        data_models: vec![],
        plugins: vec![],
        applications: vec![PortableApplication {
            id: source_application,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Extension),
            name: "Portable Workflow".into(),
            description: "".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
            flow_document: workflow_document(source_flow, "draft definition"),
            mapping: Some(mapping.clone()),
            published: Some(PortablePublication {
                flow_document: workflow_document(source_flow, "published definition"),
                mapping,
                api_enabled: true,
            }),
            schedule: None,
            dependency_issues: vec![],
        }],
    };
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(result.complete, "{:?}", result.failures);
    let application_id: Uuid = result.id_map[&source_application.to_string()]
        .parse()
        .unwrap();
    let publication = store
        .load_active_application_publication(application_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(publication.workspace_id, workspace.id);
    assert!(publication.api_enabled);
    assert_eq!(
        publication.extension_slug.as_deref(),
        Some("portable-workflow")
    );
    assert!(
        ApplicationCompiledPlanRepository::get_application_compiled_plan(
            &store,
            publication.compiled_plan_id
        )
        .await
        .unwrap()
        .is_some()
    );
    let editor = FlowService::new(store.clone())
        .get_or_create_editor_state(actor.id, application_id)
        .await
        .unwrap();
    assert_ne!(editor.flow.id, source_flow);
    assert_eq!(editor.draft.document["meta"]["name"], "draft definition");
    assert_eq!(
        publication.document_snapshot["meta"]["name"],
        "published definition"
    );
    assert_eq!(
        editor.draft.document["meta"]["flowId"],
        editor.flow.id.to_string()
    );
    assert_eq!(
        publication.document_snapshot["meta"]["flowId"],
        editor.flow.id.to_string()
    );
    let snapshot = store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap();
    let app = snapshot
        .applications
        .iter()
        .find(|a| a.id == application_id)
        .unwrap();
    assert_eq!(app.flow_document["meta"]["name"], "draft definition");
    assert_eq!(
        app.published.as_ref().unwrap().flow_document["meta"]["name"],
        "published definition"
    );
    package.applications[0].name = "Updated Workflow".into();
    package.applications[0].flow_document = workflow_document(source_flow, "updated draft");
    package.applications[0]
        .published
        .as_mut()
        .unwrap()
        .flow_document = workflow_document(source_flow, "updated publication");
    let repeated = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package)
        .await
        .unwrap();
    assert!(repeated.complete, "{:?}", repeated.failures);
    assert!(repeated.created.is_empty());
    assert!(repeated
        .updated
        .iter()
        .any(|item| item.kind == "application" && item.target_id == application_id.to_string()));
    let updated = store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap();
    let updated = updated
        .applications
        .iter()
        .find(|item| item.id == application_id)
        .unwrap();
    assert_eq!(updated.name, "Updated Workflow");
    assert_eq!(updated.flow_document["meta"]["name"], "updated draft");
    assert_eq!(
        updated.published.as_ref().unwrap().flow_document["meta"]["name"],
        "updated publication"
    );
}

#[tokio::test]
async fn visibility_merges_existing_target_grants_and_missing_role_blocks_all_writes() {
    use control_plane::{
        frontstage::{CreateFrontstagePageCommand, FrontstagePageService},
        role::{ReplaceRoleFrontstageRoutesCommand, RoleService},
    };
    let (store, workspace, actor) = support::seed_store().await;
    let existing = FrontstagePageService::new(store.clone())
        .create_page(CreateFrontstagePageCommand {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            title: Some("Existing".into()),
            icon: None,
            tooltip: None,
            parent_id: None,
            rank: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
            slug: Some("existing".into()),
        })
        .await
        .unwrap();
    RoleService::new(store.clone())
        .replace_frontstage_routes(ReplaceRoleFrontstageRoutesCommand {
            actor_user_id: actor.id,
            role_code: "member".into(),
            page_ids: vec![existing.page.id],
            tab_ids: vec![],
        })
        .await
        .unwrap();
    let mut package = fixture();
    package.pages[0].visibility_rules = vec![PortablePageVisibilityRule {
        role_code: "does-not-exist".into(),
        tab_id: None,
        visibility: domain::frontstage::FrontstagePageVisibility::Visible,
    }];
    let error = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("target_role_missing"));
    assert!(!store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap()
        .data_models
        .iter()
        .any(|m| m.code == "portable_orders"));
    // An unrelated hidden target rule must not be replaced by this visible-only package.
    sqlx::query("UPDATE frontstage_page_visibility_rules SET visibility='hidden' WHERE workspace_id=$1 AND page_id=$2 AND role_id=(SELECT id FROM roles WHERE workspace_id=$1 AND code='member')")
        .bind(workspace.id).bind(existing.page.id).execute(store.pool()).await.unwrap();
    package.pages[0].visibility_rules[0].role_code = "member".into();
    let source_page = package.pages[0].id;
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package)
        .await
        .unwrap();
    assert!(result.complete, "{:?}", result.failures);
    let page_id: Uuid = result.id_map[&source_page.to_string()].parse().unwrap();
    let view = RoleService::new(store)
        .get_frontstage_routes(actor.id, "member")
        .await
        .unwrap();
    assert!(view
        .rules
        .iter()
        .any(|r| r.page_id == Some(existing.page.id)
            && r.visibility == domain::frontstage::FrontstagePageVisibility::Hidden));
    assert!(view.rules.iter().any(|r| r.page_id == Some(page_id)));
}

#[tokio::test]
async fn repeat_install_updates_mapped_resources_and_preserves_target_only_model() {
    use control_plane::model_definition::{CreateModelDefinitionCommand, ModelDefinitionService};
    let (store, workspace, actor) = support::seed_store().await;
    let mut package = fixture();
    let first = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(first.complete, "{:?}", first.failures);
    let model_id: Uuid = first.id_map[&package.data_models[0].id.to_string()]
        .parse()
        .unwrap();
    let page_id: Uuid = first.id_map[&package.pages[0].id.to_string()]
        .parse()
        .unwrap();
    let field_id: Uuid = first.id_map[&package.data_models[0].fields[0].id.to_string()]
        .parse()
        .unwrap();
    let block_id = first.id_map[&package.pages[0].tabs[0].blocks[0].block_id].clone();
    let target_only = ModelDefinitionService::new(store.clone())
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: actor.id,
            scope_kind: domain::DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            external_capabilities: None,
            template_provider: "core".into(),
            template_code: "general".into(),
            template_version: "v1".into(),
            code: "target_only_orders".into(),
            title: "Target only".into(),
            description: None,
            status: Some(domain::DataModelStatus::Draft),
        })
        .await
        .unwrap();

    package.data_models[0].title = "Updated orders".into();
    package.data_models[0].fields[0].title = "Updated label".into();
    package.pages[0].title = Some("Updated page".into());
    package.pages[0].tabs[0].blocks[0].title = Some("Updated block".into());
    let preview = PortableTemplateService::new(store.clone())
        .preview(actor.id, &package)
        .await
        .unwrap();
    assert!(preview.valid, "{:?}", preview.failures);
    assert!(preview
        .effects
        .iter()
        .any(|effect| effect.kind == "data_model"
            && effect.action == "update"
            && effect.target_id.as_deref() == Some(model_id.to_string().as_str())));
    assert!(preview.effects.iter().any(|effect| effect.kind == "page"
        && effect.action == "update"
        && effect.target_id.as_deref() == Some(page_id.to_string().as_str())));

    let second = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(second.complete, "{:?}", second.failures);
    assert!(second.created.is_empty(), "{:?}", second.created);
    assert!(second
        .updated
        .iter()
        .any(|item| item.kind == "model_field" && item.target_id == field_id.to_string()));
    assert!(second
        .updated
        .iter()
        .any(|item| item.kind == "block" && item.target_id == block_id));
    let snapshot = store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap();
    assert!(snapshot
        .data_models
        .iter()
        .any(|item| item.id == target_only.id));
    assert_eq!(
        snapshot
            .data_models
            .iter()
            .find(|item| item.id == model_id)
            .unwrap()
            .title,
        "Updated orders"
    );
    assert_eq!(
        snapshot
            .data_models
            .iter()
            .find(|item| item.id == model_id)
            .unwrap()
            .fields
            .iter()
            .find(|item| item.id == field_id)
            .unwrap()
            .title,
        "Updated label"
    );
    assert_eq!(
        snapshot
            .pages
            .iter()
            .find(|item| item.id == page_id)
            .unwrap()
            .title
            .as_deref(),
        Some("Updated page")
    );
    assert_eq!(
        snapshot
            .pages
            .iter()
            .find(|item| item.id == page_id)
            .unwrap()
            .tabs[0]
            .blocks
            .iter()
            .find(|item| item.block_id == block_id)
            .unwrap()
            .title
            .as_deref(),
        Some("Updated block")
    );
}

#[tokio::test]
async fn release_ledger_preserves_failed_retry_immutable_digest_and_workspace_scope() {
    let (store, workspace, _) = support::seed_store().await;
    let id = "@taichuy/gateway-demo";
    store
        .record_application_template_release(workspace.id, id, 1, "digest", false)
        .await
        .unwrap();
    let rows = store
        .load_application_template_releases(workspace.id, id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].successful);
    let initial: (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT id, scope_id, created_at::text FROM application_template_releases WHERE workspace_id=$1 AND template_id=$2 AND release_version=1",
    )
    .bind(workspace.id)
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(initial.0.get_version_num(), 7);
    assert_eq!(initial.1, workspace.id);

    assert!(store
        .record_application_template_release(workspace.id, id, 1, "changed", true)
        .await
        .is_err());
    assert!(store
        .load_application_template_releases(Uuid::new_v4(), id)
        .await
        .unwrap()
        .is_empty());
    store
        .record_application_template_release(workspace.id, id, 1, "digest", true)
        .await
        .unwrap();
    store
        .record_application_template_release(workspace.id, id, 1, "digest", false)
        .await
        .unwrap();
    let rows = store
        .load_application_template_releases(workspace.id, id)
        .await
        .unwrap();
    assert!(
        rows[0].successful,
        "retry reservation must not erase prior success"
    );
    assert_eq!(rows[0].checksum, "digest");
    let after_retry: (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT id, scope_id, created_at::text FROM application_template_releases WHERE workspace_id=$1 AND template_id=$2 AND release_version=1",
    )
    .bind(workspace.id)
    .bind(id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        after_retry, initial,
        "retry must preserve platform identity and creation time"
    );
}

#[tokio::test]
async fn safe_merge_preserves_local_page_and_unknown_block_but_updates_and_adds_independent_items()
{
    use control_plane::frontstage::{FrontstagePageService, UpdateFrontstagePageMetadataCommand};
    let (store, workspace, actor) = support::seed_store().await;
    let mut package = fixture();
    let initial = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(initial.complete, "{:?}", initial.failures);
    let page_id: Uuid = initial.id_map[&package.pages[0].id.to_string()]
        .parse()
        .unwrap();
    FrontstagePageService::new(store.clone())
        .update_metadata(UpdateFrontstagePageMetadataCommand {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            page_id,
            title: Some(Some("Local title".into())),
            icon: None,
            tooltip: None,
            is_hidden: None,
            placement: None,
            content_presentation: None,
            slug: None,
        })
        .await
        .unwrap();
    let scope = template_baseline_scope(workspace.id, &package);
    let old = store.load_template_baselines(&scope).await.unwrap();
    // Historical packages had identity records but no per-item baseline.
    let old_block = package.pages[0].tabs[0].blocks[0].block_id.clone();
    sqlx::query("DELETE FROM application_template_resource_baselines WHERE workspace_id=$1 AND template_id=$2 AND kind='block' AND source_id=$3")
        .bind(workspace.id).bind(&scope.template_id).bind(&old_block).execute(store.pool()).await.unwrap();
    package.pages[0].title = Some("Upstream title".into());
    package.pages[0].tabs[0].blocks[0].source_code =
        "export default function Component() { return 'upstream overwrite'; }".into();
    package.data_models[0].title = "Upstream model".into();
    let mut added = package.pages[0].tabs[0].blocks[0].clone();
    added.block_id = Uuid::now_v7().to_string();
    added.code_ref = format!("new-code-{}", added.block_id);
    added.rank = "b".into();
    added.source_code =
        "export default function Component() { return 'new independent block'; }".into();
    package.pages[0].tabs[0].blocks.push(added.clone());
    let preview = PortableTemplateService::new(store.clone())
        .preview(actor.id, &package)
        .await
        .unwrap();
    assert!(preview.valid, "{:?}", preview.failures);
    assert!(preview.effects.iter().any(|e| e.kind == "page"
        && e.action == "skip"
        && e.reason.as_deref() == Some("user_modified")));
    assert!(preview.effects.iter().any(|e| e.source_id == old_block
        && e.action == "skip"
        && e.reason.as_deref() == Some("unknown_baseline")));
    assert!(preview
        .effects
        .iter()
        .any(|e| e.source_id == added.block_id && e.action == "create"));
    let installed = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(installed.complete, "{:?}", installed.failures);
    let snapshot = store
        .portable_template_snapshot(actor.id, workspace.id)
        .await
        .unwrap();
    let page = snapshot.pages.iter().find(|p| p.id == page_id).unwrap();
    assert_eq!(page.title.as_deref(), Some("Local title"));
    assert_eq!(page.tabs[0].blocks.len(), 2);
    assert!(!page.tabs[0]
        .blocks
        .iter()
        .any(|b| b.source_code.contains("upstream overwrite")));
    assert!(page.tabs[0]
        .blocks
        .iter()
        .any(|b| b.source_code.contains("new independent block")));
    assert!(snapshot
        .data_models
        .iter()
        .any(|m| m.title == "Upstream model"));
    let after = store.load_template_baselines(&scope).await.unwrap();
    let page_key = TemplateResourceKey {
        kind: "page".into(),
        source_id: package.pages[0].id.to_string(),
    };
    assert_eq!(
        old.iter().find(|b| b.key == page_key).unwrap(),
        after.iter().find(|b| b.key == page_key).unwrap()
    );
    assert!(!after
        .iter()
        .any(|b| b.key.kind == "block" && b.key.source_id == old_block));
}

#[tokio::test]
async fn scoped_native_transaction_savepoints_rollback_and_editor_write_exclusion() {
    use control_plane::frontstage::{FrontstagePageService, UpdateFrontstagePageMetadataCommand};
    let (store, workspace, actor) = support::seed_store().await;
    let package = fixture();
    let installed = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package.clone())
        .await
        .unwrap();
    assert!(installed.complete, "{:?}", installed.failures);
    let page_id: Uuid = installed.id_map[&package.pages[0].id.to_string()]
        .parse()
        .unwrap();
    let transaction = store.begin_portable_template_transaction().await.unwrap();
    // The native owner opens and commits its own transaction, which must be a savepoint.
    FrontstagePageService::new(transaction.repository.clone())
        .update_metadata(UpdateFrontstagePageMetadataCommand {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            page_id,
            title: Some(Some("Uncommitted".into())),
            icon: None,
            tooltip: None,
            is_hidden: None,
            placement: None,
            content_presentation: None,
            slug: None,
        })
        .await
        .unwrap();
    assert_eq!(
        FrontstagePageRepository::get_frontstage_page(&store, workspace.id, page_id)
            .await
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Portable")
    );
    let editing_store = store.clone();
    let mut editor = tokio::spawn(async move {
        FrontstagePageService::new(editing_store)
            .update_metadata(UpdateFrontstagePageMetadataCommand {
                actor_user_id: actor.id,
                workspace_id: workspace.id,
                page_id,
                title: Some(Some("Concurrent editor".into())),
                icon: None,
                tooltip: None,
                is_hidden: None,
                placement: None,
                content_presentation: None,
                slug: None,
            })
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut editor)
            .await
            .is_err(),
        "ordinary editor must wait for real table write lock"
    );
    let scoped_pool = transaction.repository.pool().clone();
    drop(transaction); // cancellation/Drop must rollback, never commit the native savepoint.
    tokio::time::timeout(std::time::Duration::from_secs(5), scoped_pool.close_event())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), &mut editor)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        FrontstagePageRepository::get_frontstage_page(&store, workspace.id, page_id)
            .await
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Concurrent editor")
    );
}

#[tokio::test]
async fn transaction_connection_loss_cannot_reconnect_and_commit_a_partial_batch() {
    let (store, _, _) = support::seed_store().await;
    let mut transaction = store.begin_portable_template_transaction().await.unwrap();
    let connection = transaction.repository.pool().acquire().await.unwrap();
    // Explicitly discard the physical session without altering the shared test store.
    connection.close().await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        transaction.guard.commit(),
    )
    .await;
    assert!(
        !matches!(result, Ok(Ok(()))),
        "lost outer transaction must never commit a replacement session"
    );
    transaction.guard.rollback().await.unwrap();
}

fn mcp_template_fixture() -> domain::McpBundlePackage {
    serde_json::from_value(json!({
        "manifest":{"schema_version":"1flowbase.mcp.bundle/v2","organization":"test","bundle_id":"template","bundle_version":"1.0.0","locale":"en_US","minimum_host_version":"1.0.0","exported_from_system_version":"1.0.0","exported_at":"2026-10-10T00:00:00Z","files":[]},
        "connections":[],
        "tools":[{"tool_id":"shared_template_tool","name":"Tool","short_description":"Tool","full_description":"Tool","interface_id":"workspace_list","parameter_schema_snapshot":{},"result_schema_snapshot":{},"input_mapping":{},"output_mapping":{},"risk_level_snapshot":"low","status":"enabled"}],
        "instances":[{"instance_id":"1flowbase","name":"Official","description_short":null,"status":"enabled","default_entry_path":"/","groups":[{"path":"/group","display_name":"Group","description_short":null,"enabled":true,"sort_order":0}],"bindings":[{"group_path":"/group","tool_id":"shared_template_tool","display_alias":null,"visible":true,"sort_order":0}],"discovery_policy":{"list_default_limit":20,"list_max_depth":8,"list_regex_enabled":false,"list_regex_max_length":128,"list_return_fields":[]}}]
    })).unwrap()
}
fn mcp_template_catalog() -> Vec<domain::McpInterfaceCatalogEntry> {
    vec![domain::McpInterfaceCatalogEntry {
        interface_id: "workspace_list".into(),
        source: domain::McpInterfaceCatalogSource::StaticApi,
        method: "GET".into(),
        path: "/api/workspaces".into(),
        name: "Workspaces".into(),
        short_description: "Workspaces".into(),
        parameter_descriptors: vec![],
        parameter_schema: json!({}),
        result_schema: json!({}),
        permission_code: None,
        security: json!([]),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }]
}
#[tokio::test]
async fn mcp_template_updates_independent_group_preserving_local_instance_and_record_ids() {
    let (store, workspace, actor) = support::seed_store().await;
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "mcp-safe".into(),
    };
    let mut package = mcp_template_fixture();
    let catalog = mcp_template_catalog();
    let first = mcp_merge::install(&store, &scope, actor.id, &package, &catalog, "1.0.0")
        .await
        .unwrap();
    assert!(first.failures.is_empty(), "{:?}", first.failures);
    assert_eq!(first.created.len(), 5);
    let instance_id: Uuid = sqlx::query_scalar(
        "select id from mcp_instances where workspace_id=$1 and instance_id='1flowbase'",
    )
    .bind(workspace.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let group_id: Uuid = sqlx::query_scalar(
        "select id from mcp_groups where instance_record_id=$1 and path='/group'",
    )
    .bind(instance_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let binding_id: Uuid =
        sqlx::query_scalar("select id from mcp_tool_bindings where instance_record_id=$1")
            .bind(instance_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    sqlx::query("update mcp_instances set name='Local' where id=$1")
        .bind(instance_id)
        .execute(store.pool())
        .await
        .unwrap();
    package.instances[0].name = "New official".into();
    package.instances[0].groups[0].display_name = "New group".into();
    let preview = mcp_merge::preview(&store, &scope, actor.id, &package, &catalog, "1.0.0")
        .await
        .unwrap();
    assert!(preview
        .effects
        .iter()
        .any(|e| e.kind == "mcp_instance" && e.reason.as_deref() == Some("user_modified")));
    let next = mcp_merge::install(&store, &scope, actor.id, &package, &catalog, "1.0.0")
        .await
        .unwrap();
    assert!(next.failures.is_empty(), "{:?}", next.failures);
    assert_eq!(next.updated.len(), 1);
    assert_eq!(next.updated[0].kind, "mcp_group");
    let name: String = sqlx::query_scalar("select name from mcp_instances where id=$1")
        .bind(instance_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(name, "Local");
    let group: (Uuid, String) = sqlx::query_as(
        "select id,display_name from mcp_groups where instance_record_id=$1 and path='/group'",
    )
    .bind(instance_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(group, (group_id, "New group".into()));
    let retained: Uuid =
        sqlx::query_scalar("select id from mcp_tool_bindings where instance_record_id=$1")
            .bind(instance_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(retained, binding_id);
    let baseline = store.load_template_baselines(&scope).await.unwrap();
    assert_eq!(
        baseline
            .iter()
            .find(|b| b.key.kind == "mcp_instance")
            .unwrap()
            .generation,
        1
    );
}
#[tokio::test]
async fn mcp_template_legacy_instance_reset_keeps_unknown_shared_tool_unowned() {
    use control_plane::mcp_bundle::{ExportMcpBundleCommand, ImportMcpBundleCommand};
    use control_plane::mcp_management::McpManagementService;
    let (store, workspace, actor) = support::seed_store().await;
    let service = McpManagementService::new(store.clone());
    let catalog = mcp_template_catalog();
    service
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: actor.id,
            package: mcp_template_fixture(),
            interface_catalog: catalog.clone(),
            current_system_version: "1.0.0".into(),
        })
        .await
        .unwrap();
    let package = service
        .export_bundle(ExportMcpBundleCommand {
            actor_user_id: actor.id,
            organization: "test".into(),
            bundle_id: "template".into(),
            bundle_version: "1.0.0".into(),
            locale: "en_US".into(),
            current_system_version: "1.0.0".into(),
        })
        .await
        .unwrap();
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "legacy-reset".into(),
    };
    let existing = mcp_merge::install(&store, &scope, actor.id, &package, &catalog, "1.0.0")
        .await
        .unwrap();
    assert!(existing.created.is_empty());
    assert!(existing.updated.is_empty());
    assert!(store
        .load_template_baselines(&scope)
        .await
        .unwrap()
        .is_empty());
    let tool:(Uuid,i32)=sqlx::query_as("select id,revision from mcp_tools where workspace_id=$1 and tool_id='shared_template_tool'").bind(workspace.id).fetch_one(store.pool()).await.unwrap();
    sqlx::query("delete from mcp_instances where workspace_id=$1 and instance_id='1flowbase'")
        .bind(workspace.id)
        .execute(store.pool())
        .await
        .unwrap();
    let reset = mcp_merge::install(&store, &scope, actor.id, &package, &catalog, "1.0.0")
        .await
        .unwrap();
    assert!(reset.failures.is_empty(), "{:?}", reset.failures);
    assert_eq!(reset.created.len(), 4);
    let retained:(Uuid,i32)=sqlx::query_as("select id,revision from mcp_tools where workspace_id=$1 and tool_id='shared_template_tool'").bind(workspace.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(retained, tool);
    assert!(store
        .load_template_baselines(&scope)
        .await
        .unwrap()
        .iter()
        .all(|b| b.key.kind != "mcp_tool"));
}

#[tokio::test]
async fn mcp_owner_failure_rolls_back_tools_and_baseline_intents_together() {
    let (store, workspace, actor) = support::seed_store().await;
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "mcp-rollback".into(),
    };
    sqlx::query("create function reject_template_mcp_group() returns trigger language plpgsql as $$ begin raise exception 'template fixture group failure'; end $$")
        .execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_template_mcp_group before insert on mcp_groups for each row execute function reject_template_mcp_group()")
        .execute(store.pool()).await.unwrap();
    let result = mcp_merge::install(
        &store,
        &scope,
        actor.id,
        &mcp_template_fixture(),
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(!result.failures.is_empty());
    assert!(result.created.is_empty());
    assert!(store
        .load_template_baselines(&scope)
        .await
        .unwrap()
        .is_empty());
    let count: i64 = sqlx::query_scalar(
        "select count(*) from mcp_tools where workspace_id=$1 and tool_id='shared_template_tool'",
    )
    .bind(workspace.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("drop trigger reject_template_mcp_group on mcp_groups")
        .execute(store.pool())
        .await
        .unwrap();
    let retry = mcp_merge::install(
        &store,
        &scope,
        actor.id,
        &mcp_template_fixture(),
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(retry.failures.is_empty(), "{:?}", retry.failures);
    assert_eq!(retry.created.len(), 5);
}
