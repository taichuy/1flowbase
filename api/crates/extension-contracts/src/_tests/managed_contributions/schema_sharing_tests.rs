use super::*;

fn legacy_compile(
    contract: &ManagedProjectionContract,
    schema_limit: usize,
) -> ContractResult<CompiledManagedProjection> {
    if !identity(&contract.contract_id) || !identity(&contract.contract_version) {
        return Err(ManagedInterfaceContractError(
            "invalid projection identity".into(),
        ));
    }
    let schema_bytes = serde_json::to_vec(&contract.schema)
        .map_err(|_| schema_violation("$", "serialization failed"))?
        .len();
    if schema_bytes > schema_limit {
        return Err(schema_violation(
            "$",
            &format!("serialized_bytes actual={schema_bytes} limit={schema_limit}"),
        ));
    }
    validate_schema_shape(&contract.schema, 0, "$")?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&contract.schema)
        .map_err(|_| schema_violation("$", "draft202012 compilation failed"))?;
    let mut canonical = contract.schema.clone();
    canonical.sort_all_objects();
    let fingerprint = format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(
            serde_json::to_vec(&(&contract.contract_id, &contract.contract_version, canonical,))
                .map_err(|_| invalid_schema())?
        ))
    );
    Ok(CompiledManagedProjection {
        validator: Arc::new(validator),
        fingerprint,
        contract_id: contract.contract_id.clone(),
        contract_version: contract.contract_version.clone(),
    })
}

fn contract(id: &str, schema: Value) -> ManagedProjectionContract {
    ManagedProjectionContract {
        contract_id: id.into(),
        contract_version: "1".into(),
        schema,
    }
}

#[test]
fn registration_sharing_matches_legacy_identity_schema_and_error_matrix() {
    use serde_json::json;
    let schemas = [
        json!({"type":"null"}),
        json!({"type":"boolean"}),
        json!({"type":"object","properties":{},"additionalProperties":false}),
        Value::Null,
        json!(true),
        json!({"type":"string"}),
        json!({"type":"integer","minimum":"wrong"}),
        json!({"type":"object","properties":{},"additionalProperties":true}),
    ];
    let values = [
        Value::Null,
        json!(true),
        json!({}),
        json!(12),
        json!("text"),
        json!({"extra":1}),
    ];
    let mut compiler = ManagedProjectionCompiler::default();
    let mut known_errors = 0;
    for id in ["a", "b", "", " "] {
        for schema in &schemas {
            let c = contract(id, schema.clone());
            let before = serde_json::to_vec(&c.schema).unwrap();
            let old = legacy_compile(&c, crate::MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES);
            let new =
                compiler.compile_for_registration(&c.contract_id, &c.contract_version, &c.schema);
            match (old, new) {
                (Err(a), Err(b)) => {
                    assert_eq!(a, b);
                    known_errors += 1;
                }
                (Ok(a), Ok(b)) => {
                    assert_eq!(a.reference(), b.reference());
                    for v in &values {
                        let expected = a.validate(v);
                        let actual = b.validate(v);
                        if expected.is_err() {
                            known_errors += 1;
                        }
                        assert_eq!(expected, actual);
                    }
                    for field in 0..3 {
                        let mut reference = a.reference();
                        match field {
                            0 => reference.contract_id = "forged".into(),
                            1 => reference.contract_version = "2".into(),
                            _ => reference.schema_fingerprint = "sha256:forged".into(),
                        };
                        let view = crate::ManagedInterfaceReferenceView {
                            contract: reference,
                            value: Value::Null,
                        };
                        assert!(a.validate_reference(&view).is_err());
                        assert!(b.validate_reference(&view).is_err());
                        known_errors += 1;
                    }
                }
                _ => panic!("legacy compilation outcome changed"),
            }
            assert_eq!(serde_json::to_vec(&c.schema).unwrap(), before);
        }
    }
    eprintln!(
        "GM-P2 known identity/schema/value/reference errors retained {known_errors}/{known_errors}"
    );
}

#[test]
fn registration_shares_program_only_and_keeps_original_order_and_generation_lifetime() {
    use serde_json::json;
    let schema = json!({"type":"string","maxLength":32});
    let original = serde_json::to_vec(&schema).unwrap();
    let mut compiler = ManagedProjectionCompiler::default();
    let a = compiler
        .compile_for_registration("a", "1", &schema)
        .unwrap();
    let b = compiler
        .compile_for_registration("b", "1", &schema)
        .unwrap();
    assert!(Arc::ptr_eq(&a.validator, &b.validator));
    assert_ne!(a.reference(), b.reference());
    let swapped = crate::ManagedInterfaceReferenceView {
        contract: b.reference(),
        value: json!("valid"),
    };
    assert!(a.validate_reference(&swapped).is_err());
    let changed = compiler
        .compile_for_registration("a", "1", &json!({"type":"boolean"}))
        .unwrap();
    assert!(!Arc::ptr_eq(&a.validator, &changed.validator));
    assert_ne!(a.fingerprint(), changed.fingerprint());
    let reordered: Value = serde_json::from_str(r#"{"maxLength":32,"type":"string"}"#).unwrap();
    let reordered_bytes = serde_json::to_vec(&reordered).unwrap();
    assert_ne!(original, reordered_bytes);
    let c = compiler
        .compile_for_registration("a", "1", &reordered)
        .unwrap();
    assert_eq!(a.fingerprint(), c.fingerprint());
    assert_eq!(serde_json::to_vec(&schema).unwrap(), original);
    assert_eq!(serde_json::to_vec(&reordered).unwrap(), reordered_bytes);
    let weak = Arc::downgrade(&a.validator);
    drop(compiler);
    drop(b);
    assert!(weak.upgrade().is_some());
    let a = Arc::new(a);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let a = a.clone();
            scope.spawn(move || {
                for _ in 0..100 {
                    a.validate(&json!("valid")).unwrap();
                    assert!(a.validate(&json!(true)).is_err());
                }
            });
        }
    });
    drop(a);
    assert!(weak.upgrade().is_none());
}

#[test]
fn registration_cache_never_bypasses_identity_budget_shape_or_compile_errors() {
    use serde_json::json;
    let mut compiler = ManagedProjectionCompiler::default();
    let mut known_errors = 0;
    for limit in [
        MANAGED_INTERFACE_MAX_SCHEMA_BYTES,
        crate::MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES,
    ] {
        for extra in [0, 1] {
            let mut schema = json!({"type":"string","maxLength":16384,"enum":[""]});
            let overhead = serde_json::to_vec(&schema).unwrap().len();
            schema["enum"][0] = json!("x".repeat(limit + extra - overhead));
            let c = contract("budget", schema);
            let old = legacy_compile(&c, limit);
            let new = compile_projection(
                &c.contract_id,
                &c.contract_version,
                &c.schema,
                limit,
                Some(&mut compiler.validators),
            );
            assert_eq!(old.as_ref().err(), new.as_ref().err());
            assert_eq!(old.is_ok(), extra == 0);
            if extra == 1 {
                known_errors += 1;
            }
            // Registration can warm a schema larger than the wire budget.
            let _ = compiler.compile_for_registration("budget", "1", &c.schema);
            assert_eq!(
                legacy_compile(&c, MANAGED_INTERFACE_MAX_SCHEMA_BYTES).err(),
                c.compile().err()
            );
            let mut invalid = c.clone();
            invalid.contract_id.clear();
            assert_eq!(
                legacy_compile(&invalid, limit).err(),
                compile_projection(
                    "",
                    "1",
                    &invalid.schema,
                    limit,
                    Some(&mut compiler.validators)
                )
                .err()
            );
            known_errors += 1;
        }
    }
    for schema in [
        json!({"type":"string","maxLength":1,"unknown":true}),
        json!({"type":"integer","minimum":"bad"}),
        json!({"type":"array","maxItems":257,"items":{"type":"null"}}),
    ] {
        let c = contract("invalid", schema);
        for attempt in 0..2 {
            let old = legacy_compile(&c, crate::MANAGED_INTERFACE_MAX_REGISTRATION_SCHEMA_BYTES)
                .err()
                .unwrap();
            let new = compiler
                .compile_for_registration("invalid", "1", &c.schema)
                .err()
                .unwrap();
            assert_eq!(old, new);
            if attempt == 0 {
                known_errors += 1;
            }
        }
    }
    eprintln!("GM-P2 known budget/precedence/shape/compiler errors retained {known_errors}/{known_errors}");
}
