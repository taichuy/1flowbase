use super::*;

fn set_variable_aggregator_fixture(document: &mut Value, groups: Value, outputs: Value) {
    document["graph"]["nodes"][0]["config"]["input_fields"] = json!([{
        "key": "query",
        "valueType": "string"
    }]);
    document["graph"]["nodes"][1] = json!({
        "id": "node-aggregator",
        "type": "variable_aggregator",
        "alias": "First available",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "groups": {
                "kind": "variable_groups",
                "value": groups
            }
        },
        "outputs": outputs
    });
    document["graph"]["edges"][0]["target"] = json!("node-aggregator");
}

// Root AC-001/012/013: groups are the ordered semantic source of outputs and selectors.
#[test]
fn compile_variable_aggregator_preserves_group_order_outputs_and_dependencies() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([
            {
                "key": "query",
                "valueType": "string",
                "candidates": [["node-start", "query"], ["node-start", "query"]]
            },
            {
                "key": "history",
                "valueType": "array",
                "candidates": [["node-start", "history"]]
            }
        ]),
        json!([
            { "key": "query", "title": "query", "valueType": "string" },
            { "key": "history", "title": "history", "valueType": "array" }
        ]),
    );

    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("variable aggregator should compile");
    let aggregator = &plan.nodes["node-aggregator"];

    assert!(plan
        .compile_issues
        .iter()
        .all(|issue| { issue.code != CompileIssueCode::UnsupportedNodeType }));
    assert_eq!(
        aggregator.bindings["groups"].selector_paths,
        vec![
            vec!["node-start".to_string(), "query".to_string()],
            vec!["node-start".to_string(), "query".to_string()],
            vec!["node-start".to_string(), "history".to_string()],
        ]
    );
    assert_eq!(
        aggregator
            .outputs
            .iter()
            .map(|output| (
                output.key.as_str(),
                output.title.as_str(),
                output.value_type.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("query", "query", "string"),
            ("history", "history", "array")
        ]
    );
    assert!(aggregator
        .dependency_node_ids
        .contains(&"node-start".to_string()));
    assert!(
        plan.topological_order
            .iter()
            .position(|id| id == "node-start")
            < plan
                .topological_order
                .iter()
                .position(|id| id == "node-aggregator")
    );
}

#[test]
fn compile_rejects_variable_aggregator_outputs_that_diverge_from_groups() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([{
            "key": "value",
            "valueType": "string",
            "candidates": [["node-start", "query"]]
        }]),
        json!([{ "key": "value", "title": "Wrong title", "valueType": "string" }]),
    );

    let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect_err("variable aggregator must reject extra public outputs");

    assert!(error
        .to_string()
        .contains("invalid variable_aggregator output contract"));
}

// Root AC-012/013/014/016/020: malformed and unsupported group contracts fail closed.
#[test]
fn compile_rejects_invalid_variable_aggregator_group_contracts() {
    let invalid_groups = [
        json!([]),
        json!([{ "key": "", "valueType": "string", "candidates": [["node-start", "query"]] }]),
        json!([{ "key": "value", "valueType": "json", "candidates": [["node-start", "query"]] }]),
        json!([{ "key": "value", "valueType": "string", "candidates": [] }]),
        json!([{ "key": "value", "valueType": "string", "candidates": [["node-start", ""]] }]),
        json!([
            { "key": "value", "valueType": "string", "candidates": [["node-start", "query"]] },
            { "key": "value", "valueType": "string", "candidates": [["node-start", "query"]] }
        ]),
    ];

    for groups in invalid_groups {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        set_variable_aggregator_fixture(
            &mut document,
            groups,
            json!([{ "key": "value", "title": "value", "valueType": "string" }]),
        );
        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
            .expect_err("invalid variable group contract must be rejected");
    }
}

// Root AC-016/020: group keys use the public output variable format and remain unique/nonempty.
#[test]
fn compile_rejects_invalid_variable_aggregator_group_keys() {
    let invalid_keys = ["", "   ", "bad-key", "has space", "变量"];

    for key in invalid_keys {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        set_variable_aggregator_fixture(
            &mut document,
            json!([{ "key": key, "valueType": "string", "candidates": [["node-start", "query"]] }]),
            json!([{ "key": key, "title": key, "valueType": "string" }]),
        );

        let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
            .expect_err("invalid variable aggregator group key must be rejected");
        assert!(
            format!("{error:#}").contains(
                "group keys must be unique, nonempty, and contain only ASCII letters, digits, or underscores"
            ),
            "key {key:?} failed with an unrelated error: {error:#}"
        );
    }

    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([
            { "key": "duplicate", "valueType": "string", "candidates": [["node-start", "query"]] },
            { "key": "duplicate", "valueType": "string", "candidates": [["node-start", "query"]] }
        ]),
        json!([
            { "key": "duplicate", "title": "duplicate", "valueType": "string" },
            { "key": "duplicate", "title": "duplicate", "valueType": "string" }
        ]),
    );

    let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect_err("duplicate variable aggregator group keys must be rejected");
    assert!(format!("{error:#}").contains("group keys must be unique"));
}

// Root AC-016/020: every ASCII alphanumeric/underscore position accepted by the UI also compiles.
#[test]
fn compile_accepts_variable_aggregator_group_keys_in_public_output_format() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([
            { "key": "Alpha1", "valueType": "string", "candidates": [["node-start", "query"]] },
            { "key": "_value", "valueType": "string", "candidates": [["node-start", "query"]] },
            { "key": "9lives", "valueType": "string", "candidates": [["node-start", "query"]] }
        ]),
        json!([
            { "key": "Alpha1", "title": "Alpha1", "valueType": "string" },
            { "key": "_value", "title": "_value", "valueType": "string" },
            { "key": "9lives", "title": "9lives", "valueType": "string" }
        ]),
    );

    FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("public output format variable aggregator group keys should compile");
}

#[test]
fn compile_rejects_legacy_variable_aggregator_candidates_binding() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([{ "key": "value", "valueType": "string", "candidates": [["node-start", "query"]] }]),
        json!([{ "key": "value", "title": "value", "valueType": "string" }]),
    );
    let groups = document["graph"]["nodes"][1]["bindings"]["groups"].take();
    document["graph"]["nodes"][1]["bindings"] = json!({ "candidates": groups });

    let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect_err("legacy candidates binding must be rejected");
    assert!(error
        .to_string()
        .contains("legacy candidates/value bindings"));
}

#[test]
fn compile_rejects_variable_aggregator_candidate_with_incompatible_declared_type() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([{ "key": "value", "valueType": "number", "candidates": [["node-start", "query"]] }]),
        json!([{ "key": "value", "title": "value", "valueType": "number" }]),
    );

    let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect_err("candidate type mismatch must be rejected by topology validation");
    assert!(error.to_string().contains("expects valueType number"));
}

#[test]
fn compile_normalizes_typed_array_candidate_to_array_group_type() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    set_variable_aggregator_fixture(
        &mut document,
        json!([{ "key": "items", "valueType": "array", "candidates": [["node-start", "query"]] }]),
        json!([{ "key": "items", "title": "items", "valueType": "array" }]),
    );
    document["graph"]["nodes"][0]["config"]["input_fields"][0]["valueType"] =
        json!("array[object]");

    FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
        .expect("typed upstream arrays must normalize to the array group type");
}

#[test]
fn compile_rejects_forbidden_upstream_variable_group_types() {
    for forbidden_type in ["json", "unknown", "protocol_context"] {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        set_variable_aggregator_fixture(
            &mut document,
            json!([{ "key": "value", "valueType": "object", "candidates": [["node-start", "query"]] }]),
            json!([{ "key": "value", "title": "value", "valueType": "object" }]),
        );
        document["graph"]["nodes"][0]["config"]["input_fields"][0]["valueType"] =
            json!(forbidden_type);

        let error = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
            .expect_err("forbidden upstream value types must be rejected");
        assert!(error.to_string().contains("forbidden upstream valueType"));
    }
}

// Root AC-012/013/014: backend declarations provide the same strict truth for all run-level namespaces.
#[test]
fn compile_variable_aggregator_accepts_declared_run_level_concrete_types() {
    let flow_id = Uuid::now_v7();
    let mut document = sample_document(flow_id);
    document["variables"] = json!({
        "conversation": [{
            "name": "approved",
            "valueType": "boolean",
            "description": ""
        }]
    });
    set_variable_aggregator_fixture(
        &mut document,
        json!([
            { "key": "user_id", "valueType": "string", "candidates": [["sys", "user_id"]] },
            { "key": "items", "valueType": "array", "candidates": [["env", "Items"]] },
            { "key": "approved", "valueType": "boolean", "candidates": [["conversation", "approved"]] },
            { "key": "trigger_type", "valueType": "string", "candidates": [["trigger", "type"]] }
        ]),
        json!([
            { "key": "user_id", "title": "user_id", "valueType": "string" },
            { "key": "items", "title": "items", "valueType": "array" },
            { "key": "approved", "title": "approved", "valueType": "boolean" },
            { "key": "trigger_type", "title": "trigger_type", "valueType": "string" }
        ]),
    );
    let mut context = compile_context();
    context.run_level_variables = vec![
        FlowCompileRunLevelVariable {
            selector: vec!["sys".to_string(), "user_id".to_string()],
            value_type: "string".to_string(),
        },
        FlowCompileRunLevelVariable {
            selector: vec!["env".to_string(), "Items".to_string()],
            value_type: "array[object]".to_string(),
        },
        FlowCompileRunLevelVariable {
            selector: vec!["trigger".to_string(), "type".to_string()],
            value_type: "string".to_string(),
        },
    ];

    FlowCompiler::compile(flow_id, "draft-1", &document, &context)
        .expect("declared concrete run-level selectors must compile");
}

#[test]
fn compile_variable_aggregator_rejects_unknown_run_level_selectors() {
    for selector in [
        ["sys", "missing"],
        ["env", "Missing"],
        ["conversation", "missing"],
        ["trigger", "missing"],
    ] {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        document["variables"] = json!({ "conversation": [] });
        set_variable_aggregator_fixture(
            &mut document,
            json!([{ "key": "value", "valueType": "string", "candidates": [selector] }]),
            json!([{ "key": "value", "title": "value", "valueType": "string" }]),
        );

        FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context())
            .expect_err("unknown run-level selectors must be rejected");
    }
}

#[test]
fn compile_variable_aggregator_rejects_incompatible_or_unknown_run_level_types() {
    for declared_type in ["number", "unknown"] {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        set_variable_aggregator_fixture(
            &mut document,
            json!([{ "key": "value", "valueType": "string", "candidates": [["sys", "user_id"]] }]),
            json!([{ "key": "value", "title": "value", "valueType": "string" }]),
        );
        let mut context = compile_context();
        context.run_level_variables = vec![FlowCompileRunLevelVariable {
            selector: vec!["sys".to_string(), "user_id".to_string()],
            value_type: declared_type.to_string(),
        }];

        FlowCompiler::compile(flow_id, "draft-1", &document, &context)
            .expect_err("incompatible and unknown run-level types must be rejected");
    }
}

#[test]
fn answer_aggregator_stream_sources_preserve_branch_reachability_and_exclusivity() {
    use orchestration_runtime::answer_presentation::AnswerPresentationPlan;

    // Direct targets exercise zero-length reachability; a shared target must belong to both handles.
    for (indirect, merged, repeated_source) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let flow_id = Uuid::now_v7();
        let mut document = sample_document(flow_id);
        add_second_llm_and_answer(
            &mut document,
            "{{node-aggregator.value}}{{node-aggregator.other}}",
            false,
        );
        let nodes = document["graph"]["nodes"].as_array_mut().unwrap();
        let mut branch = nodes[1].clone();
        branch["id"] = json!("node-if");
        branch["type"] = json!("if_else");
        branch["config"] = json!({});
        branch["outputs"] = json!([]);
        branch["bindings"] = json!({ "branches": { "kind": "if_else_branches", "value": {
            "branches": [
                { "id": "if", "kind": "if", "title": "If", "sourceHandle": "if",
                  "condition": { "operator": "and", "conditions": [{ "kind": "rule",
                    "left": ["node-start", "query"], "comparator": "contains",
                    "right": { "kind": "constant", "value": "vip" } }] } },
                { "id": "else", "kind": "else", "title": "Else", "sourceHandle": "else" }
            ]
        } } });
        let mut aggregator = branch.clone();
        aggregator["id"] = json!("node-aggregator");
        aggregator["type"] = json!("variable_aggregator");
        let second_source = if repeated_source {
            "node-llm"
        } else {
            "node-llm-2"
        };
        let groups = ["value", "other"].map(|key| {
            json!({ "key": key, "valueType": "string",
                "candidates": [["node-llm", "text"], [second_source, "text"]] })
        });
        aggregator["bindings"] =
            json!({ "groups": { "kind": "variable_groups", "value": groups } });
        let outputs = ["value", "other"]
            .map(|key| json!({ "key": key, "title": key, "valueType": "string" }));
        aggregator["outputs"] = json!(outputs);
        if indirect {
            let mut intermediate = nodes[1].clone();
            intermediate["id"] = json!("node-intermediate");
            nodes.push(intermediate);
        }
        nodes.extend([branch, aggregator]);
        let mut edges = vec![
            ("node-start", "node-if", None),
            (
                "node-if",
                if indirect {
                    "node-intermediate"
                } else {
                    "node-llm"
                },
                Some("if"),
            ),
            ("node-if", "node-llm-2", Some("else")),
            ("node-llm", "node-aggregator", None),
            ("node-llm-2", "node-aggregator", None),
            ("node-aggregator", "node-answer", None),
        ];
        if indirect {
            edges.push(("node-intermediate", "node-llm", None));
        }
        if merged {
            edges.push(("node-if", "node-llm", Some("else")));
        }
        document["graph"]["edges"] = Value::Array(
            edges
                .into_iter()
                .enumerate()
                .map(|(index, (source, target, handle))| {
                    json!({ "id": format!("edge-{index}"), "source": source, "target": target,
                    "sourceHandle": handle, "targetHandle": null })
                })
                .collect(),
        );

        let plan =
            FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();
        let presentation = AnswerPresentationPlan::from_plan(&plan).unwrap();
        let expected_sources = if merged || repeated_source { 0 } else { 2 };
        assert_eq!(
            presentation.stream_sources.len(),
            expected_sources,
            "indirect={indirect}, merged={merged}, repeated_source={repeated_source}"
        );
        for sources in presentation.stream_sources.values() {
            assert_eq!(
                sources
                    .iter()
                    .map(|source| source.node_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["node-llm", "node-llm-2"]
            );
        }
        assert!(!plan
            .compile_issues
            .iter()
            .any(|issue| issue.code == CompileIssueCode::InvalidAnswerPresentationOrder));
    }
}
