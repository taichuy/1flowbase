//! Independent query contract and proof mode, never part of append performance.
use super::*;
use sqlx::{Column, Row, TypeInfo, ValueRef};

// Frozen verbatim from business baseline 71ff2602. Keep this independent of the
// compiled production SQL so candidate-vs-expected cannot replace old-vs-new.
const BASELINE_QUERY: &str = "select f.*, runtime_original_json(f.input_payload,f.raw_json_payloads,'input_payload') as input_payload, runtime_original_json(f.output_payload,f.raw_json_payloads,'output_payload') as output_payload, runtime_original_json(f.error_payload,f.raw_json_payloads,'error_payload') as error_payload, (select account from users where id=f.created_by) as authorized_account from flow_runs f where f.id=$1";

const MAPPER_FIELDS: [&str; 28] = [
    "id",
    "application_id",
    "flow_id",
    "flow_draft_id",
    "compiled_plan_id",
    "debug_session_id",
    "flow_schema_version",
    "document_hash",
    "run_mode",
    "target_node_id",
    "title",
    "status",
    "input_payload",
    "output_payload",
    "error_payload",
    "created_by",
    "authorized_account",
    "api_key_id",
    "publication_version_id",
    "external_user",
    "external_conversation_id",
    "external_trace_id",
    "compatibility_mode",
    "idempotency_key",
    "started_at",
    "finished_at",
    "created_at",
    "updated_at",
];

pub(super) fn query_contract() -> (String, String, Vec<String>) {
    let refresh = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/src/orchestration_runtime_repository/application_run_logs/run_conversation_message_item_methods.rs"));
    let literal = refresh
        .split_once("async fn refresh_completed_output_projection(")
        .unwrap()
        .1
        .split_once("sqlx::query(")
        .unwrap()
        .1
        .trim_start();
    // The scoped product uses an ordinary string literal. Deserialize its quoted
    // spelling rather than depending on sqlx::query and the quote sharing a line.
    let mut strings = serde_json::Deserializer::from_str(literal).into_iter::<String>();
    let production = strings
        .next()
        .unwrap()
        .expect("actual compiled refresh SQL literal");
    assert!(production.starts_with("select "));
    assert!(production.ends_with("from flow_runs f where f.id=$1"));
    let mapper = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../storage/durable/postgres/src/orchestration_runtime_repository/record_mappers.rs"
    ));
    let mapper = mapper
        .split_once("pub(super) fn map_flow_run_record(")
        .unwrap()
        .1
        .split_once("pub(super) fn map_node_run_record(")
        .unwrap()
        .0;
    let actual_fields = mapper
        .lines()
        .filter(|line| line.contains("row.get"))
        .map(|line| line.split('"').nth(1).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_fields, MAPPER_FIELDS,
        "review all mapper fields if the contract changes"
    );
    let fields = MAPPER_FIELDS
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>();
    let columns = MAPPER_FIELDS
        .iter()
        .map(|field| match *field {
            "input_payload" | "output_payload" | "error_payload" => {
                format!("runtime_original_json(f.{field},f.raw_json_payloads,'{field}') as {field}")
            }
            "authorized_account" => {
                "(select account from users where id=f.created_by) as authorized_account".into()
            }
            _ => format!("f.{field}"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let precise = format!("select {columns} from flow_runs f where f.id=$1");
    write_json(
        output().join("query-contract.json"),
        &json!({
            "source":"actual compiled refresh SQL and independently frozen expected mapper field list",
            "baseline_71ff":BASELINE_QUERY,"production":production,
            "precise_read_only_control":precise,"mapper_fields":fields,
        }),
    );
    (production, precise, fields)
}

pub(super) async fn alias_sentinel(store: &PgControlPlaneStore) {
    let physical = json!({"sentinel":"physical \\u0000"});
    let restored = json!({"sentinel":"restored\0"});
    let sidecar = json!({"input_payload":serde_json::to_string(&restored).unwrap()});
    let row = sqlx::query("select $1::jsonb as input_payload, runtime_original_json($1::jsonb,$2::jsonb,'input_payload') as input_payload")
        .bind(&physical).bind(&sidecar).fetch_one(store.pool()).await.unwrap();
    let first: Value = row.try_get(0).unwrap();
    let last: Value = row.try_get(1).unwrap();
    let named: Value = row.try_get("input_payload").unwrap();
    assert_eq!(first, physical);
    assert_eq!(last, restored);
    assert_ne!(first, last);
    assert_eq!(named, last, "SQLx must select the final restored alias");
    assert_eq!(
        row.try_get_raw("input_payload")
            .unwrap()
            .as_bytes()
            .unwrap(),
        row.try_get_raw(1).unwrap().as_bytes().unwrap()
    );
    write_json(
        output().join("alias-sentinel.json"),
        &json!({
            "physical":first,"restored":last,"named":named,"last_alias_selected":true,
            "physical_ordinal":0,"restored_ordinal":1,
            "physical_type":row.columns()[0].type_info().name(),
            "restored_type":row.columns()[1].type_info().name(),
        }),
    );
}

fn raw_length(row: &sqlx::postgres::PgRow, index: usize) -> usize {
    let value = row.try_get_raw(index).unwrap();
    if value.is_null() {
        0
    } else {
        value.as_bytes().unwrap().len()
    }
}

fn row_metadata(row: &sqlx::postgres::PgRow, fields: &[String]) -> Value {
    let mut columns = Vec::new();
    let mut selected_bytes = 0;
    let mut total = 0;
    for (index, column) in row.columns().iter().enumerate() {
        let selected = fields.iter().any(|field| field == column.name())
            && row
                .columns()
                .iter()
                .rposition(|c| c.name() == column.name())
                == Some(index);
        let bytes = raw_length(row, index);
        total += bytes;
        if selected {
            selected_bytes += bytes;
        }
        let value = row.try_get_raw(index).unwrap();
        columns.push(
            json!({"ordinal":index,"name":column.name(),"type":column.type_info().name(),
            "format":format!("{:?}",value.format()),"is_null":value.is_null(),
            "value_bytes":bytes,"effective_mapper_field":selected}),
        );
    }
    json!({"columns":columns,"column_count":row.columns().len(),"raw_value_bytes":total,
        "selected_value_bytes":selected_bytes,"unselected_value_bytes":total-selected_bytes,
        "data_row_body_bytes":2+4*row.columns().len()+total,
        "scope":"actual PgValueRef bytes plus DataRow body framing; excludes message header, allocator and DB-internal JSONB storage"})
}

fn assert_selected_bytes_equal(
    left: &sqlx::postgres::PgRow,
    right: &sqlx::postgres::PgRow,
    fields: &[String],
) {
    assert_eq!(right.columns().len(), fields.len());
    for (index, field) in fields.iter().enumerate() {
        assert_eq!(right.columns()[index].name(), field);
        let left = left.try_get_raw(field.as_str()).unwrap();
        let right = right.try_get_raw(field.as_str()).unwrap();
        assert_eq!(left.type_info(), right.type_info(), "type for {field}");
        assert_eq!(left.format(), right.format(), "wire format for {field}");
        assert_eq!(left.is_null(), right.is_null(), "null for {field}");
        if !left.is_null() {
            assert_eq!(
                left.as_bytes().unwrap(),
                right.as_bytes().unwrap(),
                "bytes for {field}"
            );
        }
    }
}

fn decoded_string_bytes(value: &Value) -> usize {
    match value {
        Value::String(s) => s.len(),
        Value::Array(v) => v.iter().map(decoded_string_bytes).sum(),
        Value::Object(v) => v
            .iter()
            .map(|(k, v)| k.len() + decoded_string_bytes(v))
            .sum(),
        _ => 0,
    }
}

pub(super) async fn row_proof(store: &PgControlPlaneStore, run: Uuid, trace: &mut Probe) {
    let (production, precise, fields) = query_contract();
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("set transaction isolation level repeatable read, read only")
        .execute(&mut *tx)
        .await
        .unwrap();
    let snapshot: (String,String,String) = sqlx::query_as("select txid_current_snapshot()::text,current_setting('transaction_isolation'),current_setting('transaction_read_only')")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(snapshot.1, "repeatable read");
    assert_eq!(snapshot.2, "on");
    trace.begin("context-baseline-row-read");
    let baseline = sqlx::query(BASELINE_QUERY)
        .bind(run)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    trace.point("context-baseline-row-held-no-unused-json-decode", true);
    let baseline_metadata = row_metadata(&baseline, &fields);
    trace.begin("context-precise-row-read");
    let precise = sqlx::query(sqlx::AssertSqlSafe(precise))
        .bind(run)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    trace.point("context-baseline-and-precise-rows-held", true);
    assert_selected_bytes_equal(&baseline, &precise, &fields);
    let precise_metadata = row_metadata(&precise, &fields);
    let production = sqlx::query(sqlx::AssertSqlSafe(production))
        .bind(run)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_selected_bytes_equal(&production, &precise, &fields);
    let production_metadata = row_metadata(&production, &fields);
    trace.begin("context-selected-json-decode");
    let input: Value = production.try_get("input_payload").unwrap();
    let decoded_output: Option<Value> = production.try_get("output_payload").unwrap();
    let error: Option<Value> = production.try_get("error_payload").unwrap();
    trace.point("context-mapper-json-decoded", true);
    let decoded = json!({"input_string_key_utf8_bytes":decoded_string_bytes(&input),
        "output_string_key_utf8_bytes":decoded_output.as_ref().map(decoded_string_bytes),
        "error_string_key_utf8_bytes":error.as_ref().map(decoded_string_bytes),
        "log_context_and_sidecar_json_values_decoded":false,
        "scope":"logical UTF8 bytes in selected JSON keys/strings; separate from Rust allocations"});
    drop((input, decoded_output, error, baseline, precise, production));
    trace.point("context-diagnostic-rows-dropped", true);
    tx.rollback().await.unwrap();
    write_json(
        output().join("row-bytes.json"),
        &json!({
        "baseline":baseline_metadata,"production":production_metadata,"precise":precise_metadata,
        "mapper_fields":fields,"selected_columns_byte_type_format_null_equivalence":true,
        "snapshot":snapshot.0,"isolation":snapshot.1,"read_only":snapshot.2,"decoded":decoded}),
    );
}
