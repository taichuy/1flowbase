mod support;
use control_plane::{
    portable_template::*,
    ports::{ApplicationRepository, FrontstagePageRepository, ModelDefinitionRepository},
};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn fixture() -> PortableTemplatePackage {
    let model_id = Uuid::new_v4();
    let page_id = Uuid::new_v4();
    let tab_id = Uuid::new_v4();
    let block_id = Uuid::new_v4().to_string();
    PortableTemplatePackage {schema_version:PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),plugins:vec![],mcp_bundle:None,applications:vec![],
        data_models:vec![PortableDataModel {id:model_id,code:"portable_orders".into(),title:"Portable orders".into(),description:None,scope_kind:domain::DataModelScopeKind::Workspace,template_provider:"core".into(),template_code:"general".into(),template_version:"v1".into(),status:domain::DataModelStatus::Published,builtin:false,fields:vec![PortableModelField {id:Uuid::new_v4(),code:"label".into(),title:"Label".into(),description:None,field_kind:domain::ModelFieldKind::String,is_system:false,is_required:false,api_required:false,is_unique:false,default_value:None,display_interface:None,display_options:json!({"page_id":page_id}),relation_target_model_id:None,relation_options:json!({})}]}],
        pages:vec![PortablePage {id:page_id,parent_id:None,kind:domain::FrontstagePageKind::Page,title:Some("Portable".into()),icon:None,tooltip:None,is_hidden:false,placement:domain::frontstage::FrontstageNavigationPlacement::Topbar,content_presentation:domain::frontstage::FrontstagePageContentPresentation::Single,slug:Some("portable".into()),rank:"a".into(),visibility_rules:vec![],tabs:vec![PortableTab {id:tab_id,title:Some("Default".into()),rank:"a".into(),is_default:true,route_segment:None,document_root_uid:format!("source-tab-{tab_id}"),document_payload:json!({"root":format!("source-tab-{tab_id}"),"model":model_id,"page":page_id,"tab":tab_id}),blocks:vec![PortableBlock {block_id:block_id.clone(),parent_block_id:None,rank:"a".into(),presentation:domain::frontstage::FrontstageBlockPresentation::Inline,title:Some("Orders".into()),description:None,code_ref:format!("frontstage.block.{block_id}"),schema_version:1,input_mapping:BTreeMap::new(),output_mapping:BTreeMap::new(),runtime_descriptor:json!({"props":{"model_id":model_id}}),source_code:format!("export default function Component() {{ return '{model_id}:{page_id}:{tab_id}:{block_id}'; }}")}]}]}]}
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
async fn owner_failure_returns_created_resources_and_stops_before_pages() {
    let (store, workspace, actor) = support::seed_store().await;
    let mut package = fixture();
    package.data_models[0].fields[0].code = "id".into();
    let source_model = package.data_models[0].id;
    let result = PortableTemplateInstallService::new(store.clone())
        .install(actor.id, package)
        .await
        .unwrap();
    assert!(!result.complete);
    assert!(!result.failures.is_empty());
    assert!(result
        .created
        .iter()
        .any(|r| r.kind == "data_model" && r.source_id == source_model.to_string()));
    assert!(!result.created.iter().any(|r| r.kind == "page"));
    let id = result.id_map[&source_model.to_string()].parse().unwrap();
    assert!(
        ModelDefinitionRepository::get_model_definition(&store, workspace.id, id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        ApplicationRepository::load_actor_context_for_user(&store, actor.id)
            .await
            .unwrap()
            .is_root
    );
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
        .any(|r| r.page_id == Some(existing.page.id)));
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
