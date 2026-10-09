use super::*;
use domain::{ResourceFilterExpr as F, ResourceFilterOperator as O};
use serde_json::json;
fn predicate(field: &str, operator: O, value: serde_json::Value) -> F {
    F::Field {
        field: field.into(),
        operator,
        value,
    }
}
#[test]
fn validate_rejects_unknown_fields_types_and_nontext_contains_before_empty_scan() {
    let fields = record_trajectory_query_fields();
    for bad in [
        predicate("raw", O::Includes, json!("secret")),
        predicate("request_id", O::Eq, json!("bogus")),
        predicate("sequence", O::Includes, json!("1")),
        predicate("created_at", O::Gt, json!("yesterday")),
        predicate("status", O::In, json!(["succeeded", 17])),
    ] {
        let error =
            validate_log_query_filter(&F::Any(vec![F::All(vec![]), bad]), &fields).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<crate::ControlPlaneContractError>(),
            Some(crate::ControlPlaneContractError::InvalidInput(_))
        ));
    }
}
#[test]
fn summary_filter_preserves_nullable_fields_and_all_any_semantics() {
    let fields = record_trajectory_query_fields();
    let record = json!({"category":"tool","name":"Search","namespace":null,"preview":"Short summary","status":"succeeded","sequence":19});
    let filter = F::All(vec![
        predicate("namespace", O::Eq, json!(null)),
        F::Any(vec![
            predicate("name", O::Includes, json!("SEARCH")),
            predicate("status", O::Eq, json!("failed")),
        ]),
        predicate("sequence", O::Gt, json!(18)),
    ]);
    validate_log_query_filter(&filter, &fields).unwrap();
    assert!(log_query_filter_matches(&filter, &record, &fields));
    assert!(!log_query_filter_matches(
        &predicate("namespace", O::Ne, json!("foo")),
        &record,
        &fields
    ));
    assert!(!log_query_filter_matches(&F::Any(vec![]), &record, &fields));
    assert!(log_query_filter_matches(&F::All(vec![]), &record, &fields));
    assert!(!log_query_filter_matches(
        &predicate("preview", O::Includes, json!("body-only keyword")),
        &record,
        &fields
    ));
}
#[test]
fn comparisons_use_actual_uuid_and_timestamp_types() {
    let fields = record_trajectory_query_fields();
    let record = json!({"request_id":"ABCDEFAB-CDEF-ABCD-EFAB-CDEFABCDEFAB","created_at":"2026-10-09T12:00:00+08:00"});
    assert!(log_query_filter_matches(
        &predicate(
            "request_id",
            O::Eq,
            json!("abcdefab-cdef-abcd-efab-cdefabcdefab")
        ),
        &record,
        &fields
    ));
    assert!(log_query_filter_matches(
        &predicate("created_at", O::Eq, json!("2026-10-09T04:00:00Z")),
        &record,
        &fields
    ));
}
fn trajectory_query() -> RecordClientTrajectoryQuery {
    RecordClientTrajectoryQuery {
        filter: F::All(vec![]),
        cursor: None,
        limit: 10,
        keyword: Some("answer".into()),
        search_sections: vec![],
    }
}
#[test]
fn search_defaults_to_result_and_never_accepts_raw_or_unknown_sections() {
    let mut query = trajectory_query();
    assert_eq!(log_query_search_sections(&query).unwrap(), vec!["result"]);
    for section in ["raw", "unknown"] {
        query.search_sections = vec![section.into()];
        assert!(log_query_search_sections(&query).is_err());
    }
    query.search_sections = vec!["overview".into(), "parameters".into(), "overview".into()];
    assert_eq!(
        log_query_search_sections(&query).unwrap(),
        vec!["overview", "parameters"]
    );
    query.keyword = Some(" ".into());
    assert!(log_query_search_sections(&query).is_err());
}
#[test]
fn snippets_restore_nul_and_multibyte_offsets_without_global_lowercase_offsets() {
    assert_eq!(
        log_query_snippet("before\0答案之后", "答案"),
        Some("before\0答案之后".into())
    );
    assert_eq!(
        log_query_snippet("İ ANSWER 结束", "answer"),
        Some("İ ANSWER 结束".into())
    );
    assert_eq!(log_query_snippet("other body", "secret"), None);
    let text = format!("{}答案{}", "前".repeat(200), "后".repeat(200));
    let snippet = log_query_snippet(&text, "答案").unwrap();
    assert!(snippet.contains("答案"));
    assert!(snippet.chars().count() < 170);
}
#[test]
fn cursors_bind_scope_visibility_filter_sort_and_search_but_not_page_size() {
    let scope = Uuid::from_u128(101);
    let app = Uuid::from_u128(102);
    let other = Uuid::from_u128(103);
    let mut query = ApplicationLogRecordsQuery {
        filter: F::All(vec![]),
        sort_field: "started_at".into(),
        descending: true,
        cursor: None,
        limit: 10,
    };
    let binding = application_log_query_fingerprint(scope, &[app, other], &query);
    query.limit = 20;
    assert_eq!(
        binding,
        application_log_query_fingerprint(scope, &[other, app, app], &query)
    );
    for changed in [
        application_log_query_fingerprint(Uuid::from_u128(104), &[app, other], &query),
        application_log_query_fingerprint(scope, &[app], &query),
    ] {
        assert!(validate_log_query_cursor_binding(1, &binding, &changed).is_err());
    }
    query.filter = predicate("status", O::Eq, json!("failed"));
    assert_ne!(
        binding,
        application_log_query_fingerprint(scope, &[app, other], &query)
    );
    query.filter = F::All(vec![]);
    query.descending = false;
    assert_ne!(
        binding,
        application_log_query_fingerprint(scope, &[app, other], &query)
    );
    assert!(validate_log_query_cursor_binding(2, &binding, &binding).is_err());
    let mut query = trajectory_query();
    let record = Uuid::from_u128(105);
    let sections = log_query_search_sections(&query).unwrap();
    let binding = record_trajectory_query_fingerprint(app, record, &query, &sections);
    query.keyword = Some("different".into());
    assert_ne!(
        binding,
        record_trajectory_query_fingerprint(app, record, &query, &sections)
    );
    assert_ne!(
        binding,
        record_trajectory_query_fingerprint(other, record, &trajectory_query(), &sections)
    );
    assert_ne!(
        binding,
        record_trajectory_query_fingerprint(
            app,
            Uuid::from_u128(106),
            &trajectory_query(),
            &sections
        )
    );
}

#[test]
fn nul_predicate_is_supported_by_restored_trajectory_and_rejected_for_task_text() {
    let filter = predicate("preview", O::Includes, json!("\u{0}"));
    validate_log_query_filter(&filter, &record_trajectory_query_fields()).unwrap();
    assert!(log_query_filter_matches(
        &filter,
        &json!({"preview":"source\u{0}identity"}),
        &record_trajectory_query_fields()
    ));
    assert!(validate_application_log_query_filter(&predicate(
        "user_input",
        O::Includes,
        json!("\u{0}")
    ))
    .is_err());
}

#[test]
fn new_page_limit_has_no_guessed_business_ceiling() {
    assert_eq!(log_query_page_lookahead(100_001).unwrap(), 100_002);
    assert_eq!(log_query_page_lookahead(1).unwrap(), 2);
    for invalid in [0, -1, i64::MAX] {
        assert!(log_query_page_lookahead(invalid).is_err());
    }
}
#[test]
fn filter_contains_uses_sql_wildcards_but_keyword_search_is_literal() {
    assert!(log_query_text_pattern_matches("aXYZb", "a%b"));
    assert!(log_query_text_pattern_matches("aXb", "a_b"));
    assert!(log_query_text_pattern_matches("a_b", r"a\_b"));
    assert!(!log_query_text_pattern_matches("aXb", r"a\_b"));
    assert!(log_query_text_pattern_matches("literal%", r"literal\"));
    assert!(!log_query_text_pattern_matches("literalx", r"literal\"));
    assert_eq!(log_query_snippet("aXb", "a_b"), None);
    assert_eq!(log_query_snippet("a_b", "a_b"), Some("a_b".into()));
}
