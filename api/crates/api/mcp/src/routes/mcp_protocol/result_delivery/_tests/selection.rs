use super::*;

#[test]
fn fields_select_nested_arrays_and_escaped_names_without_prefix_leaks() {
    let detail =
        json!({"items": [{"body": "hello", "secret": true}], "itemship": 7, "a/b": {"~": 9}});
    let selection = ResultSelection::parse(
        &json!({"response_fields": ["/items/0/body", "/a~1b/~0"]}),
        None,
    )
    .unwrap();
    let leaves = selection.leaves(&detail).unwrap();
    assert_eq!(
        leaves
            .iter()
            .map(|leaf| leaf.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/a~1b/~0", "/items/0/body"]
    );
    let parent = ResultSelection::parse(&json!({"response_fields": ["/items"]}), None).unwrap();
    assert_eq!(parent.leaves(&detail).unwrap().len(), 2);
}

#[test]
fn explicit_empty_fields_override_defaults_without_returning_business_data() {
    let defaults = vec!["/title".to_owned()];
    let selection =
        ResultSelection::parse(&json!({"response_fields": []}), Some(&defaults)).unwrap();
    assert!(selection
        .leaves(&json!({"title": "secret"}))
        .unwrap()
        .is_empty());
    assert_eq!(
        ResultSelection::parse(&json!({}), Some(&defaults))
            .unwrap()
            .response_fields,
        Some(defaults)
    );
}

#[test]
fn ranges_use_unicode_offsets_and_keep_original_length() {
    let selection = ResultSelection::parse(&json!({"response_fields": ["/body"], "string_ranges": {"/body": {"offset": 1, "length": 2}}}), None).unwrap();
    let leaves = selection
        .leaves(&json!({"body": "甲😀乙丙丁", "other": 1}))
        .unwrap();
    assert_eq!(leaves[0].value, json!("😀乙"));
    assert_eq!(leaves[0].base_offset, 1);
    assert_eq!(leaves[0].total_chars, Some(5));
}

#[test]
fn invalid_selectors_are_rejected_instead_of_silently_falling_back() {
    for arguments in [
        json!({"response_fields": null}),
        json!({"response_fields": ["body"]}),
        json!({"response_fields": ["/bad~2"]}),
        json!({"string_ranges": {"/body": {"length": 0}}}),
        json!({"string_ranges": {"/body": {"offset": -1, "length": 1}}}),
        json!({"string_ranges": {"/body": {"length": 1, "extra": true}}}),
        json!({"response_fields": [], "string_ranges": {"/body": {"length": 1}}}),
    ] {
        assert!(
            ResultSelection::parse(&arguments, None).is_err(),
            "{arguments}"
        );
    }
    let selection =
        ResultSelection::parse(&json!({"response_fields": ["/missing"]}), None).unwrap();
    assert!(selection.leaves(&json!({"body": "hello"})).is_err());
}

#[test]
fn selection_identity_is_stable_and_changes_with_range_or_fields() {
    let first = ResultSelection::parse(
        &json!({"response_fields": ["/body"], "string_ranges": {"/body": {"length": 2}}}),
        None,
    )
    .unwrap();
    let second = ResultSelection::parse(
        &json!({"response_fields": ["/body"], "string_ranges": {"/body": {"length": 3}}}),
        None,
    )
    .unwrap();
    assert_eq!(first.identity(), first.clone().identity());
    assert_ne!(first.identity(), second.identity());
    assert_eq!(ResultSelection::default().identity(), None);
}
