#[test]
fn many_to_many_write_and_read_contracts_use_relation_arrays() {
    let field = writable_field(
        Uuid::nil(),
        "tags",
        domain::ModelFieldKind::ManyToMany,
        false,
    );
    let write_schema = field_schema(&field);
    let write = jsonschema::validator_for(&write_schema).unwrap();
    let id = Uuid::now_v7().to_string();
    assert_eq!(write_schema["items"]["format"], "uuid");
    assert!(write.is_valid(&json!([])));
    assert!(write.is_valid(&json!([id])));
    assert!(!write.is_valid(&json!(id)));
    assert!(!write.is_valid(&Value::Null));
    assert!(!write.is_valid(&json!([{"id":id}])));
    let read = jsonschema::validator_for(&record_field_schema(&field)).unwrap();
    assert!(read.is_valid(&json!([])));
    assert!(read.is_valid(&json!([id])));
    assert!(read.is_valid(&json!([{"id":id}])));
    assert!(!read.is_valid(&Value::Null));
}
