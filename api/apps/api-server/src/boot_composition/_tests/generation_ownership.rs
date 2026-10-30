use super::*;
use crate::{
    _tests::support::{default_test_config, test_api_state_with_database_url},
    generation_dispatch::{GenerationDispatch, WeakGenerationDispatch},
    openapi_interface::{
        CallableDispatchError, CallableDispatchForwarding, CallableDispatchPort,
        CallableDispatchResult, DispatchArguments, OpenApiInterfaceCatalogEntry,
    },
    routes::{
        mcp_management::{
            debug_execute::{McpDebugExecuteBody, McpDebugExecuteError, McpServerBoundInputs},
            interface_catalog::{
                DynamicMcpInterfaceRegistrySnapshotPort, McpInterfaceRegistrySnapshotPort,
            },
            McpDebugResponseMode,
        },
        mcp_protocol::virtual_ui::McpInterfaceDispatchPort,
    },
};
use axum::{
    body::{to_bytes, Body, Bytes},
    http::Request,
    response::Response,
    routing::get,
};
use serde_json::json;

fn public_entry() -> OpenApiInterfaceCatalogEntry {
    OpenApiInterfaceCatalogEntry {
        operation_id: "public.providers".into(),
        method: "GET".into(),
        path: "/api/public/auth/providers".into(),
        name: "Providers".into(),
        description: String::new(),
        parameter_descriptors: vec![],
        request_schema: json!({}),
        response_schema: json!({}),
        request_media_type: None,
        response_media_type: Some("application/json".into()),
        security: json!({}),
    }
}

async fn dispatch_public(owner: &GenerationDispatch) {
    match owner
        .dispatch(
            &public_entry(),
            DispatchArguments::default(),
            BTreeMap::new(),
            CallableDispatchForwarding::default(),
        )
        .await
    {
        Ok(CallableDispatchResult::Json(_)) => {}
        _ => panic!("the selected generation must still execute its real public providers route"),
    }
}

#[tokio::test]
async fn repeated_catalog_publication_rejects_before_mutation_and_forks_have_independent_slots() {
    let (state, _) = test_api_state_with_database_url().await;
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let owner = boot.publish_complete_catalog(&state).unwrap();
    let before = boot.interface_registry().unwrap().snapshot();
    assert_eq!(boot.dispatch_assembly_count(), 1);
    assert!(boot.publish_complete_catalog(&state).is_err());
    assert!(Arc::ptr_eq(
        &before,
        &boot.interface_registry().unwrap().snapshot()
    ));
    assert_eq!(
        boot.dispatch_assembly_count(),
        1,
        "repeat must reject before assembly too"
    );
    let fork = boot.fork_generation(BTreeMap::new()).unwrap();
    assert_eq!(fork.fingerprint(), boot.fingerprint());
    assert_ne!(fork.generation_id(), owner.generation_id());
    assert!(fork.dispatch_owner().is_err());
    assert!(
        fork.publish_complete_catalog(&state).is_err(),
        "same fingerprint is not the same generation"
    );
    assert!(Arc::ptr_eq(
        &owner,
        &GenerationDispatch::acquire(&state).unwrap()
    ));
    drop(owner);
    assert!(GenerationDispatch::acquire(&state).is_err());
    assert!(boot.publish_complete_catalog(&state).is_err());
    assert!(Arc::ptr_eq(
        &before,
        &boot.interface_registry().unwrap().snapshot()
    ));
    assert_eq!(
        boot.dispatch_assembly_count(),
        1,
        "expired owner must never be reconstructed"
    );
}

#[tokio::test]
async fn abandoned_prepared_candidate_releases_without_mutating_serving_generation() {
    let (state, _) = test_api_state_with_database_url().await;
    let owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let publisher = ManagedGenerationPublisher::new(
        &state,
        default_test_config(),
        vec![],
        crate::console_router(state.clone(), true),
    )
    .unwrap();
    let candidate = publisher.prepare(vec![], BTreeMap::new()).await.unwrap();
    let candidate_state = candidate.state.clone();
    let candidate_owner = Arc::downgrade(&candidate.owner);
    assert_ne!(candidate.owner.generation_id(), owner.generation_id());
    drop(candidate);
    assert!(candidate_state.upgrade().is_none());
    assert!(candidate_owner.upgrade().is_none());
    assert!(Arc::ptr_eq(
        &publisher.current_generation().unwrap().owner,
        &owner
    ));
    dispatch_public(&owner).await;
}

#[tokio::test]
async fn expired_dispatch_and_snapshot_weak_ports_return_errors_with_metadata_still_retained() {
    let (state, _) = test_api_state_with_database_url().await;
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let owner = boot.publish_complete_catalog(&state).unwrap();
    let metadata = boot.interface_registry().unwrap().snapshot();
    let snapshots =
        DynamicMcpInterfaceRegistrySnapshotPort::new(boot.interface_registry().unwrap().clone());
    let weak = WeakGenerationDispatch(Arc::downgrade(&owner));
    let state_probe = Arc::downgrade(&state);
    drop(owner);
    drop(state);
    assert!(
        state_probe.upgrade().is_none(),
        "metadata must not own state through a factory or dispatcher"
    );
    assert!(snapshots.snapshot().is_err());
    let result = weak
        .dispatch(
            &public_entry(),
            DispatchArguments::default(),
            BTreeMap::new(),
            CallableDispatchForwarding::default(),
        )
        .await;
    assert!(matches!(result, Err(CallableDispatchError::Api(_))));
    let entry = domain::McpInterfaceCatalogEntry {
        interface_id: "public.providers".into(),
        source: domain::McpInterfaceCatalogSource::StaticApi,
        method: "GET".into(),
        path: "/api/public/auth/providers".into(),
        name: "Providers".into(),
        short_description: String::new(),
        parameter_descriptors: vec![],
        parameter_schema: json!({}),
        result_schema: json!({}),
        permission_code: None,
        security: json!({}),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    };
    let result = weak
        .execute(
            axum::http::HeaderMap::new(),
            entry,
            McpDebugExecuteBody {
                interface_id: "public.providers".into(),
                debug_response_mode: McpDebugResponseMode::ToolResult,
                mcp_arguments: json!({}),
                input_mapping: json!({}),
                output_mapping: json!({}),
            },
            McpServerBoundInputs {
                workspace_id: Uuid::nil(),
            },
        )
        .await;
    assert!(matches!(result, Err(McpDebugExecuteError::Api(_))));
    assert!(metadata.definitions().len() > 0);
}

#[tokio::test]
async fn admitted_http_first_internal_dispatch_uses_old_generation_after_publication() {
    let (state, _) = test_api_state_with_database_url().await;
    let owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let old_id = owner.generation_id();
    let state_probe = Arc::downgrade(&state);
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let initial = Router::new().route(
        "/held",
        get({
            let entered = entered.clone();
            let release = release.clone();
            let state_probe = state_probe.clone();
            move || {
                let entered = entered.clone();
                let release = release.clone();
                let state_probe = state_probe.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    let state = state_probe.upgrade().unwrap();
                    let owner = GenerationDispatch::acquire(&state).unwrap();
                    dispatch_public(&owner).await;
                    assert_eq!(
                        state
                            .extension_boot_snapshot
                            .as_ref()
                            .unwrap()
                            .dispatch_assembly_count(),
                        1
                    );
                    axum::Json(json!({"generation":owner.generation_id()}))
                }
            }
        }),
    );
    let publisher =
        ManagedGenerationPublisher::new(&state, default_test_config(), vec![], initial).unwrap();
    let ingress = publisher.ingress();
    drop(owner);
    drop(state);
    let request = tokio::spawn({
        let ingress = ingress.clone();
        async move {
            ingress
                .oneshot(Request::builder().uri("/held").body(Body::empty()).unwrap())
                .await
                .unwrap()
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    assert_ne!(
        publisher
            .current_generation()
            .unwrap()
            .owner
            .generation_id(),
        old_id
    );
    assert!(state_probe.upgrade().is_some());
    release.notify_one();
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), request)
        .await
        .unwrap()
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(value["generation"], json!(old_id));
    assert!(state_probe.upgrade().is_none());
    dispatch_public(&publisher.current_generation().unwrap().owner).await;
}

#[tokio::test]
async fn streaming_body_and_atomic_host_callback_each_keep_selected_generation_until_release() {
    let (state, _) = test_api_state_with_database_url().await;
    let owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let old_id = owner.generation_id();
    let state_probe = Arc::downgrade(&state);
    let release = Arc::new(tokio::sync::Notify::new());
    let initial = Router::new().route(
        "/stream",
        get({
            let release = release.clone();
            move || {
                let release = release.clone();
                async move {
                    Response::new(Body::from_stream(futures_util::stream::once(async move {
                        release.notified().await;
                        Ok::<_, std::io::Error>(Bytes::from_static(b"data: done\n\n"))
                    })))
                }
            }
        }),
    );
    let publisher =
        ManagedGenerationPublisher::new(&state, default_test_config(), vec![], initial).unwrap();
    let ingress = publisher.ingress();
    let host = publisher.current_generation().unwrap();
    let response = ingress
        .oneshot(
            Request::builder()
                .uri("/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    drop(owner);
    drop(state);
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    assert_eq!(host.owner.generation_id(), old_id);
    assert_eq!(
        host.state
            .extension_boot_snapshot
            .as_ref()
            .unwrap()
            .generation_id(),
        old_id
    );
    dispatch_public(&host.owner).await;
    drop(host);
    assert!(
        state_probe.upgrade().is_some(),
        "headers released, streaming body is the remaining old root"
    );
    release.notify_one();
    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        "data: done\n\n"
    );
    assert!(
        state_probe.upgrade().is_none(),
        "EOF must retire the old generation"
    );
}

#[tokio::test]
async fn initial_communication_worker_keeps_its_generation_and_releases_on_stop() {
    let (state, _) = test_api_state_with_database_url().await;
    let owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let user: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor =
        control_plane::ports::AuthRepository::load_actor_context_for_user(&state.store, user)
            .await
            .unwrap();
    let state_probe = Arc::downgrade(&state);
    let owner_probe = Arc::downgrade(&owner);
    let retired = owner.retirement_notification();
    let worker =
        crate::communication_runtime::CommunicationWorker::spawn(state.clone(), owner.clone());
    let publisher = ManagedGenerationPublisher::new(
        &state,
        default_test_config(),
        vec![],
        crate::console_router(state.clone(), true),
    )
    .unwrap();
    drop(owner);
    drop(state);
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    let initial = state_probe.upgrade().unwrap();
    let boot = initial.extension_boot_snapshot.as_ref().unwrap();
    let assemblies = boot.dispatch_assembly_count();
    let factory = crate::runtime_internal_tool_invoker_factory(&initial, &actor)
        .await
        .unwrap();
    assert_eq!(boot.dispatch_assembly_count(), assemblies);
    drop(factory);
    drop(initial);
    assert!(owner_probe.upgrade().is_some());
    worker.stop().await;
    drop(worker);
    tokio::time::timeout(std::time::Duration::from_secs(5), retired.notified())
        .await
        .unwrap();
    assert!(state_probe.upgrade().is_none());
    assert!(owner_probe.upgrade().is_none());
}

#[tokio::test]
async fn failed_candidate_catalog_releases_partial_assembly_and_preserves_serving_generation() {
    let (state, _) = test_api_state_with_database_url().await;
    let serving_owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let publisher = ManagedGenerationPublisher::new(
        &state,
        default_test_config(),
        vec![],
        crate::console_router(state.clone(), true),
    )
    .unwrap();
    let mut candidate = state.as_ref().clone();
    let boot = Arc::new(
        state
            .extension_boot_snapshot
            .as_ref()
            .unwrap()
            .fork_generation(BTreeMap::new())
            .unwrap(),
    );
    let service = crate::managed_services::ManagedServiceRegistration {
        installation_id: Uuid::now_v7(), plugin_code: "invalid-candidate".into(), plugin_version: "1.0.0".into(), pages: vec![],
        declaration: serde_json::from_value(json!({
            "scope":"system", "feature":{"feature_id":"invalid-candidate.settings","label":"Invalid candidate","description":"Reject candidate","route_id":"invalid-candidate","path":"/settings/invalid-candidate"},
            "operations":[{"interface_id":"invalid-candidate.list","contribution_id":"invalid-candidate.list","method":"GET","path":"/api/console/managed-services/invalid-candidate/items","summary":"List items","description":"Reject invalid schema", "input_schema":{"type":"object"},"output_schema":{"type":"invalid-json-schema-type"}}]
        })).unwrap(),
    };
    candidate.console_surface_registry = Arc::new(
        candidate
            .console_surface_registry
            .as_ref()
            .clone()
            .with_managed_services(vec![service])
            .unwrap(),
    );
    candidate.extension_boot_snapshot = Some(boot.clone());
    let candidate = Arc::new(candidate);
    let state_probe = Arc::downgrade(&candidate);
    let boot_probe = Arc::downgrade(&boot);
    let dynamic_probe = Arc::downgrade(boot.interface_registry().unwrap());
    let before = boot.interface_registry().unwrap().snapshot();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        boot.publish_complete_catalog(&candidate)
    }));
    assert!(
        !matches!(result, Ok(Ok(_))),
        "invalid candidate cannot publish"
    );
    assert_eq!(
        boot.dispatch_assembly_count(),
        0,
        "invalid contributions fail before final Router assembly"
    );
    assert!(boot.dispatch_owner().is_err());
    assert!(Arc::ptr_eq(
        &before,
        &boot.interface_registry().unwrap().snapshot()
    ));
    drop(candidate);
    drop(boot);
    assert!(state_probe.upgrade().is_none());
    assert!(boot_probe.upgrade().is_none());
    assert!(dynamic_probe.upgrade().is_none());
    assert!(Arc::ptr_eq(
        &serving_owner,
        &publisher.current_generation().unwrap().owner
    ));
    dispatch_public(&serving_owner).await;
}

#[tokio::test]
async fn nondefault_upload_limit_is_identical_for_published_http_and_callable_multipart() {
    use crate::_tests::support::login_and_capture_cookie;
    use axum::http::StatusCode;

    let (state, _) = test_api_state_with_database_url().await;
    let mut config = default_test_config();
    config.plugin_upload_max_bytes = 128 * 1024;
    let initial = crate::app_with_state_and_config(state.clone(), &config);
    let publisher =
        ManagedGenerationPublisher::new(&state, config.clone(), vec![], initial).unwrap();
    let ingress = publisher.ingress();
    let (cookie, csrf) = login_and_capture_cookie(&ingress, "root", "change-me").await;
    // Exercise managed candidate publication, not just the initial convenience router.
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    let selected = publisher.current_generation().unwrap();
    let boot = selected.state.extension_boot_snapshot.as_ref().unwrap();
    assert_eq!(boot.dispatch_assembly_count(), 1);

    let mut entry = public_entry();
    entry.operation_id = "plugin_install_upload".into();
    entry.method = "POST".into();
    entry.path = "/api/console/plugins/install-upload".into();
    entry.request_media_type = Some("multipart/form-data".into());
    entry.request_schema = json!({
        "type": "object", "properties": {"body": {
            "type": "object", "properties": {"probe": {"type": "string"}},
            "required": ["probe"]
        }}, "required": ["body"]
    });
    // Callable encoder uses a random UUID with this fixed boundary length.
    let boundary = format!("1flowbase-{}", "0".repeat(32));
    let prefix = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"probe\"\r\n\r\n");
    let suffix = format!("\r\n--{boundary}--\r\n");
    for total in [
        config.plugin_upload_max_bytes - 1,
        config.plugin_upload_max_bytes,
        config.plugin_upload_max_bytes + 1,
    ] {
        let probe = "x".repeat(total - prefix.len() - suffix.len());
        let body = format!("{prefix}{probe}{suffix}");
        assert_eq!(body.len(), total);
        // Within the limit, the real multipart handler rejects the absent file (400).
        // Capacity rejection must instead be 413, including exactly one byte over.
        let expected = if total <= config.plugin_upload_max_bytes {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::PAYLOAD_TOO_LARGE
        };
        let response = ingress
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&entry.path)
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "HTTP total={total}");
        if total <= config.plugin_upload_max_bytes {
            let error: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            assert_eq!(
                error["code"], "plugin_file",
                "must reach real multipart handler"
            );
        }
        let result = selected
            .owner
            .dispatch(
                &entry,
                DispatchArguments {
                    body: json!({"probe": probe}),
                    ..Default::default()
                },
                BTreeMap::new(),
                CallableDispatchForwarding {
                    cookie: Some(cookie.as_bytes().to_vec()),
                    csrf_token: Some(csrf.as_bytes().to_vec()),
                    ..Default::default()
                },
            )
            .await;
        match result {
            Err(CallableDispatchError::Target(response)) => {
                assert_eq!(response.status, expected.as_u16(), "callable total={total}");
                if total <= config.plugin_upload_max_bytes {
                    let error: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
                    assert_eq!(
                        error["code"], "plugin_file",
                        "callable must reach real multipart handler"
                    );
                }
            }
            _ => panic!("real upload route must reject missing file or oversized multipart"),
        }
    }
    assert_eq!(
        boot.dispatch_assembly_count(),
        1,
        "requests must reuse the compiled router"
    );
}
