use super::*;
use runtime_extension_sdk::PluginDataRow;

fn row_result(values: BTreeMap<String, PluginDataValue>) -> PluginDataOperationResult {
    PluginDataOperationResult::Rows {
        rows: vec![PluginDataRow { values }],
    }
}

fn valid_values() -> BTreeMap<String, PluginDataValue> {
    BTreeMap::from([
        ("id".into(), PluginDataValue::Uuid("record-1".into())),
        ("name".into(), PluginDataValue::String("First".into())),
    ])
}

#[test]
fn rows_rejects_missing_id_without_unwinding() {
    let mut values = valid_values();
    values.remove("id");
    let _ = rows(&row_result(values));
}

#[test]
fn rows_rejects_wrong_id_type_without_unwinding() {
    let mut values = valid_values();
    values.insert("id".into(), PluginDataValue::String("record-1".into()));
    let _ = rows(&row_result(values));
}

#[test]
fn rows_rejects_missing_name_without_unwinding() {
    let mut values = valid_values();
    values.remove("name");
    let _ = rows(&row_result(values));
}

#[test]
fn rows_rejects_wrong_name_type_without_unwinding() {
    let mut values = valid_values();
    values.insert("name".into(), PluginDataValue::Uuid("record-2".into()));
    let _ = rows(&row_result(values));
}

#[test]
fn rows_returns_field_errors_and_rejects_a_batch_with_a_malformed_row() {
    let cases = [
        ("id", None, "fixture_row_id"),
        (
            "id",
            Some(PluginDataValue::String("record-1".into())),
            "fixture_row_id",
        ),
        ("name", None, "fixture_row_name"),
        (
            "name",
            Some(PluginDataValue::Uuid("record-2".into())),
            "fixture_row_name",
        ),
    ];
    for (field, value, error) in cases {
        let mut values = valid_values();
        if let Some(value) = value {
            values.insert(field.into(), value);
        } else {
            values.remove(field);
        }
        let response = PluginDataOperationResult::Rows {
            rows: vec![
                PluginDataRow {
                    values: valid_values(),
                },
                PluginDataRow { values },
            ],
        };
        assert_eq!(rows(&response), Err(error.to_owned()));
    }
}

#[test]
fn rows_preserves_valid_values_and_order_and_projects_only_record_fields() {
    let first_id = "00000000-0000-0000-0000-000000000002";
    let second_id = "00000000-0000-0000-0000-000000000001";
    let response = PluginDataOperationResult::Rows {
        rows: [(first_id, "Zulu \"first\""), (second_id, "Alpha 中文")]
            .into_iter()
            .map(|(id, name)| PluginDataRow {
                values: BTreeMap::from([
                    ("id".into(), PluginDataValue::Uuid(id.into())),
                    ("name".into(), PluginDataValue::String(name.into())),
                    (
                        "extra".into(),
                        PluginDataValue::String("not projected".into()),
                    ),
                ]),
            })
            .collect(),
    };
    assert_eq!(
        rows(&response),
        Ok(vec![
            json!({"id":first_id,"name":"Zulu \"first\""}),
            json!({"id":second_id,"name":"Alpha 中文"}),
        ])
    );
}

#[test]
fn rows_preserves_empty_and_non_row_results_as_empty_lists() {
    assert_eq!(
        rows(&PluginDataOperationResult::Rows { rows: vec![] }),
        Ok(vec![])
    );
    assert_eq!(
        rows(&PluginDataOperationResult::Count { count: 3 }),
        Ok(vec![])
    );
}
