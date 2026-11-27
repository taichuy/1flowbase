use super::*;

fn metadata(entry: &OpenApiCapabilityCatalogEntry) -> OpenApiCapabilityIndexEntry {
    OpenApiCapabilityIndexEntry {
        interface_id: entry.interface.operation_id.clone(),
        method: entry.interface.method.clone(),
        path: entry.interface.path.clone(),
        name: entry.interface.name.clone(),
        short_description: entry.interface.description.clone(),
        source: entry.source,
        risk_level: entry.risk_level,
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason,
    }
}

#[tokio::test]
async fn capability_catalog_index_and_detail_keep_metadata_and_media_decisions() {
    let operation = |id: &str, responses: Value| json!({"operationId":id,"summary":"Read examples", "description":"Read test records.", "responses":responses});
    let json_response = json!({"description":"ok", "content":{"application/problem+json":{"schema":{"type":"object"}}}});
    let api_docs = crate::openapi_docs::build_api_docs_registry(json!({
        "openapi":"3.1.0", "info":{"title":"Index fixture","version":"1"},
        "paths":{
            "/api/console/json":{"get":operation("json", json!({"200":{"$ref":"#/components/responses/Json"}}))},
            "/api/console/stream":{"get":operation("stream", json!({"200":{"description":"stream","content":{"text/event-stream":{"schema":{"type":"string"}}}}}))},
            "/api/console/empty":{"delete":operation("empty", json!({"204":{"description":"deleted"}}))},
            "/api/console/upgrade":{"get":operation("upgrade", json!({"101":{"description":"upgrade"}}))},
            "/api/internal/only":{"get":operation("internal", json!({"200":json_response}))}
        },
        "components":{"responses":{"Json":json_response}}
    })).unwrap();
    let store = storage_durable_postgres::MainDurableStore::new(
        // An unreachable database: static and missing entries must not need dynamic repositories.
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://postgres@127.0.0.1:1/unreachable")
            .unwrap(),
    );
    let dependencies = OpenApiCapabilityCatalogDependencies {
        store,
        api_docs: Arc::new(api_docs),
        interface_registry: None,
        template_catalog:
            runtime_core::data_model_template_registry::DataModelTemplateCatalog::core(),
        console_operations: Arc::new(access_control::ConsoleOperationCompiledInventory {
            schema_version: "test",
            operations: vec![],
            resources: vec![],
            locale_catalog: None,
            interfaces: vec![access_control::ConsoleInterfaceInventoryEntry {
                interface_id: "missing".into(),
                route: access_control::ConsoleRouteBinding {
                    method: "GET".into(),
                    path: "/api/console/missing/:id".into(),
                },
                summary: "Read missing route".into(),
                description: "Missing OpenAPI contract.".into(),
                authorization_operation_id: None,
            }],
        }),
    };
    for (id, reason) in [
        ("json", None),
        ("stream", Some("unsupported_response_media_type")),
        ("empty", None),
        ("upgrade", Some("unsupported_response_semantics")),
        ("internal", Some("unsupported_interface_scope")),
        ("missing", Some("missing_openapi_contract")),
    ] {
        let entry = get_openapi_capability_with(&dependencies, uuid::Uuid::nil(), id)
            .await
            .unwrap()
            .unwrap();
        let index = get_openapi_capability_index_entry_with(&dependencies, uuid::Uuid::nil(), id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(metadata(&entry), index);
        assert_eq!(index.disabled_reason, reason);
        assert_eq!(index.bindable, reason.is_none());
    }
    assert!(
        get_openapi_capability_with(&dependencies, uuid::Uuid::nil(), "not-found")
            .await
            .unwrap()
            .is_none()
    );
}
