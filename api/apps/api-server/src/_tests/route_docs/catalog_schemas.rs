use serde_json::{json, Value};

use crate::openapi_interface::{catalog_entry_from_operation, OpenApiInterfaceCatalogEntry};

fn document(schema: Value, components: Value) -> Value {
    json!({
        "openapi": "3.1.0", "info": {"title": "Catalog schemas", "version": "1"},
        "paths": {"/api/demo/tree": {"post": {
            "operationId": "tree",
            "parameters": [{"$ref": "#/components/parameters/Tree"}],
            "requestBody": {"$ref": "#/components/requestBodies/Tree"},
            "responses": {"200": {"$ref": "#/components/responses/Tree"}}
        }}},
        "components": {
            "schemas": components,
            "parameters": {"Tree": {"name": "tree", "in": "query", "schema": schema}},
            "requestBodies": {"Tree": {"required": true, "content": {
                "application/json": {"schema": schema}
            }}},
            "responses": {"Tree": {"description": "Tree", "content": {
                "application/json": {"schema": schema}
            }}}
        }
    })
}

fn project(document: Value) -> OpenApiInterfaceCatalogEntry {
    let registry = crate::openapi_docs::build_api_docs_registry(document).unwrap();
    let operation = &registry.category_operations("demo").unwrap().operations[0];
    let spec = registry.operation_spec("tree").unwrap();
    let view = registry.operation_spec_view("tree").unwrap();
    let full = catalog_entry_from_operation(operation, &spec).unwrap();
    let scoped = crate::openapi_interface::catalog_entry_from_view(operation, &view).unwrap();
    assert_eq!(
        serde_json::to_value(&full).unwrap(),
        serde_json::to_value(scoped).unwrap(),
        "scoped component resolution must match the final materialized scope"
    );
    full
}

fn node_schema(reference: &str) -> Value {
    json!({
        "type": "object", "required": ["id", "children"], "additionalProperties": false,
        "properties": {
            "id": {"type": "integer"},
            "children": {"type": "array", "items": {"$ref": reference}}
        }
    })
}

fn tree(depth: usize, leaf_id: Value) -> Value {
    let mut tree = json!({"id": leaf_id, "children": []});
    for _ in 0..depth {
        tree = json!({"id": 1, "children": [tree]});
    }
    tree
}

fn assert_validates(schema: &Value, valid: &Value, invalid: &Value) {
    let validator = jsonschema::validator_for(schema).expect("extracted schema must compile alone");
    assert!(
        validator.is_valid(valid),
        "valid recursive instance rejected"
    );
    assert!(
        !validator.is_valid(invalid),
        "deep invalid instance accepted"
    );
}

#[test]
fn mcp_role_permission_catalog_recursive_request_response_and_descriptor_roots_are_closed() {
    let entry = project(document(
        json!({"$ref": "#/components/schemas/Node"}),
        json!({
            "Node": node_schema("#/components/schemas/Node"),
            "Unused": {"type": "string", "description": "must-not-be-bundled"}
        }),
    ));
    let valid = tree(64, json!(1));
    let invalid = tree(64, json!("wrong-type"));
    assert_validates(&entry.response_schema, &valid, &invalid);
    assert_validates(
        &entry.request_schema,
        &json!({"body": valid, "query": {"tree": valid}}),
        &json!({"body": invalid, "query": {"tree": valid}}),
    );
    assert_validates(
        &entry.request_schema,
        &json!({"body": valid, "query": {"tree": valid}}),
        &json!({"body": valid, "query": {"tree": invalid}}),
    );
    for descriptor in &entry.parameter_descriptors {
        let (valid, invalid) = match descriptor.name.as_str() {
            "tree" => (valid.clone(), invalid.clone()),
            "children" => (json!([valid]), json!([invalid])),
            "id" => (json!(1), json!("wrong-type")),
            other => panic!("unexpected descriptor {other}"),
        };
        assert_validates(&descriptor.schema, &valid, &invalid);
    }
    assert_eq!(
        entry.response_schema["properties"]["children"]["type"],
        "array"
    );
    let encoded = serde_json::to_string(&entry.response_schema).unwrap();
    assert!(!encoded.contains("must-not-be-bundled"));
    assert!(
        encoded.len() < 3000,
        "recursive definitions must be finite and reachable only"
    );
}

#[test]
fn mcp_role_permission_catalog_mutual_recursion_and_escaped_component_pointers_remain_strict() {
    let entry = project(document(
        json!({"$ref": "#/components/schemas/A~1B~0C"}),
        json!({
            "A/B~C": node_schema("#/components/schemas/Next"),
            "Next": {"$ref": "#/components/schemas/A~1B~0C"}
        }),
    ));
    assert_validates(
        &entry.response_schema,
        &tree(48, json!(1)),
        &tree(48, json!(false)),
    );
    let mut extra_property = tree(0, json!(1));
    extra_property["unexpected"] = json!(true);
    assert_validates(&entry.response_schema, &tree(0, json!(1)), &extra_property);
}

#[test]
fn mcp_role_permission_catalog_deep_reference_chain_uses_finite_definitions_not_instance_depth_limits(
) {
    let mut schemas = serde_json::Map::new();
    let mut valid = json!(true);
    let mut invalid = json!("not-a-boolean");
    schemas.insert("N40".into(), json!({"type": "boolean"}));
    for n in (0..40).rev() {
        schemas.insert(
            format!("N{n}"),
            json!({
                "type": "object", "required": ["next"], "additionalProperties": false,
                "properties": {"next": {"$ref": format!("#/components/schemas/N{}", n + 1)}}
            }),
        );
        valid = json!({"next": valid});
        invalid = json!({"next": invalid});
    }
    let entry = project(document(
        json!({"$ref": "#/components/schemas/N0"}),
        schemas.into(),
    ));
    assert_validates(&entry.response_schema, &valid, &invalid);
    assert_validates(
        &entry.request_schema,
        &json!({"body": valid}),
        &json!({"body": invalid}),
    );
    for descriptor in &entry.parameter_descriptors {
        jsonschema::validator_for(&descriptor.schema).expect("deep descriptor must compile alone");
    }
}

#[test]
fn mcp_role_permission_catalog_shared_reference_graph_is_not_expanded_exponentially() {
    let mut schemas = serde_json::Map::new();
    schemas.insert("N32".into(), json!({"type": "integer"}));
    for n in (0..32).rev() {
        let reference = json!({"$ref": format!("#/components/schemas/N{}", n + 1)});
        schemas.insert(
            format!("N{n}"),
            json!({
                "type": "object", "properties": {"left": reference, "right": reference}
            }),
        );
    }
    let entry = project(document(
        json!({"$ref": "#/components/schemas/N0"}),
        schemas.into(),
    ));
    jsonschema::validator_for(&entry.response_schema).expect("shared graph must compile");
    assert!(serde_json::to_vec(&entry.response_schema).unwrap().len() < 20_000);
}

#[test]
fn mcp_role_permission_catalog_nonrecursive_contract_keeps_inline_shape_and_ref_siblings() {
    let entry = project(document(
        json!({"$ref": "#/components/schemas/Name", "description": "local description"}),
        json!({"Name": {"type": "string", "minLength": 2, "description": "component"}}),
    ));
    assert_eq!(
        entry.response_schema,
        json!({
            "type": "string", "minLength": 2, "description": "local description"
        })
    );
    assert_validates(&entry.response_schema, &json!("ok"), &json!("x"));
    for descriptor in &entry.parameter_descriptors {
        assert_eq!(descriptor.schema, entry.response_schema);
    }
}

#[test]
fn mcp_role_permission_catalog_missing_references_still_fail_closed_even_if_output_has_same_pointer(
) {
    let original = document(json!({"type": "string"}), json!({}));
    let registry = crate::openapi_docs::build_api_docs_registry(original.clone()).unwrap();
    let operation = &registry.category_operations("demo").unwrap().operations[0];
    for reference in ["#/components/schemas/Missing", "#/properties/value"] {
        let mut broken = original.clone();
        let schema = json!({
            "type": "object", "properties": {"value": {"$ref": reference}}
        });
        broken["components"]["responses"]["Tree"]["content"]["application/json"]["schema"] =
            schema.clone();
        broken["components"]["requestBodies"]["Tree"]["content"]["application/json"]["schema"] =
            schema.clone();
        broken["components"]["parameters"]["Tree"]["schema"] = schema;
        let entry = catalog_entry_from_operation(operation, &broken).unwrap();
        for schema in [&entry.response_schema, &entry.request_schema] {
            assert!(jsonschema::validator_for(schema).is_err(), "{reference}");
        }
        for descriptor in &entry.parameter_descriptors {
            assert!(
                jsonschema::validator_for(&descriptor.schema).is_err(),
                "{reference}"
            );
        }
    }
}

#[test]
fn mcp_role_permission_catalog_actual_role_frontstage_response_compiles_and_checks_deep_children() {
    let registry = crate::openapi_docs::build_default_api_docs_registry().unwrap();
    let operation = registry
        .category_operations("console")
        .unwrap()
        .operations
        .iter()
        .find(|operation| operation.id == "roles.frontstage_routes.view")
        .unwrap();
    let view = registry.operation_spec_view(&operation.id).unwrap();
    let entry = crate::openapi_interface::catalog_entry_from_view(operation, &view).unwrap();
    let spec = registry.operation_spec(&operation.id).unwrap();
    let full = catalog_entry_from_operation(operation, &spec).unwrap();
    assert_eq!(entry.response_schema, full.response_schema);
    assert_eq!(entry.response_schema["properties"]["tree"]["type"], "array");
    let mut valid_node = json!({
        "id": "00000000-0000-0000-0000-000000000001", "kind": "page",
        "title": null, "slug": null, "children": []
    });
    let mut invalid_node = valid_node.clone();
    invalid_node["kind"] = json!(42);
    for _ in 0..64 {
        valid_node = json!({"id": "00000000-0000-0000-0000-000000000001",
            "kind": "page", "title": null, "slug": null, "children": [valid_node]});
        invalid_node = json!({"id": "00000000-0000-0000-0000-000000000001",
            "kind": "page", "title": null, "slug": null, "children": [invalid_node]});
    }
    let response = |node: Value| {
        json!({
            "role_code": "member", "checked_page_ids": [], "checked_tab_ids": [], "tree": [node]
        })
    };
    assert_validates(
        &entry.response_schema,
        &response(valid_node),
        &response(invalid_node),
    );
}

#[test]
fn mcp_role_permission_catalog_generated_definitions_do_not_replace_existing_defs() {
    let mut node = node_schema("#/components/schemas/Node");
    node["$defs"] = json!({"__catalog_refs": {"const": "reserved"}});
    let entry = project(document(
        json!({"$ref": "#/components/schemas/Node"}),
        json!({"Node": node}),
    ));
    assert_eq!(
        entry.response_schema["$defs"]["__catalog_refs"],
        json!({"const": "reserved"})
    );
    assert_validates(
        &entry.response_schema,
        &tree(32, json!(1)),
        &tree(32, json!("invalid")),
    );
}

#[test]
fn mcp_role_permission_catalog_repeated_terminal_refs_keep_nonrecursive_shape() {
    let entry = project(document(
        json!({"type": "object", "properties": {
            "first": {"$ref": "#/components/schemas/Name"},
            "last": {"$ref": "#/components/schemas/Name"}
        }}),
        json!({"Name": {"type": "string"}}),
    ));
    assert_eq!(
        entry.response_schema,
        json!({"type": "object", "properties": {
            "first": {"type": "string"}, "last": {"type": "string"}
        }})
    );
}

#[test]
fn mcp_role_permission_catalog_boolean_ref_remains_restrictive() {
    let entry = project(document(
        json!({"$ref": "#/components/schemas/Never"}),
        json!({"Never": false}),
    ));
    assert_eq!(entry.response_schema, json!(false));
    assert_eq!(entry.request_schema["properties"]["body"], json!(false));
    assert!(!jsonschema::validator_for(&entry.response_schema)
        .unwrap()
        .is_valid(&json!({})));
}

#[test]
fn mcp_role_permission_catalog_parameter_description_keeps_component_precedence() {
    let mut source = document(
        json!({"$ref": "#/components/schemas/Name"}),
        json!({"Name": {"type": "string", "description": "component description"}}),
    );
    source["components"]["parameters"]["Tree"]["description"] = json!("parameter description");
    let entry = project(source);
    assert_eq!(
        entry.request_schema["properties"]["query"]["properties"]["tree"]["description"],
        "component description"
    );
    let descriptor = entry
        .parameter_descriptors
        .iter()
        .find(|descriptor| descriptor.name == "tree")
        .unwrap();
    assert_eq!(
        descriptor.description.as_deref(),
        Some("parameter description")
    );
    assert_eq!(descriptor.schema["description"], "component description");
}
