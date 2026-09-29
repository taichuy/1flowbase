use super::*;

#[test]
fn native_spans_preserve_whitespace_numbers_escapes_and_distinct_occurrences() {
    let body = " \n{\"messages\": [ {\"text\":\"a\\u0000b\",\"n\":9007199254740993}, {\"text\":\"a\\\\u0000b\"} ],\"native_request\":{\"wire_body\":{\"input\":[{\"x\":null},{\"x\":null}],\"tools\":[{\"schema\":{\"nul\\u0000key\":1}}]}},\"tools\":[]} \n";
    let ranges = spans(body).unwrap();
    assert_eq!(ranges.len(), 5);
    assert_eq!(ranges[2].1, ranges[3].1);
    assert_ne!(ranges[0].1, ranges[1].1);
    let mut restored = String::new();
    let mut end = 0;
    for (start, text) in ranges {
        restored.push_str(&body[end..start]);
        restored.push_str(text);
        end = start + text.len();
    }
    restored.push_str(&body[end..]);
    assert_eq!(restored, body);
    assert!(spans("{\"messages\":[]}").is_none());
    assert!(spans("invalid").is_none());
    assert!(spans("{\"input\":[1],\"s\":\"\0\"}").is_none());
}
