use super::*;
use serde_json::json;

fn field(code: &str, kind: domain::ModelFieldKind) -> domain::ModelFieldRecord {
    domain::ModelFieldRecord {
        id: Uuid::nil(),
        data_model_id: Uuid::nil(),
        code: code.into(),
        title: code.into(),
        description: None,
        physical_column_name: code.into(),
        external_field_key: None,
        field_kind: kind,
        is_system: true,
        is_writable: false,
        is_required: false,
        api_required: false,
        is_unique: false,
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order: 0,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    }
}

#[test]
fn missing_report_dimensions_compile_to_sql_null_predicates() {
    for code in ["created_by", "requested_model_id"] {
        let field = field(code, domain::ModelFieldKind::String);
        for (operator, predicate) in [
            (domain::ResourceFilterOperator::Eq, "is null"),
            (domain::ResourceFilterOperator::Ne, "is not null"),
        ] {
            let mut query = QueryBuilder::<Postgres>::new("select 1 where ");
            append_field_filter_expr(&mut query, &field, operator, &Value::Null).unwrap();
            assert_eq!(
                query.sql(),
                format!("select 1 where \"{code}\" {predicate}")
            );
        }
    }
}

#[test]
fn populated_and_json_null_filters_keep_bound_value_semantics() {
    for (field, value) in [
        (
            field("requested_model_id", domain::ModelFieldKind::String),
            json!("model-A"),
        ),
        (field("meta", domain::ModelFieldKind::Json), Value::Null),
    ] {
        let mut query = QueryBuilder::<Postgres>::new("");
        append_field_filter_expr(
            &mut query,
            &field,
            domain::ResourceFilterOperator::Eq,
            &value,
        )
        .unwrap();
        assert_eq!(query.sql(), format!("\"{}\" = $1", field.code));
    }
}
