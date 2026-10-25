use serde_json::json;

use api_server::openapi_docs::{build_api_docs_registry, paginate_category_operations};

#[test]
fn canonical_agent_logs_ingest_uses_defined_application_bearer_credential() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry.operation_spec("ingest_events").unwrap();
    assert_eq!(
        spec["paths"]["/api/logs/v1/events"]["post"]["security"],
        json!([{ "applicationApiKey": [] }])
    );
    let schemes = spec["components"]["securitySchemes"].as_object().unwrap();
    assert_eq!(schemes.len(), 1);
    let credential = &schemes["applicationApiKey"];
    assert_eq!(credential["type"], "http");
    assert_eq!(credential["scheme"], "bearer");
    assert_eq!(credential["bearerFormat"], "Application API Key");
    let description = credential["description"].as_str().unwrap();
    assert!(description.contains("bound to the target application and workspace"));
}

#[test]
fn registry_rejects_undefined_ingest_security_scheme() {
    let canonical = json!({
        "openapi": "3.1.0",
        "info": { "title": "T", "version": "1" },
        "paths": {
            "/api/logs/v1/events": {
                "post": {
                    "operationId": "ingest_events",
                    "security": [{ "undefinedApplicationCredential": [] }],
                    "responses": { "200": { "description": "Accepted" } }
                }
            }
        }
    });
    let error = build_api_docs_registry(canonical).expect_err("undefined credential must fail");
    assert!(error.to_string().contains("missing security scheme"));
    assert!(error.to_string().contains("undefinedApplicationCredential"));
}

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

#[test]
fn canonical_otlp_trace_ingest_advertises_application_bearer_and_standard_wire_formats() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    let spec = registry.operation_spec("ingest_traces").unwrap();
    let operation = &spec["paths"]["/api/logs/otlp/v1/traces"]["post"];
    assert_eq!(operation["security"], json!([{ "applicationApiKey": [] }]));
    for mime in ["application/json", "application/x-protobuf"] {
        assert!(operation["requestBody"]["content"].get(mime).is_some());
        assert!(operation["responses"]["200"]["content"].get(mime).is_some());
    }
    assert!(operation["responses"].get("415").is_some());
    assert!(spec["components"]["securitySchemes"]
        .get("applicationApiKey")
        .is_some());
}

#[test]
fn scope_plans_keep_path_siblings_cycles_escaped_refs_and_overlapping_components() {
    let registry = build_api_docs_registry(json!({
        "openapi": "3.1.0", "info": {"title":"T", "version":"1"},
        "tags": [{"name":"used"},{"name":"unused"}],
        "paths": {"/api/demo/{id}": {
            "parameters": [{"$ref":"#/components/parameters/Id"}],
            "x-path-note": "keep",
            "get": {"operationId":"read", "tags":["used"], "responses":{"200":{
                "content":{"application/json":{"schema":{"$ref":"#/components/schemas/A~1B~0C"}}}
            }}},
            "post": {"operationId":"write", "requestBody":{"$ref":"#/components/requestBodies/Body"}}
        }},
        "components": {
            "parameters":{"Id":{"name":"id","in":"path","schema":{"type":"string"}}},
            "requestBodies":{"Body":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/A~1B~0C/properties/id"}}}}},
            "schemas":{"A/B~C":{"type":"object","properties":{
                "id":{"type":"string"}, "next":{"$ref":"#/components/schemas/Next"}
            }}, "Next":{"$ref":"#/components/schemas/A~1B~0C"},
            "Unused":{"$ref":"https://unused.invalid/schema"}}
        }
    })).unwrap();
    let spec = registry.operation_spec("read").unwrap();
    let path = &spec["paths"]["/api/demo/{id}"];
    assert_eq!(path["x-path-note"], "keep");
    assert!(path["parameters"].is_array());
    assert!(path.get("post").is_none());
    assert_eq!(spec["tags"], json!([{"name":"used"}]));
    assert_eq!(
        spec["components"]["schemas"]["Next"]["$ref"],
        "#/components/schemas/A~1B~0C"
    );
    assert!(spec["components"]["schemas"].get("Unused").is_none());
    let category = registry.category_spec("demo").unwrap();
    assert_eq!(
        category["components"]["schemas"]["A/B~C"]["properties"]["id"]["type"],
        "string"
    );
    assert!(category["paths"]["/api/demo/{id}"]["post"].is_object());
}

#[test]
fn scope_plans_close_path_item_refs_without_other_methods() {
    let registry = build_api_docs_registry(json!({
        "openapi":"3.1.0", "info":{"title":"T","version":"1"},
        "paths":{"/api/demo":{
            "$ref":"#/components/pathItems/Shared",
            "get":{"operationId":"read"},
            "post":{"operationId":"write","schema":{"$ref":"#/components/schemas/PostOnly"}}
        }},
        "components":{
            "pathItems":{"Shared":{"parameters":[{"$ref":"#/components/parameters/Id"}]}},
            "parameters":{"Id":{"name":"id","in":"query","schema":{"$ref":"#/components/schemas/Id"}}},
            "schemas":{"Id":{"type":"string"},"PostOnly":{"type":"integer"},
                "Unused":{"$ref":"https://unused.invalid/schema"}}
        }
    })).unwrap();
    let operation = registry.operation_spec("read").unwrap();
    let category = registry.category_spec("demo").unwrap();
    for spec in [&operation, &category] {
        assert_eq!(
            spec["paths"]["/api/demo"]["$ref"],
            "#/components/pathItems/Shared"
        );
        assert!(spec["components"]["pathItems"]["Shared"].is_object());
        assert!(spec["components"]["parameters"]["Id"].is_object());
        assert_eq!(spec["components"]["schemas"]["Id"]["type"], "string");
        assert!(spec["components"]["schemas"].get("Unused").is_none());
    }
    assert!(operation["paths"]["/api/demo"].get("post").is_none());
    assert!(operation["components"]["schemas"].get("PostOnly").is_none());
    assert!(category["components"]["schemas"]["PostOnly"].is_object());
}

#[test]
fn scope_plans_reject_invalid_path_item_refs_during_build() {
    for (reference, message) in [
        ("https://example.invalid/Path", "unsupported external $ref"),
        ("#/components/pathItems/Absent", "missing referenced node"),
        ("#/info", "unsupported non-components $ref"),
    ] {
        let error = build_api_docs_registry(json!({
            "openapi":"3.1.0", "info":{"title":"T","version":"1"},
            "paths":{"/demo":{"$ref":reference,"get":{"operationId":"read"}}}
        }))
        .unwrap_err();
        assert_eq!(error.to_string(), format!("{message} `{reference}`"));
    }
}

#[test]
fn scope_plan_rejects_pointer_insertion_failure_before_any_getter() {
    let error = build_api_docs_registry(json!({
        "openapi":"3.1.0", "info":{"title":"T","version":"1"},
        "paths":{"/demo":{"get":{"operationId":"read","schema":{"$ref":"#/components/schemas/Array"}}}},
        "components":{"schemas":{"Array":[{"$ref":"#/components/schemas/Array/0"}]}}
    })).unwrap_err();
    assert_eq!(error.to_string(), "JSON pointer target must be an object");
}

#[test]
fn scope_plans_preserve_legacy_scheme_name_insertion_and_empty_security() {
    let registry = build_api_docs_registry(json!({
        "openapi":"3.1.0", "info":{"title":"T","version":"1"},
        "security":[{"default":[]}],
        "paths":{
            "/demo":{"get":{"operationId":"named","security":[{"a/b~1c":[]}]}},
            "/empty":{"get":{"operationId":"empty","security":[]}}
        },
        "components":{"securitySchemes":{
            "default":{"type":"http","scheme":"bearer"},
            "a/b~1c":{"type":"apiKey","in":"header","name":"x-key"}
        }}
    }))
    .unwrap();
    let named = registry.operation_spec("named").unwrap();
    assert_eq!(
        named["components"]["securitySchemes"]["a"]["b/c"]["name"],
        "x-key"
    );
    let empty = registry.operation_spec("empty").unwrap();
    assert!(empty.get("security").is_none());
    assert_eq!(empty["paths"]["/empty"]["get"]["security"], json!([]));
}

#[test]
fn borrowed_catalog_projection_matches_every_materialized_operation() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    for category in &registry.catalog().categories {
        for operation in &registry
            .category_operations(&category.id)
            .unwrap()
            .operations
        {
            let spec = registry.operation_spec(&operation.id).unwrap();
            let view = registry.operation_spec_view(&operation.id).unwrap();
            let expected = crate::openapi_interface::catalog_entry_from_operation(operation, &spec);
            let actual = crate::openapi_interface::catalog_entry_from_view(operation, &view);
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{}",
                operation.id
            );
        }
    }
}

#[test]
fn operation_documents_are_independent_across_requests_and_registry_snapshots() {
    let canonical = json!({"openapi":"3.1.0","info":{"title":"T","version":"1"},"paths":{"/demo":{"get":{"operationId":"read"}}}});
    let registry = std::sync::Arc::new(build_api_docs_registry(canonical.clone()).unwrap());
    let weak = std::sync::Arc::downgrade(&registry);
    let mut first = registry.operation_spec("read").unwrap();
    first["info"]["title"] = json!("request-local");
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let registry = &registry;
            scope.spawn(move || {
                assert_eq!(
                    registry.operation_spec("read").unwrap()["info"]["title"],
                    "T"
                )
            });
        }
    });
    let mut next = canonical;
    next["info"]["version"] = json!("2");
    let next = build_api_docs_registry(next).unwrap();
    assert_eq!(
        registry.operation_spec("read").unwrap()["info"]["version"],
        "1"
    );
    assert_eq!(next.operation_spec("read").unwrap()["info"]["version"], "2");
    drop(registry);
    assert!(weak.upgrade().is_none());
    assert_eq!(first["info"]["title"], "request-local");
}
