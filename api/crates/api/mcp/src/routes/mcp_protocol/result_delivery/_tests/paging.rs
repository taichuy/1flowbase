use super::*;

#[test]
fn selected_string_pages_account_for_metadata_and_keep_absolute_offsets() {
    let source = "甲😀乙丙丁".repeat(3000);
    let selection = ResultSelection::parse(&json!({"response_fields": ["/body"], "string_ranges": {"/body": {"offset": 7, "length": 1000}}}), None).unwrap();
    let leaves = selection.leaves(&json!({"body": source})).unwrap();
    let mut cursor = ContinuationCursor {
        selection_id: selection.identity(),
        ..Default::default()
    };
    let mut reconstructed = String::new();
    loop {
        let page = bounded_page(
            &leaves,
            cursor,
            700,
            json!({"result_ref": "test", "detail_status": "available", "retry_original": false}),
        );
        assert!(serialized(&page).chars().count() <= 700);
        for entry in page["entries"].as_array().unwrap() {
            assert_eq!(entry["char_offset"], 7 + reconstructed.chars().count());
            assert_eq!(entry["total_chars"], 15000);
            reconstructed.push_str(entry["value"].as_str().unwrap());
            assert_eq!(entry["next_offset"], 7 + reconstructed.chars().count());
            assert_eq!(entry["complete"], false);
        }
        let Some(encoded) = page["next_cursor"].as_str() else {
            break;
        };
        cursor = ContinuationCursor::parse(encoded).unwrap();
        assert_eq!(cursor.selection_id, selection.identity());
    }
    assert_eq!(
        reconstructed,
        source.chars().skip(7).take(1000).collect::<String>()
    );
}

#[test]
fn initial_selection_page_reserves_both_cursor_locations() {
    let selection = ResultSelection::parse(&json!({"response_fields": ["/body"]}), None).unwrap();
    let leaves = selection
        .leaves(&json!({"body": "界".repeat(3000)}))
        .unwrap();
    let cursor = ContinuationCursor {
        selection_id: selection.identity(),
        ..Default::default()
    };
    let page = bounded_page(
        &leaves,
        cursor,
        900,
        json!({"outcome": "succeeded", "detail": {"result_ref": "test", "next_cursor": null}, "retry_original": false}),
    );
    assert!(serialized(&page).chars().count() <= 900);
    assert!(!page["entries"].as_array().unwrap().is_empty());
    assert_eq!(page["next_cursor"], page["detail"]["next_cursor"]);
}

#[test]
fn page_budget_too_small_keeps_the_same_retry_cursor_in_both_locations() {
    let leaves = ResultSelection::default()
        .leaves(&json!({"body": "界".repeat(3000)}))
        .unwrap();
    let cursor = ContinuationCursor::default();
    let page = bounded_page(
        &leaves,
        cursor,
        1,
        json!({"detail": {"result_ref": "test", "next_cursor": null}}),
    );
    assert_eq!(page["detail_status"], "page_budget_too_small");
    assert_eq!(page["next_cursor"], json!(cursor.encode()));
    assert_eq!(page["detail"]["next_cursor"], page["next_cursor"]);
    assert!(
        ContinuationCursor::parse(page["detail"]["next_cursor"].as_str().unwrap())
            .unwrap()
            .is_valid_for(&leaves)
    );
}

#[test]
fn empty_string_range_at_the_end_is_not_a_missing_or_invalid_field() {
    let selection = ResultSelection::parse(
        &json!({"string_ranges": {"/body": {"offset": 2, "length": 5}}}),
        None,
    )
    .unwrap();
    let leaves = selection.leaves(&json!({"body": "甲😀"})).unwrap();
    let cursor = ContinuationCursor {
        selection_id: selection.identity(),
        ..Default::default()
    };
    assert!(cursor.is_valid_for(&leaves));
    let page = bounded_page(&leaves, cursor, 1000, json!({"detail_status": "available"}));
    assert_eq!(page["entries"][0]["char_count"], 0);
    assert_eq!(page["entries"][0]["char_offset"], 2);
    assert_eq!(page["entries"][0]["complete"], true);
    assert_eq!(page["entries"][0]["next_offset"], Value::Null);
}

#[test]
fn budgets_above_the_previous_ceiling_are_valid_but_unlimited_values_are_not() {
    assert_eq!(inline_limit(&json!({"max_inline_chars": 80000})), Ok(80000));
    for value in [
        json!(0),
        json!(-1),
        Value::Null,
        json!("unlimited"),
        json!(1.5),
    ] {
        assert!(inline_limit(&json!({"max_inline_chars": value})).is_err());
    }
}
