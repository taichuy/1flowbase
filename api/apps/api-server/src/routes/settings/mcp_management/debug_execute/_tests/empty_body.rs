use super::*;
use axum::{routing::post, Json};
use domain::mcp_management::{McpInterfaceCatalogSource, McpRiskLevel};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn interface(body_schema: Value) -> domain::McpInterfaceCatalogEntry {
    domain::McpInterfaceCatalogEntry {
        interface_id: "empty_query".into(),
        source: McpInterfaceCatalogSource::StaticApi,
        method: "POST".into(),
        path: "/empty-query".into(),
        name: "Empty query".into(),
        short_description: String::new(),
        parameter_descriptors: vec![],
        parameter_schema: json!({"type":"object", "properties":{"body":body_schema}, "required":["body"], "additionalProperties":false}),
        result_schema: json!({"type":"object", "properties":{"empty":{"type":"boolean"}}, "required":["empty"]}),
        permission_code: None,
        security: json!({}),
        risk_level: McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}

fn call() -> McpDebugExecuteBody {
    McpDebugExecuteBody {
        interface_id: "empty_query".into(),
        debug_response_mode: McpDebugResponseMode::DebugDetails,
        mcp_arguments: json!({}),
        input_mapping: json!({"mappings":[]}),
        output_mapping: json!({}),
    }
}

#[tokio::test]
async fn required_empty_query_body_reaches_both_real_dispatch_adapters() {
    for use_port in [false, true] {
        let router = Router::new().route(
            "/empty-query",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(
                    body,
                    json!({}),
                    "the HTTP JSON extractor must receive an object"
                );
                Json(json!({"data":{"empty":true}, "meta":{}}))
            }),
        );
        let entry =
            interface(json!({"type":"object", "properties":{}, "additionalProperties":false}));
        let server_bound = McpServerBoundInputs {
            workspace_id: Uuid::now_v7(),
        };
        let result = if use_port {
            let port = crate::openapi_interface::console_router_callable_dispatch_port(router);
            execute_with_dispatch_port(
                port.as_ref(),
                Default::default(),
                entry,
                call(),
                server_bound,
                None,
            )
            .await
            .map_err(|_| ())
        } else {
            execute_with_console_router(router, HeaderMap::new(), entry, call(), server_bound)
                .await
                .map_err(|_| ())
        }
        .expect("an empty mapped query must reach the declared JSON route");
        assert_eq!(result["interface_arguments"]["body"], json!({}));
        assert_eq!(result["tool_result"], json!({"empty":true}));
    }
}

#[tokio::test]
async fn missing_scalar_body_is_rejected_before_the_route() {
    for use_port in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&calls);
        let router = Router::new().route(
            "/empty-query",
            post(move || {
                seen.fetch_add(1, Ordering::SeqCst);
                async { Json(json!({"data":{"empty":false}})) }
            }),
        );
        let entry = interface(json!({"type":"string"}));
        let server_bound = McpServerBoundInputs {
            workspace_id: Uuid::now_v7(),
        };
        let error = if use_port {
            let port = crate::openapi_interface::console_router_callable_dispatch_port(router);
            match execute_with_dispatch_port(
                port.as_ref(),
                Default::default(),
                entry,
                call(),
                server_bound,
                None,
            )
            .await
            {
                Err(McpDebugDispatchError::Api(error)) => error,
                _ => panic!("missing required scalar must fail schema validation"),
            }
        } else {
            match execute_with_console_router(router, HeaderMap::new(), entry, call(), server_bound)
                .await
            {
                Err(McpDebugExecuteError::Api(error)) => error,
                _ => panic!("missing required scalar must fail schema validation"),
            }
        };
        assert!(matches!(
            error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
            Some(control_plane::errors::ControlPlaneError::InvalidInput(
                "request_schema"
            ))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
