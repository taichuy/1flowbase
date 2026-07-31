use serde_json::json;

use api_server::openapi_docs::{build_api_docs_registry, paginate_category_operations};

#[test]
fn registry_requires_operation_id_for_every_operation() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": { "/demo": { "get": { "summary": "missing op id" } } }
    });

    let error = build_api_docs_registry(canonical).expect_err("missing operationId must fail");
    assert!(error.to_string().contains("operationId"));
}

#[test]
fn registry_rejects_duplicate_operation_ids() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/demo/a": { "get": { "operationId": "dup" } },
            "/demo/b": { "post": { "operationId": "dup" } }
        }
    });

    let error = build_api_docs_registry(canonical).expect_err("duplicate operationId must fail");
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn console_registry_has_complete_unique_media_aware_inventory() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let console = registry.category_operations("console").unwrap();
    let operation_ids = console
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(operation_ids.len(), console.operations.len());
    for operation in &console.operations {
        assert!(
            registry.operation_spec(&operation.id).is_some(),
            "missing closed OpenAPI spec for {}",
            operation.id
        );
    }

    for operation_id in [
        "start_flow_debug_run_stream",
        "subscribe_flow_debug_run_stream",
    ] {
        let spec = registry.operation_spec(operation_id).unwrap();
        let operation = console
            .operations
            .iter()
            .find(|operation| operation.id == operation_id)
            .unwrap();
        assert!(
            spec["paths"][&operation.path][&operation.method.to_ascii_lowercase()]["responses"]
                ["200"]["content"]["text/event-stream"]
                .is_object()
        );
    }
}

#[test]
fn operation_spec_builder_keeps_refs_closed() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry.operation_spec("patch_me").unwrap();

    assert_eq!(spec["paths"].as_object().unwrap().len(), 1);
    assert!(spec["paths"]["/api/console/me"]["patch"].is_object());
    assert!(spec["components"].is_object());
}

#[test]
fn operation_spec_builder_exposes_system_runtime_profile_route() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry.operation_spec("get_runtime_profile").unwrap();

    assert!(spec["paths"]["/api/console/system/runtime-profile"]["get"].is_object());
}

#[test]
fn operation_spec_builder_exposes_model_provider_catalog_route() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry
        .operation_spec("model_provider_list_catalog")
        .unwrap();

    assert!(spec["paths"]["/api/console/settings/model-providers/catalog"]["get"].is_object());
}

#[test]
fn operation_spec_builder_exposes_dynamic_data_model_docs_route() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry.operation_spec("get_data_model_openapi").unwrap();

    assert!(spec["paths"]
        ["/api/console/settings/data-models/model-definitions/{model_id}/openapi.json"]["get"]
        .is_object());
    assert!(spec["components"]["schemas"]["DataModelOpenApiDocumentResponse"].is_object());
}

#[test]
fn operation_spec_builder_keeps_servers_and_security_schemes_for_try_it_out() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/console/me": {
                "patch": {
                    "operationId": "patch_me",
                    "summary": "Patch me",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/PatchMeBody"
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "PatchMeBody": {
                    "type": "object",
                    "properties": {
                        "nickname": { "type": "string" }
                    }
                }
            }
        }
    });

    let registry = build_api_docs_registry(canonical).expect("catalog should build");
    let spec = registry
        .operation_spec("patch_me")
        .expect("single operation spec should exist");

    assert_eq!(spec["servers"][0]["url"], "/");
    assert_eq!(
        spec["security"],
        json!([
            { "sessionCookie": [], "csrfHeader": [] },
            { "patBearer": [] }
        ])
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["sessionCookie"]["type"],
        "apiKey"
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["sessionCookie"]["in"],
        "cookie"
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["csrfHeader"]["name"],
        "x-csrf-token"
    );
    assert_eq!(
        spec["components"]["securitySchemes"]["patBearer"]["scheme"],
        "bearer"
    );
    assert!(spec["components"]["schemas"]["PatchMeBody"].is_object());
}

#[test]
fn registry_groups_catalog_by_api_prefix_and_singletons_for_non_api_paths() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/console/me": {
                "patch": { "operationId": "patch_me", "summary": "Patch me" }
            },
            "/api/console/settings/members": {
                "get": { "operationId": "list_members", "summary": "List members" }
            },
            "/api/runtime/jobs": {
                "get": { "operationId": "list_runtime_jobs", "summary": "List runtime jobs" }
            },
            "/health": {
                "get": { "operationId": "health", "summary": "Health check" }
            }
        }
    });

    let registry = build_api_docs_registry(canonical).expect("catalog should build");
    let catalog = registry.catalog();

    assert_eq!(catalog.categories.len(), 3);
    assert_eq!(catalog.categories[0].id, "console");
    assert_eq!(catalog.categories[0].operation_count, 2);
    assert_eq!(catalog.categories[1].id, "runtime");
    assert_eq!(catalog.categories[1].operation_count, 1);
    assert_eq!(catalog.categories[2].operation_count, 1);

    let singleton_category = registry.category_operations("single:health").unwrap();
    assert_eq!(singleton_category.operations.len(), 1);
    assert_eq!(singleton_category.operations[0].id, "health");
}

#[test]
fn category_operations_pagination_returns_stable_pages() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/console/user-api-keys": {
                "post": { "operationId": "create_user_api_key", "summary": "Create user API key" }
            },
            "/api/console/settings/members": {
                "get": { "operationId": "list_members", "summary": "List members" }
            },
            "/api/console/workspace": {
                "get": { "operationId": "get_workspace", "summary": "Get workspace" }
            }
        }
    });
    let registry = build_api_docs_registry(canonical).expect("catalog should build");
    let console_operations = registry
        .category_operations("console")
        .expect("console operations should exist");

    let first_page = paginate_category_operations(console_operations, 0, 2);
    assert_eq!(first_page.operations.len(), 2);
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.offset, 0);
    assert_eq!(first_page.limit, 2);
    assert!(first_page.has_more);
    assert_eq!(first_page.next_offset, Some(2));

    let second_page = paginate_category_operations(console_operations, 2, 2);
    assert_eq!(second_page.operations.len(), 1);
    assert_eq!(second_page.offset, 2);
    assert_eq!(second_page.next_offset, None);
    assert!(!second_page.has_more);
    assert_ne!(first_page.operations[0].id, second_page.operations[0].id);
}

#[test]
fn registry_excludes_generic_runtime_model_crud_from_public_docs_catalog() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/runtime/models/{model_code}/list": {
                "get": { "operationId": "list_records", "summary": "List runtime model records" }
            },
            "/api/runtime/models/{model_code}/create": {
                "post": { "operationId": "create_record", "summary": "Create runtime model record" }
            },
            "/api/runtime/models/{model_code}/get/{id}": {
                "get": { "operationId": "get_record", "summary": "Get runtime model record" }
            },
            "/api/runtime/models/{model_code}/update/{id}": {
                "patch": { "operationId": "update_record", "summary": "Update runtime model record" }
            },
            "/api/runtime/models/{model_code}/delete/{id}": {
                "delete": { "operationId": "delete_record", "summary": "Delete runtime model record" }
            },
            "/api/runtime/jobs": {
                "get": { "operationId": "list_runtime_jobs", "summary": "List runtime jobs" }
            }
        }
    });

    let registry = build_api_docs_registry(canonical).expect("catalog should build");
    let runtime_operations = registry.category_operations("runtime").unwrap();

    assert_eq!(runtime_operations.operations.len(), 1);
    assert_eq!(runtime_operations.operations[0].id, "list_runtime_jobs");
    assert!(registry.operation_spec("create_record").is_none());
}

#[test]
fn category_spec_builder_keeps_all_category_operations_closed() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/console/me": {
                "patch": { "operationId": "patch_me", "summary": "Patch me" }
            },
            "/api/console/settings/members": {
                "get": { "operationId": "list_members", "summary": "List members" }
            },
            "/api/runtime/jobs": {
                "get": { "operationId": "list_runtime_jobs", "summary": "List runtime jobs" }
            }
        }
    });

    let registry = build_api_docs_registry(canonical).expect("catalog should build");
    let spec = registry
        .category_spec("console")
        .expect("console category spec should exist");

    assert_eq!(spec["paths"].as_object().unwrap().len(), 2);
    assert!(spec["paths"]["/api/console/me"]["patch"].is_object());
    assert!(spec["paths"]["/api/console/settings/members"]["get"].is_object());
    assert!(spec["paths"]["/api/runtime/jobs"].is_null());
    assert!(spec["components"].is_object());
}
