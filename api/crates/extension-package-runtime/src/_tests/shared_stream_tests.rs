use super::{json, parse, record_fixture, Value};

fn shared_fixture() -> Value {
    let mut value = record_fixture();
    let mut operations = Vec::new();
    let input = json!({"type":"object","properties":{"body":{"type":"object","properties":{
        "record_id":{"type":"string"},"interval_ms":{"type":"integer","minimum":1}
    },"required":["record_id","interval_ms"]}},"required":["body"]});
    let sample =
        json!({"type":"object","properties":{"value":{"type":"number"}},"required":["value"]});
    for (index, name) in ["source", "resolve", "snapshot", "aggregate"]
        .iter()
        .enumerate()
    {
        let id = format!("managed_fixture.{name}");
        let mut operation = value["managed_service"]["operations"][0].clone();
        operation["interface_id"] = json!(id);
        operation["contribution_id"] = json!(id);
        operation["method"] = json!("POST");
        operation["path"] = json!(format!(
            "/api/console/managed-services/managed_fixture/{name}"
        ));
        operation["input_schema"] = input.clone();
        operation["output_schema"] = sample.clone();
        operation["record_permission"] = json!({"resource_code":"hosts","action_code":"open","selector":{"kind":"record","pointer":"/body/record_id"}});
        match index {
            0 => {
                operation["stream"] = json!({"input_schema":{"type":"object"},"event_schema":{
                "type":"object","properties":{"type":{"type":"string","enum":["sample","status"]},"sample":sample}
            },"shared_source":{"resolve_interface_id":"managed_fixture.resolve","interval_pointer":"/body/interval_ms"}})
            }
            1 => {
                operation["output_schema"] = json!({"type":"object","properties":{"revision":{"type":"string"}},"required":["revision"]})
            }
            2 => {
                operation["shared_snapshot"] = json!({"source_interface_id":"managed_fixture.source","value_pointer":"/sample","ready_pointer":"/type","ready_value":"sample","interval_pointer":"/body/interval_ms"})
            }
            3 => {
                operation
                    .as_object_mut()
                    .unwrap()
                    .remove("record_permission");
                operation["stream"] = json!({"input_schema":{"type":"object"},"event_schema":{"type":"object"},"aggregate_source_interface_id":"managed_fixture.source"});
            }
            _ => unreachable!(),
        }
        operations.push(operation);
        let mut contribution = value["managed"]["module"]["contributions"][0].clone();
        contribution["contribution_id"] = json!(id);
        value["managed"]["module"]["contributions"]
            .as_array_mut()
            .unwrap()
            .push(contribution);
        let mut binding = value["managed"]["execution_bindings"][0].clone();
        binding["contribution_id"] = json!(id);
        binding["runtime"]["capabilities"] = json!(["runtime_duplex/v1"]);
        value["managed"]["execution_bindings"]
            .as_array_mut()
            .unwrap()
            .push(binding);
    }
    value["managed_service"]["operations"] = json!(operations);
    value["managed_service"]["record_resources"][0]["actions"] = json!([{"action_code":"open","label":"Open","description":"Read a record","operation_ids":["managed_fixture.source","managed_fixture.resolve","managed_fixture.snapshot"]}]);
    value
}

#[test]
fn shared_stream_contract_round_trips_and_legacy_fields_stay_absent() {
    let parsed = parse(&shared_fixture()).unwrap();
    let operations = &parsed.managed_service.as_ref().unwrap().operations;
    assert_eq!(
        operations[0]
            .stream
            .as_ref()
            .unwrap()
            .shared_source
            .as_ref()
            .unwrap()
            .resolve_interface_id,
        "managed_fixture.resolve"
    );
    assert!(operations[3].record_permission.is_none());
    let serialized = json!({"managed_service":parsed.managed_service});
    assert_eq!(
        serialized["managed_service"]["operations"][2]["shared_snapshot"]["ready_value"],
        "sample"
    );
    let legacy = json!({"managed_service":parse(&super::fixture()).unwrap().managed_service});
    assert!(legacy["managed_service"]["operations"][0]
        .get("shared_snapshot")
        .is_none());
}

#[test]
fn shared_stream_contract_rejects_cross_owner_recursive_scope_and_projection_forgery() {
    for (pointer, replacement) in [
        (
            "/managed_service/operations/0/stream/shared_source/resolve_interface_id",
            json!("other.resolve"),
        ),
        (
            "/managed_service/operations/0/stream/shared_source/interval_pointer",
            json!("/body/record_id"),
        ),
        (
            "/managed_service/operations/0/stream/shared_source/interval_pointer",
            json!("/body/interval_ms~2"),
        ),
        (
            "/managed_service/operations/0/stream/aggregate_source_interface_id",
            json!("managed_fixture.source"),
        ),
        (
            "/managed_service/operations/1/output_schema/properties/revision/type",
            json!("integer"),
        ),
        (
            "/managed_service/operations/1/output_schema/required",
            json!([]),
        ),
        (
            "/managed_service/operations/1/record_permission/selector/pointer",
            json!("/body/other_id"),
        ),
        (
            "/managed_service/operations/2/shared_snapshot/source_interface_id",
            json!("managed_fixture.aggregate"),
        ),
        (
            "/managed_service/operations/2/shared_snapshot/value_pointer",
            json!("/missing"),
        ),
        (
            "/managed_service/operations/2/shared_snapshot/ready_value",
            json!("forged"),
        ),
        (
            "/managed_service/operations/2/shared_snapshot/ready_pointer",
            json!(""),
        ),
        (
            "/managed_service/operations/2/output_schema",
            json!({"type":"string"}),
        ),
        (
            "/managed_service/operations/3/stream/aggregate_source_interface_id",
            json!("managed_fixture.aggregate"),
        ),
        (
            "/managed_service/operations/3/record_permission",
            json!({"resource_code":"hosts","action_code":"open","selector":{"kind":"record","pointer":"/body/record_id"}}),
        ),
    ] {
        let mut value = shared_fixture();
        // Replacements may add optional declaration fields.
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap()[key] = replacement;
        assert!(parse(&value).is_err(), "{pointer} must fail closed");
    }
}

#[test]
fn shared_source_rejects_unrequired_cadence_and_draft_record_identity() {
    for (pointer, replacement) in [
        (
            "/managed_service/operations/0/input_schema/properties/body/required",
            json!(["record_id"]),
        ),
        (
            "/managed_service/operations/0/record_permission/selector/allow_missing",
            json!(true),
        ),
        (
            "/managed_service/operations/0/record_permission/action_code",
            json!("view"),
        ),
    ] {
        let mut value = shared_fixture();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap()[key] = replacement;
        assert!(parse(&value).is_err(), "{pointer}");
    }
}

#[test]
fn shared_source_and_snapshot_allow_optional_cadence_with_positive_schema_default() {
    let mut value = shared_fixture();
    for index in [0, 2] {
        value["managed_service"]["operations"][index]["input_schema"]["properties"]["body"]
            ["required"] = json!(["record_id"]);
        value["managed_service"]["operations"][index]["input_schema"]["properties"]["body"]
            ["properties"]["interval_ms"]["default"] = json!(5000);
    }
    assert!(parse(&value).is_ok());
    value["managed_service"]["operations"][0]["input_schema"]["properties"]["body"]["properties"]
        ["interval_ms"]["default"] = json!(0);
    assert!(parse(&value).is_err());
}

#[test]
fn shared_source_close_output_must_match_its_frozen_terminal_schema() {
    let mut value = shared_fixture();
    value["managed_service"]["operations"][0]["stream"]["shared_source"]["close_output"] =
        json!({"value":1});
    assert!(parse(&value).is_ok());
    value["managed_service"]["operations"][0]["stream"]["shared_source"]["close_output"] =
        json!({"value":"forged"});
    assert!(parse(&value).is_err());
}
