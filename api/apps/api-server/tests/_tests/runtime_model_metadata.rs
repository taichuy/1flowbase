use super::*;

const DEFINITIONS: &str = "/api/console/settings/data-models/model-definitions";
const METADATA: &str = "/api/runtime/models/data_blocks/metadata";

async fn create_model(app: &Router, cookie: &str, csrf: &str) -> String {
    let (status, model) = json_request(
        app,
        "POST",
        DEFINITIONS,
        cookie,
        csrf,
        json!({
            "scope_kind":"workspace", "template_provider":"core", "template_code":"general",
            "template_version":"v1", "code":"data_blocks", "title":"Data Blocks", "status":"draft"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{model}");
    let id = model["data"]["id"].as_str().unwrap().to_owned();
    let (status, field) = json_request(
        app,
        "POST",
        &format!("{DEFINITIONS}/{id}/fields"),
        cookie,
        csrf,
        json!({
            "code":"title", "title":"Title", "description":"Record title", "field_kind":"string",
            "is_required":true, "display_options":{"placeholder":"Title"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{field}");
    let (status, published) = json_request(
        app,
        "PATCH",
        &format!("{DEFINITIONS}/{id}"),
        cookie,
        csrf,
        json!({"status":"published"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    id
}

async fn data_policy(app: &Router, cookie: &str, csrf: &str, can_view: bool, write: bool) {
    let (status, payload) = json_request(app, "PUT", "/api/console/settings/roles/member/data-policy", cookie, csrf, json!({
        "default_policy": {"can_view":can_view,"can_create":write,"can_update":write,"can_delete":write,
            "default_view_scope":"scope_all","default_update_scope":"scope_all","default_delete_scope":"scope_all"},
        "model_policies":[]
    })).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
}

#[tokio::test]
async fn runtime_metadata_and_visible_blocks_use_data_grants_without_design_permission() {
    let fixture = fixture().await;
    let app = &fixture.app;
    assert_eq!(get(app, METADATA, "").await.0, StatusCode::UNAUTHORIZED);
    let (root, csrf) = login(app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(app, &root).await;
    let model_id = create_model(app, &root, &csrf).await;
    let (status, target) = json_request(
        app,
        "POST",
        DEFINITIONS,
        &root,
        &csrf,
        json!({
            "scope_kind":"workspace", "template_provider":"core", "template_code":"general",
            "template_version":"v1", "code":"data_targets", "title":"Targets", "status":"published"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{target}");
    let target_id = target["data"]["id"].as_str().unwrap();
    let (status, relation) = json_request(app, "POST", &format!("{DEFINITIONS}/{model_id}/fields"), &root, &csrf, json!({
        "code":"target", "title":"Target", "field_kind":"many_to_one", "relation_target_model_id":target_id
    })).await;
    assert_eq!(status, StatusCode::CREATED, "{relation}");
    let (page_id, tab_id, _) = create_page(app, &root, &csrf, workspace_id).await;
    let block =
        create_block_node(app, &root, &csrf, workspace_id, page_id, tab_id, "Records").await;
    create_member(app, &root, &csrf, "data-reader", "temp-pass").await;
    let (status, payload) = json_request(
        app,
        "PUT",
        "/api/console/settings/roles/member/frontstage-routes",
        &root,
        &csrf,
        json!({"page_ids":[page_id],"tab_ids":[tab_id]}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{payload}");
    data_policy(app, &root, &csrf, true, true).await;
    let (reader, token) = login(app, "data-reader", "temp-pass").await;
    // The user cannot manage model definitions or enumerate the design catalog.
    for path in [
        DEFINITIONS,
        "/api/console/frontstage/interface-capabilities",
        "/api/console/frontstage/interface-capabilities/runtime.models.metadata.get",
    ] {
        assert_eq!(
            get(app, path, &reader).await.0,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    let (status, metadata) = get(app, METADATA, &reader).await;
    assert_eq!(status, StatusCode::OK, "{metadata}");
    assert_eq!(metadata["data"]["model_id"], model_id);
    assert_eq!(metadata["data"]["model_code"], "data_blocks");
    assert_eq!(metadata["data"]["scope_id"], workspace_id.to_string());
    assert_eq!(metadata["data"]["record_id_field"], "id");
    assert_eq!(
        metadata["data"]["record_capabilities"],
        json!({"can_list":true,"can_get":true,"can_create":true,"can_update":true,"can_delete":true})
    );
    assert!(metadata["data"].get("title").is_none());
    for private in [
        "physical_table_name",
        "physical_column_name",
        "template_provider",
        "data_source_instance_id",
        "external_resource_key",
    ] {
        assert!(!metadata["data"].to_string().contains(private), "{private}");
    }
    let title = metadata["data"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["code"] == "title")
        .unwrap();
    assert_eq!(title["title"], "Title");
    assert_eq!(title["description"], "Record title");
    assert_eq!(title["is_writable"], true);
    assert_eq!(title["display_options"], json!({"placeholder":"Title"}));
    let target_field = metadata["data"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["code"] == "target")
        .unwrap();
    assert_eq!(target_field["relation_target_model_id"], target_id);
    assert_eq!(target_field["relation_target_model_code"], "data_targets");
    let catalog = catalog_entry(app, &root, workspace_id, "runtime.models.metadata.get").await;
    assert_eq!(catalog["path"], "/api/runtime/models/{model_code}/metadata");
    // Metadata's read-only target still requires the outer callable POST's CSRF token.
    let (status, denied) = dispatch(
        app,
        &reader,
        "",
        workspace_id,
        page_id,
        tab_id,
        &block,
        "GET",
        "/api/runtime/models/{model_code}/metadata",
        json!({"path":{"model_code":"data_blocks"}}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    let (status, projected) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "GET",
        "/api/runtime/models/{model_code}/metadata",
        json!({"path":{"model_code":"data_blocks"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{projected}");
    assert_eq!(projected["data"], metadata["data"]);

    let (status, created) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "POST",
        "/api/runtime/models/data_blocks/create",
        json!({"body":{"title":"created"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["data"]["id"].as_str().unwrap();
    let (status, updated) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "PATCH",
        "/api/runtime/models/data_blocks/update/{id}",
        json!({"path":{"id":id},"body":{"title":"updated"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["data"]["title"], "updated");
    let (status, listed) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "GET",
        "/api/runtime/models/data_blocks/list",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["data"]["total"], 1);
    assert_eq!(
        dispatch(
            app,
            &reader,
            "",
            workspace_id,
            page_id,
            tab_id,
            &block,
            "GET",
            "/api/runtime/models/data_blocks/list",
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        dispatch(
            app,
            &reader,
            &token,
            workspace_id,
            page_id,
            tab_id,
            &block,
            "GET",
            "/api/console/settings/members",
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        dispatch(
            app,
            &reader,
            &token,
            workspace_id,
            page_id,
            Uuid::now_v7(),
            &block,
            "GET",
            "/api/runtime/models/data_blocks/list",
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    // Read-only role policy must affect both advertised capabilities and the same target write.
    data_policy(app, &root, &csrf, true, false).await;
    let (reader, token) = login(app, "data-reader", "temp-pass").await;
    let (status, readonly) = get(app, METADATA, &reader).await;
    assert_eq!(status, StatusCode::OK, "{readonly}");
    assert_eq!(
        readonly["data"]["record_capabilities"],
        json!({"can_list":true,"can_get":true,"can_create":false,"can_update":false,"can_delete":false})
    );
    for (method, path, request) in [
        (
            "POST",
            "/api/runtime/models/data_blocks/create",
            json!({"body":{"title":"denied"}}),
        ),
        (
            "PATCH",
            "/api/runtime/models/data_blocks/update/{id}",
            json!({"path":{"id":id},"body":{"title":"denied"}}),
        ),
        (
            "DELETE",
            "/api/runtime/models/data_blocks/delete/{id}",
            json!({"path":{"id":id}}),
        ),
    ] {
        assert_eq!(
            dispatch(
                app,
                &reader,
                &token,
                workspace_id,
                page_id,
                tab_id,
                &block,
                method,
                path,
                request
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "{method}"
        );
    }
    assert_eq!(
        get(
            app,
            &format!("/api/runtime/models/data_blocks/get/{id}"),
            &reader
        )
        .await
        .1["data"]["title"],
        "updated"
    );
    data_policy(app, &root, &csrf, true, true).await;
    let (reader, token) = login(app, "data-reader", "temp-pass").await;
    let (status, deleted) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "DELETE",
        "/api/runtime/models/data_blocks/delete/{id}",
        json!({"path":{"id":id}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["data"], json!({"deleted":true}));
    assert_eq!(
        get(
            app,
            &format!("/api/runtime/models/data_blocks/get/{id}"),
            &reader
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // Hidden page cannot be used as a dispatch envelope even when direct model reads are legal.
    let (status, payload) = json_request(
        app,
        "PUT",
        "/api/console/settings/roles/member/frontstage-routes",
        &root,
        &csrf,
        json!({"page_ids":[],"tab_ids":[]}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{payload}");
    let (reader, token) = login(app, "data-reader", "temp-pass").await;
    assert_eq!(get(app, METADATA, &reader).await.0, StatusCode::OK);
    let (status, hidden_metadata) = dispatch(
        app,
        &reader,
        &token,
        workspace_id,
        page_id,
        tab_id,
        &block,
        "GET",
        "/api/runtime/models/{model_code}/metadata",
        json!({"path":{"model_code":"data_blocks"}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{hidden_metadata}");
    assert_eq!(
        dispatch(
            app,
            &reader,
            &token,
            workspace_id,
            page_id,
            tab_id,
            &block,
            "GET",
            "/api/runtime/models/data_blocks/list",
            json!({})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    data_policy(app, &root, &csrf, false, false).await;
    let (reader, _) = login(app, "data-reader", "temp-pass").await;
    let (status, denied) = get(app, METADATA, &reader).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "data_model_scope_not_granted");
    let (status, disabled) = json_request(
        app,
        "PATCH",
        &format!("{DEFINITIONS}/{model_id}"),
        &root,
        &csrf,
        json!({"status":"disabled"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{disabled}");
    let (status, disabled) = get(app, METADATA, &root).await;
    assert_eq!(status, StatusCode::CONFLICT, "{disabled}");
    assert_eq!(disabled["code"], "model_disabled");
}

#[tokio::test]
async fn runtime_metadata_does_not_resolve_another_workspaces_model() {
    let (state, config, database) = fixture_state().await;
    let app = app_with_state_and_config(Arc::clone(&state), &config);
    let (root, csrf) = login(&app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&app, &root).await;
    create_model(&app, &root, &csrf).await;
    let registry = state.runtime_engine.registry();
    let mut model = registry
        .get(
            domain::DataModelScopeKind::Workspace,
            workspace_id,
            "data_blocks",
        )
        .unwrap();
    registry.remove(model.scope_kind, model.scope_id, &model.model_code);
    model.scope_id = Uuid::now_v7();
    registry.upsert(model);
    let (status, missing) = get(&app, METADATA, &root).await;
    assert_eq!(status, StatusCode::CONFLICT, "{missing}");
    assert_eq!(missing["code"], "runtime_model_unavailable");
    drop(database);
}

#[tokio::test]
async fn runtime_metadata_reports_source_record_identity_without_guessing() {
    let (state, config, database) = fixture_state().await;
    let app = app_with_state_and_config(Arc::clone(&state), &config);
    let (root, csrf) = login(&app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&app, &root).await;
    create_model(&app, &root, &csrf).await;
    let registry = state.runtime_engine.registry();
    let mut model = registry
        .get(
            domain::DataModelScopeKind::Workspace,
            workspace_id,
            "data_blocks",
        )
        .unwrap();
    let key = model
        .fields
        .iter_mut()
        .find(|field| field.physical_column_name == "id")
        .unwrap();
    key.code = "record_key".into();
    registry.upsert(model.clone());
    let (status, native) = get(&app, METADATA, &root).await;
    assert_eq!(status, StatusCode::OK, "{native}");
    assert_eq!(native["data"]["record_id_field"], "record_key");
    model.source_kind = domain::DataModelSourceKind::ExternalSource;
    for field in &mut model.fields {
        field.is_unique = false;
    }
    registry.upsert(model.clone());
    let (status, unidentified) = get(&app, METADATA, &root).await;
    assert_eq!(status, StatusCode::OK, "{unidentified}");
    assert!(unidentified["data"]["record_id_field"].is_null());
    assert_eq!(
        unidentified["data"]["record_capabilities"],
        json!({"can_list":true,"can_get":false,"can_create":true,"can_update":false,"can_delete":false})
    );
    let key = model
        .fields
        .iter_mut()
        .find(|field| field.code == "record_key")
        .unwrap();
    key.is_unique = true;
    key.external_field_key = Some("external_record_key".into());
    registry.upsert(model.clone());
    let (status, external) = get(&app, METADATA, &root).await;
    assert_eq!(status, StatusCode::OK, "{external}");
    assert_eq!(external["data"]["record_id_field"], "record_key");
    model
        .fields
        .iter_mut()
        .find(|field| field.code == "title")
        .unwrap()
        .is_unique = true;
    registry.upsert(model);
    let (status, ambiguous) = get(&app, METADATA, &root).await;
    assert_eq!(status, StatusCode::OK, "{ambiguous}");
    assert!(ambiguous["data"]["record_id_field"].is_null());
    assert_eq!(
        ambiguous["data"]["record_capabilities"]["can_update"],
        false
    );
    drop(database);
}
