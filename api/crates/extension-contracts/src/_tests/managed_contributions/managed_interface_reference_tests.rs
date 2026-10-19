//! Root #2014 R3: AC-015/017/018; executed only in the frozen probe / Test Batch.
use crate::*;
use serde_json::{json, Value};

fn schema_at(bytes: usize) -> ManagedProjectionContract {
    let mut schema = json!({"type":"string","maxLength":16384,"enum":[""]});
    let overhead = serde_json::to_vec(&schema).unwrap().len();
    // Many bounded alternatives exercise schema size without oversized business data.
    let count = bytes / 8192;
    schema["enum"] = Value::Array(
        (0..count)
            .map(|index| json!(format!("{index:04}{}", "x".repeat(8188))))
            .collect(),
    );
    while serde_json::to_vec(&schema).unwrap().len() > bytes {
        schema["enum"].as_array_mut().unwrap().pop();
    }
    let current = serde_json::to_vec(&schema).unwrap().len();
    if current < bytes {
        schema["enum"]
            .as_array_mut()
            .unwrap()
            .push(json!("y".repeat(bytes - current - 3)));
    }
    assert!(bytes > overhead);
    assert_eq!(serde_json::to_vec(&schema).unwrap().len(), bytes);
    ManagedProjectionContract {
        contract_id: "bounded.output".into(),
        contract_version: "1".into(),
        schema,
    }
}

#[test]
fn root_2014_r3_probe_independent_budgets() {
    for bytes in [
        MANAGED_INTERFACE_MAX_SCHEMA_BYTES,
        MANAGED_INTERFACE_MAX_SCHEMA_BYTES + 1,
        MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES,
        MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES + 1,
    ] {
        let contract = schema_at(bytes);
        assert_eq!(
            contract.compile().is_ok(),
            bytes <= MANAGED_INTERFACE_MAX_SCHEMA_BYTES
        );
        assert_eq!(
            contract.compile_for_registration().is_ok(),
            bytes <= MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES
        );
    }
    let contract = ManagedProjectionContract {
        contract_id: "bounded.output".into(),
        contract_version: "1".into(),
        schema: json!({"type":"array","items":{"type":"string","maxLength":16384},"maxItems":256}),
    };
    let compiled = contract.compile_for_registration().unwrap();
    assert_eq!(
        compiled.fingerprint(),
        contract.compile().unwrap().fingerprint()
    );
    // JSON array delimiters contribute 16 bytes for five strings.
    let value = json!([
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16368),
        ""
    ]);
    assert_eq!(
        serde_json::to_vec(&value).unwrap().len(),
        MANAGED_INTERFACE_MAX_PROJECTION_BYTES
    );
    let mut view = ManagedInterfaceReferenceView {
        contract: compiled.reference(),
        value,
    };
    compiled.validate_reference(&view).unwrap();
    view.value[4] = json!("x");
    assert!(view.validate().is_err());
    view.value[4] = json!("");
    for field in ["contract_id", "contract_version", "schema_fingerprint"] {
        let mut wire = serde_json::to_value(&view).unwrap();
        wire["contract"][field] = json!("wrong");
        let forged = serde_json::from_value(wire).unwrap();
        assert!(compiled.validate_reference(&forged).is_err());
    }
    let legacy = super::managed_interface_contract_tests::request(ManagedInterfaceInput::After {
        output: ManagedInterfaceView {
            contract: contract.clone(),
            value: json!(["small"]),
        },
    });
    legacy.validate().unwrap();
    let legacy_wire = serde_json::to_value(&legacy).unwrap();
    assert_eq!(
        legacy_wire["input"]["output"]["contract"]["schema"],
        contract.schema
    );
    let mut request = ManagedInterfaceReferenceHostFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V2.into(),
        call_id: legacy.call_id,
        handler: legacy.handler,
        interface_id: legacy.interface_id,
        interface_version: legacy.interface_version,
        context: legacy.context,
        input: ManagedInterfaceReferenceInput::After { output: view },
    };
    request.validate().unwrap();
    let wire = serde_json::to_value(&request).unwrap();
    assert!(wire["input"]["output"]["contract"].get("schema").is_none());
    let mut forged = wire;
    forged["input"]["output"]["contract"]["schema"] = contract.schema;
    assert!(serde_json::from_value::<ManagedInterfaceReferenceHostFrame>(forged).is_err());
    let mut reply = ManagedInterfaceReferenceWorkerFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V2.into(),
        call_id: request.call_id.clone(),
        phase: HookPhase::After,
        outcome: ManagedHookOutcome::Observed,
    };
    reply.validate_for(&request).unwrap();
    reply.outcome = ManagedHookOutcome::Continue;
    assert!(reply.validate_for(&request).is_err());
    reply.outcome = ManagedHookOutcome::Observed;
    reply.protocol = MANAGED_INTERFACE_PROTOCOL_V1.into();
    assert!(reply.validate_for(&request).is_err());
    request.protocol = MANAGED_INTERFACE_PROTOCOL_V1.into();
    assert!(request.validate().is_err());
    // Independently enforce request and reply byte caps even on otherwise correlating JSON.
    request.protocol = MANAGED_INTERFACE_PROTOCOL_V2.into();
    request.context.actor_id = Some(String::new());
    let size = serde_json::to_vec(&request).unwrap().len();
    request.context.actor_id = Some("a".repeat(MANAGED_INTERFACE_MAX_REQUEST_BYTES - size));
    request.validate().unwrap();
    request.context.actor_id.as_mut().unwrap().push('a');
    assert!(request.validate().is_err());
    reply.protocol = MANAGED_INTERFACE_PROTOCOL_V2.into();
    reply.call_id.clear();
    let size = serde_json::to_vec(&reply).unwrap().len();
    reply.call_id = "c".repeat(MANAGED_INTERFACE_MAX_REPLY_BYTES - size);
    request.call_id = reply.call_id.clone();
    reply.validate_for(&request).unwrap();
    reply.call_id.push('c');
    request.call_id = reply.call_id.clone();
    assert!(reply.validate_for(&request).is_err());
}
