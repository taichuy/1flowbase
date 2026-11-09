use super::*;
async fn expect(
    f: &Fixture,
    root: bool,
    method: &str,
    path: &str,
    body: Option<Value>,
    status: StatusCode,
) -> Value {
    let (actual, result) = f.request(root, method, path, body).await;
    assert_eq!(actual, status, "{method} {path}: {result}");
    result["data"].clone()
}
async fn get(f: &Fixture, root: bool, id: &str, status: StatusCode) -> Value {
    expect(f, root, "GET", &format!("/records/{id}"), None, status).await
}
async fn capabilities(f: &Fixture, root: bool, id: &str) -> Value {
    let (status, result) = http(
        &f.app,
        if root { &f.root } else { &f.member },
        "POST",
        "/api/console/settings/roles/record-capabilities",
        Some(json!({"plugin_code":CODE,"resource_code":"records","record_ids":[id]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result["data"]["items"][0].clone()
}
fn resource_reference(result: &Value, payload_field: &str) -> String {
    let resource = result["result"]["content"]
        .as_array()
        .expect("native MCP content")
        .iter()
        .find(|block| block["type"] == "resource" && block["resource"].get(payload_field).is_some())
        .expect("fixture must emit the requested text or binary resource");
    let uri = resource["resource"]["uri"].as_str().unwrap();
    assert!(
        uri.starts_with(extension_contracts::ResourceReference::PREFIX),
        "Host must wrap a replayable resource reference"
    );
    uri.to_owned()
}
async fn replay(
    f: &Fixture,
    uri: &str,
) -> Result<
    control_plane::file_management::ImportedResourceFile,
    extension_contracts::FileImportError,
> {
    let generation = f.publisher.current_generation().unwrap();
    let actor = f.actor(false).await;
    let user = generation
        .store
        .find_user_by_id(actor.user_id)
        .await
        .unwrap()
        .unwrap();
    crate::routes::mcp_protocol::resources::replay_for_actor(
        &generation,
        &actor,
        user,
        uri,
        Some(&["taichuy".to_owned()]),
        None,
    )
    .await
}
async fn event_directory_uses_the_published_managed_stream_control_owner(f: &Fixture) {
    let (status, response) = http(
        &f.app,
        &f.root,
        "GET",
        "/api/console/events/api-operations",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let rows = response["data"].as_array().unwrap();
    let upgrade_id = "record_fixture.stream.stream-upgrade";
    let upgrade = rows
        .iter()
        .find(|row| row["operation_id"] == upgrade_id)
        .expect("installed managed upgrade must be in the complete directory");
    assert_eq!(upgrade["method"], "GET");
    assert_eq!(
        upgrade["before"],
        json!({"status":"unavailable","reason":"protocol_control"})
    );
    assert_eq!(upgrade["completion"], upgrade["before"]);
    let business = rows
        .iter()
        .find(|row| row["operation_id"] == "record_fixture.stream")
        .unwrap();
    assert_eq!(business["method"], "POST");
    assert_eq!(business["path"], upgrade["path"]);
    assert_eq!(business["before"]["status"], "available");
    assert_eq!(business["completion"]["status"], "available");
    let (status, detail) = http(
        &f.app,
        &f.root,
        "GET",
        &format!("/api/console/events/api-operations/{upgrade_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["data"]["before"], upgrade["before"]);
    assert_eq!(detail["data"]["completion"], upgrade["completion"]);
    assert!(detail["data"]["before_schema"].is_null());
    assert!(detail["data"]["completion_schema"].is_null());
    for phase in ["before", "completion"] {
        let (status, rejected) = http(&f.app, &f.root, "POST", "/api/console/events", Some(json!({"name":"Must reject managed upgrade control","description":"protocol control has no business event contract","enabled":true,"interface_event":{"operation_id":upgrade_id,"phase":phase}}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{rejected}");
        assert_eq!(rejected["code"], "api_event_scope");
    }
}

pub(super) async fn run(f: &Fixture) {
    event_directory_uses_the_published_managed_stream_control_owner(f).await;
    // No policy header preserves all current operation eligibility in both protocols.
    let initial = expect(
        f,
        false,
        "GET",
        "/records?limit=100&offset=0",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(initial["total"], 3);
    assert_eq!(initial["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        get(f, false, &f.alpha, StatusCode::OK).await["name"],
        "alpha"
    );
    assert_eq!(
        mcp_success(&f.mcp(false, "get", json!({"record_id":f.alpha})).await)["id"],
        f.alpha
    );
    expect(
        f,
        false,
        "PUT",
        &format!("/records/{}", f.alpha),
        Some(json!({"name":"alpha"})),
        StatusCode::OK,
    )
    .await;
    let baseline = expect(
        f,
        false,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;
    let baseline_handle = baseline["session_id"].as_str().unwrap();
    let result = expect(
        f,
        false,
        "POST",
        &format!("/sessions/{baseline_handle}/command"),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(result["record_id"], f.alpha);
    expect(
        f,
        false,
        "POST",
        "/records",
        Some(json!({"name":"delete-unrestricted"})),
        StatusCode::OK,
    )
    .await;
    let listed = expect(f, false, "GET", "/records?limit=100", None, StatusCode::OK).await;
    let temporary = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "delete-unrestricted")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    expect(
        f,
        false,
        "DELETE",
        &format!("/records/{temporary}"),
        None,
        StatusCode::OK,
    )
    .await;
    let (status, unmapped) = f.request(false, "POST", "/unmapped", Some(json!({}))).await;
    assert_ne!(
        status,
        StatusCode::OK,
        "an unmapped operation must not read the protected collection: {unmapped}"
    );
    assert!(
        unmapped.get("data").is_none(),
        "unmapped collection callback exposed rows: {unmapped}"
    );

    // Hidden rows are removed in SQL before count/offset/limit; root still sees each row.
    f.policy(&f.beta, "restricted", &[]).await;
    f.policy(&f.gamma, "restricted", &[]).await;
    let first = expect(
        f,
        false,
        "GET",
        "/records?limit=1&offset=0",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(first["total"], 1);
    assert_eq!(first["items"], json!([{"id":f.alpha,"name":"alpha"}]));
    let beyond = expect(
        f,
        false,
        "GET",
        "/records?limit=1&offset=1",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(beyond["total"], 1);
    assert_eq!(beyond["items"], json!([]));
    let mcp_page = f.mcp(false, "list", json!({"limit":1,"offset":1})).await;
    assert_eq!(mcp_success(&mcp_page)["total"], 1);
    assert_eq!(mcp_success(&mcp_page)["items"], json!([]));
    get(f, false, &f.beta, StatusCode::FORBIDDEN).await;
    get(f, false, &f.beta.to_uppercase(), StatusCode::FORBIDDEN).await;
    mcp_denied(&f.mcp(false, "get", json!({"record_id":f.beta})).await);
    assert_eq!(get(f, true, &f.beta, StatusCode::OK).await["name"], "beta");
    expect(
        f,
        false,
        "DELETE",
        &format!("/records/{}", f.beta),
        None,
        StatusCode::FORBIDDEN,
    )
    .await;
    assert_eq!(get(f, true, &f.beta, StatusCode::OK).await["name"], "beta");

    // A present empty restricted policy denies every nonroot action, while root stays exempt.
    f.policy(&f.alpha, "restricted", &[]).await;
    let hidden = expect(f, false, "GET", "/records", None, StatusCode::OK).await;
    assert_eq!(hidden["total"], 0);
    assert_eq!(hidden["items"], json!([]));
    assert_eq!(
        capabilities(f, false, &f.alpha).await,
        json!({"record_id":f.alpha,"allowed_actions":[],"can_manage_permissions":false})
    );
    let root_caps = capabilities(f, true, &f.alpha).await;
    assert_eq!(root_caps["can_manage_permissions"], true);
    for action in ["view", "update", "open", "delete"] {
        assert!(root_caps["allowed_actions"]
            .as_array()
            .unwrap()
            .contains(&json!(action)));
    }
    get(f, true, &f.alpha, StatusCode::OK).await;
    expect(
        f,
        false,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    mcp_denied(&f.mcp(false, "open", json!({"record_id":f.alpha})).await);
    let management_path=format!("/api/console/settings/roles/record-permissions?plugin_code={CODE}&resource_code=records&record_id={}",f.alpha);
    assert_eq!(
        http(&f.app, &f.member, "GET", &management_path, None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http(
            &f.app,
            &f.member,
            "PUT",
            &management_path,
            Some(json!({"mode":"inherit","grants":[]}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "host update eligibility cannot self-grant record actions"
    );

    // Four row actions remain independent and never widen the coarse interface eligibility.
    f.policy(&f.alpha, "restricted", &["view"]).await;
    get(f, false, &f.alpha, StatusCode::OK).await;
    assert_eq!(
        capabilities(f, false, &f.alpha).await["allowed_actions"],
        json!(["view"])
    );
    expect(
        f,
        false,
        "PUT",
        &format!("/records/{}", f.alpha),
        Some(json!({"name":"forbidden"})),
        StatusCode::FORBIDDEN,
    )
    .await;
    expect(
        f,
        false,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    expect(
        f,
        false,
        "DELETE",
        &format!("/records/{}", f.alpha),
        None,
        StatusCode::FORBIDDEN,
    )
    .await;
    f.policy(&f.alpha, "restricted", &["update"]).await;
    get(f, false, &f.alpha, StatusCode::FORBIDDEN).await;
    expect(
        f,
        false,
        "PUT",
        &format!("/records/{}", f.alpha),
        Some(json!({"name":"alpha-edited"})),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        get(f, true, &f.alpha, StatusCode::OK).await["name"],
        "alpha-edited"
    );
    assert_eq!(get(f, true, &f.beta, StatusCode::OK).await["name"], "beta");
    mcp_success(
        &f.mcp(
            false,
            "update",
            json!({"record_id":f.alpha,"name":"alpha-edited"}),
        )
        .await,
    );
    f.policy(&f.gamma, "restricted", &["delete"]).await;
    expect(
        f,
        false,
        "DELETE",
        &format!("/records/{}", f.gamma),
        None,
        StatusCode::OK,
    )
    .await;
    get(f, true, &f.gamma, StatusCode::NOT_FOUND).await;

    // Handles are Host-owned: a session-only action cannot substitute an accessible record.
    f.policy(&f.alpha, "restricted", &["open"]).await;
    get(f, false, &f.alpha, StatusCode::FORBIDDEN).await;
    let opened = expect(
        f,
        false,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;
    let handle = opened["session_id"].as_str().unwrap();
    let command = f
        .mcp(
            false,
            "command",
            json!({"session_id":handle,"record_id":f.beta}),
        )
        .await;
    assert_eq!(
        mcp_success(&command)["record_id"],
        f.alpha,
        "worker cannot substitute a body record id for the Host binding"
    );
    expect(
        f,
        false,
        "POST",
        "/sessions/unknown-handle/command",
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    expect(
        f,
        true,
        "POST",
        &format!("/sessions/{handle}/command"),
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    let mcp_open = f.mcp(false, "open", json!({"record_id":f.alpha})).await;
    let mcp_handle = mcp_success(&mcp_open)["session_id"].as_str().unwrap();
    expect(
        f,
        false,
        "POST",
        &format!("/sessions/{mcp_handle}/command"),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;

    // An issued MCP resource locator replays the canonical open action, including import replay.
    let resource = f.mcp(false, "resource", json!({"record_id":f.alpha})).await;
    mcp_success(&resource);
    let text_uri = resource_reference(&resource, "text");
    let binary_uri = resource_reference(&resource, "blob");
    assert_ne!(text_uri, binary_uri);
    let read = f
        .rpc(
            false,
            json!({"jsonrpc":"2.0","id":43,"method":"resources/read","params":{"uri":text_uri}}),
        )
        .await;
    assert!(read.get("error").is_none(), "{read}");
    assert_eq!(read["result"]["contents"][0]["text"], "alpha-edited");
    assert_eq!(
        replay(f, &text_uri)
            .await
            .err()
            .expect("Host must reject text resource file imports")
            .code,
        "file_import_resource_not_binary",
        "text resources remain readable but are not binary file imports"
    );
    let binary_read = f
        .rpc(
            false,
            json!({"jsonrpc":"2.0","id":45,"method":"resources/read","params":{"uri":binary_uri}}),
        )
        .await;
    assert!(binary_read.get("error").is_none(), "{binary_read}");
    assert!(binary_read["result"]["contents"][0]["blob"].is_string());
    let imported = replay(f, &binary_uri).await.unwrap();
    assert_eq!(imported.bytes.as_slice(), b"\0\xffalpha-edited");
    assert_eq!(imported.filename, "content.bin");
    assert_eq!(
        imported.mime_type.as_deref(),
        Some("application/octet-stream")
    );
    f.policy(&f.beta, "restricted", &["open"]).await;
    f.policy(&f.alpha, "restricted", &[]).await;
    expect(
        f,
        false,
        "POST",
        &format!("/sessions/{handle}/command"),
        Some(json!({"record_id":f.beta})),
        StatusCode::FORBIDDEN,
    )
    .await;
    mcp_denied(
        &f.mcp(
            false,
            "command",
            json!({"session_id":mcp_handle,"record_id":f.beta}),
        )
        .await,
    );
    for uri in [&text_uri, &binary_uri] {
        let revoked = f
            .rpc(
                false,
                json!({"jsonrpc":"2.0","id":44,"method":"resources/read","params":{"uri":uri}}),
            )
            .await;
        assert!(
            revoked.get("error").is_some() || revoked["result"]["isError"] == true,
            "revoked locator returned content: {revoked}"
        );
    }
    assert!(
        replay(f, &binary_uri).await.is_err(),
        "resource import replay must not inherit a past capability decision"
    );
    let root_open = expect(
        f,
        true,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::OK,
    )
    .await;
    assert!(
        root_open["session_id"].is_string(),
        "root new calls stay exempt from restricted empty grants"
    );

    // Row grants cannot add a missing console operation, even with all four row actions.
    f.policy(
        &f.alpha,
        "restricted",
        &["view", "update", "open", "delete"],
    )
    .await;
    let allowed = [
        "record_fixture.list",
        "record_fixture.get",
        "record_fixture.update",
        "record_fixture.delete",
    ];
    let (status,result)=http(&f.app,&f.root,"PUT","/api/console/settings/roles/record_operator/console-policy",Some(json!({"groups":[{"kind":"plugin","group_id":CODE,"enabled":true,"strategy":"custom","operations":allowed.iter().map(|operation|json!({"kind":"simple","operation_id":operation,"enabled":true})).collect::<Vec<_>>()},{"kind":"settings_feature","group_id":"system.api-key-authentication","enabled":true,"strategy":"full","operations":[]}]}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{result}");
    get(f, false, &f.alpha, StatusCode::OK).await;
    expect(
        f,
        false,
        "POST",
        &format!("/records/{}/open", f.alpha),
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
    mcp_denied(&f.mcp(false, "open", json!({"record_id":f.alpha})).await);
    assert!(!capabilities(f, false, &f.alpha).await["allowed_actions"]
        .as_array()
        .unwrap()
        .contains(&json!("open")));
    let (status,result)=http(&f.app,&f.root,"PUT","/api/console/settings/roles/record_operator/console-policy",Some(json!({"groups":[{"kind":"plugin","group_id":CODE,"enabled":true,"strategy":"full","operations":[]},{"kind":"settings_feature","group_id":"system.api-key-authentication","enabled":true,"strategy":"full","operations":[]}]}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{result}");
    f.policy(&f.alpha, "restricted", &[]).await;
    streaming::run(f).await;

    // Removing the header restores the original operation rules without changing record data.
    f.policy(&f.alpha, "inherit", &[]).await;
    assert_eq!(
        get(f, false, &f.alpha, StatusCode::OK).await["name"],
        "alpha-edited"
    );
    let restored = f.mcp(false, "get", json!({"record_id":f.alpha})).await;
    assert_eq!(mcp_success(&restored)["name"], "alpha-edited");
    let empty_header = f.policy(&f.alpha, "restricted", &[]).await;
    assert!(empty_header["revision"].as_i64().unwrap() > 0);
    f.policy(&f.alpha, "inherit", &[]).await;

    // Lifecycle identity is independent of root's row exemption. Disable closes the active
    // registry; re-enable retains data/policies, while old generation handles cannot be reused.
    let state = f.state();
    let management = PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let actor = f.actor(true).await;
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor.user_id,
            installation_id: f.installation,
        })
        .await
        .unwrap();
    let lookup=format!("/api/console/settings/roles/record-permissions?plugin_code={CODE}&resource_code=records&record_id={}",f.beta);
    assert_eq!(
        http(&f.app, &f.root, "GET", &lookup, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = f
        .request(true, "GET", &format!("/records/{}", f.alpha), None)
        .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "root must not resurrect an inactive owner: {result}"
    );
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor.user_id,
            installation_id: f.installation,
        })
        .await
        .unwrap();
    assert_eq!(
        get(f, false, &f.alpha, StatusCode::OK).await["name"],
        "alpha-edited"
    );
    assert_eq!(get(f, true, &f.beta, StatusCode::OK).await["name"], "beta");
    get(f, false, &f.beta, StatusCode::FORBIDDEN).await;
    let (status, persisted) = http(&f.app, &f.root, "GET", &lookup, None).await;
    assert_eq!(status, StatusCode::OK, "{persisted}");
    assert_eq!(persisted["data"]["mode"], "restricted");
    assert_eq!(persisted["data"]["grants"], json!([]));
    expect(
        f,
        false,
        "POST",
        &format!("/sessions/{baseline_handle}/command"),
        Some(json!({})),
        StatusCode::FORBIDDEN,
    )
    .await;
}
