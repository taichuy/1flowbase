use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[test]
fn permission_operations_have_independent_routes_metadata_and_authentication_contracts() {
    let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
        Vec::new(),
        None,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
        Vec::new(),
    )
    .unwrap();
    let inventory = plan.console_operation_registry.inventory();
    for declaration in DECLARATIONS {
        let operation = inventory
            .operations
            .iter()
            .find(|o| o.operation_id == declaration.interface_id)
            .unwrap();
        assert_eq!(operation.routes.len(), 1);
        assert_eq!(operation.routes[0].method, declaration.method);
        assert_eq!(operation.routes[0].path, declaration.path);
        let metadata = inventory
            .interfaces
            .iter()
            .find(|i| i.interface_id == declaration.interface_id)
            .unwrap();
        assert!(!metadata.summary.is_empty());
        assert!(!metadata.description.is_empty());
        assert_eq!(
            (metadata.summary.clone(), metadata.description.clone()),
            super::metadata(declaration.interface_id).unwrap()
        );
    }
}
#[test]
fn editor_input_does_not_accept_caller_identity_or_plugin_scope_override() {
    for body in [
        json!({"mode":"restricted","grants":[],"actor_user_id":"forged"}),
        json!({"mode":"restricted","grants":[],"workspace_id":"forged"}),
        json!({"mode":"restricted","grants":[{"role_id":uuid::Uuid::nil(),"actions":["open"],"is_root":true}]}),
    ] {
        assert!(serde_json::from_value::<ReplaceBody>(body).is_err());
    }
    assert!(serde_json::from_value::<CapabilitiesBody>(
        json!({"plugin_code":"fixture","resource_code":"hosts","record_ids":[],"is_root":true})
    )
    .is_err());
}
#[tokio::test]
async fn root_read_and_current_user_capabilities_reach_canonical_target_without_csrf() {
    use crate::_tests::support::{login_and_capture_cookie, test_app};
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let path="/api/console/settings/roles/record-permissions?plugin_code=inactive-fixture&resource_code=hosts&record_id=host-1";
    let missing_auth = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);
    let missing_owner = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_owner.status(), StatusCode::NOT_FOUND);
    let capabilities=app.oneshot(Request::builder().method("POST").uri("/api/console/settings/roles/record-capabilities").header("cookie",cookie).header("content-type","application/json").body(Body::from(json!({"plugin_code":"inactive-fixture","resource_code":"hosts","record_ids":[]}).to_string())).unwrap()).await.unwrap();
    assert_eq!(capabilities.status(), StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn a_role_without_record_management_cannot_grant_itself_row_permissions() {
    use crate::_tests::support::{create_member, login_and_capture_cookie, test_app};
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "record-no-management",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "record-no-management", "temp-pass").await;
    let denied=app.oneshot(Request::builder().method("PUT").uri("/api/console/settings/roles/record-permissions?plugin_code=inactive-fixture&resource_code=hosts&record_id=host-1").header("cookie",cookie).header("x-csrf-token",csrf).header("content-type","application/json").body(Body::from(json!({"mode":"inherit","grants":[]}).to_string())).unwrap()).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

#[test]
fn record_permission_events_inherit_each_api_contract_and_preserve_actual_values() {
    use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(paths(
        crate::managed_services::record_permissions::get,
        crate::managed_services::record_permissions::replace,
        crate::managed_services::record_permissions::capabilities
    ))]
    struct RecordPermissionDocs;
    let docs = crate::openapi_docs::build_api_docs_registry(
        serde_json::to_value(RecordPermissionDocs::openapi()).unwrap(),
    )
    .unwrap();
    let query = || RecordQuery {
        plugin_code: "actual-plugin".into(),
        resource_code: "actual-resource".into(),
        record_id: "actual-record".into(),
    };
    let role_id = uuid::Uuid::now_v7();
    let body = ReplaceBody {
        mode: domain::RecordPermissionMode::Restricted,
        grants: vec![GrantBody {
            role_id,
            actions: vec!["open".into()],
        }],
        expected_revision: Some(7),
    };
    let query_wire = json!({"plugin_code":"actual-plugin","resource_code":"actual-resource","record_id":"actual-record"});
    let body_wire = serde_json::to_value(&body).unwrap();
    let resource = json!({"resource_code":"actual-resource","label":"Actual resource","description":"Actual declared resource","actions":[{"action_code":"open","label":"Open","description":"Open actual record"}]});
    for (index, input, expected, result) in [
        (
            0,
            Input::Get(query()),
            json!({"query":query_wire}),
            json!({"resource":resource,"roles":[{"role_id":role_id,"code":"actual-role","name":"Actual role"}],"mode":"restricted","grants":[{"role_id":role_id,"actions":["open"]}],"revision":8}),
        ),
        (
            1,
            Input::Replace(query(), body),
            json!({"query":query_wire,"body":body_wire}),
            json!({"mode":"restricted","grants":[{"role_id":role_id,"actions":["open"]}],"revision":8}),
        ),
        (
            2,
            Input::Capabilities(CapabilitiesBody {
                plugin_code: "actual-plugin".into(),
                resource_code: "actual-resource".into(),
                record_ids: vec!["actual-record".into()],
            }),
            json!({"body":{"plugin_code":"actual-plugin","resource_code":"actual-resource","record_ids":["actual-record"]}}),
            json!({"items":[{"record_id":"actual-record","allowed_actions":["open"],"can_manage_permissions":false}]}),
        ),
    ] {
        let declaration = &DECLARATIONS[index];
        let operation = docs.operation(declaration.interface_id).unwrap();
        let view = docs.operation_spec_view(declaration.interface_id).unwrap();
        let contract =
            crate::openapi_interface::catalog_entry_from_view(&operation, &view).unwrap();
        let context = ApiOperationContext {
            interface_id: declaration.interface_id.into(),
            method: declaration.method.into(),
            path: declaration.path.into(),
        };
        assert!(
            Input::document_input_schema(&context).is_none(),
            "use the shared API request schema"
        );
        assert!(
            Output::document_output_schema(&context).is_none(),
            "use the shared API response schema"
        );
        let before = input.project_document_input(&context).unwrap();
        assert_eq!(before, expected);
        assert!(jsonschema::validator_for(&contract.request_schema)
            .unwrap()
            .is_valid(&before));
        let output = Output(result.clone());
        let after = output.project_document_output(&context).unwrap();
        assert_eq!(after, result);
        assert!(jsonschema::validator_for(&contract.response_schema)
            .unwrap()
            .is_valid(&after));
        assert!(
            !jsonschema::validator_for(&contract.response_schema)
                .unwrap()
                .is_valid(&json!({"record_permissions":true})),
            "managed-hook summaries must not replace the actual API result"
        );
    }
}
