use super::*;

// #2027 AC-004/005: overlapping matches are ambiguous, even when str::match_indices skips them.
#[test]
fn exact_replacement_rejects_overlapping_occurrences_and_empty_anchor() {
    for (old, code) in [("aa", "source_text_ambiguous"), ("", "source_text_empty")] {
        let error = text_replacements(
            "aaa",
            vec![FrontstageSourceTextEdit {
                old_text: old.into(),
                new_text: "b".into(),
            }],
        )
        .err()
        .unwrap();
        assert_eq!(
            error
                .downcast_ref::<FrontstageSourceEditError>()
                .unwrap()
                .code,
            code
        );
    }
}

#[test]
fn edits_preserve_unicode_crlf_and_untouched_bytes() {
    let source = "a\r\n中😀\r\nz";
    let ranges = text_replacements(
        source,
        vec![
            FrontstageSourceTextEdit {
                old_text: "a".into(),
                new_text: "a\r\ninsert".into(),
            },
            FrontstageSourceTextEdit {
                old_text: "😀".into(),
                new_text: "✅".into(),
            },
            FrontstageSourceTextEdit {
                old_text: "z".into(),
                new_text: "".into(),
            },
        ],
    )
    .unwrap();
    let (text, changes) = apply_replacements(source, ranges).unwrap();
    assert_eq!(text, "a\r\ninsert\r\n中✅\r\n");
    assert_eq!(changes[1].start_column, 2);
    assert_eq!(changes[1].end_column, 3);
}

#[test]
fn receipt_text_has_one_shared_unicode_budget() {
    let source = "a".repeat(20_000);
    let (text, changes) = apply_replacements(
        &source,
        vec![SourceReplacement {
            index: 0,
            start: 0,
            end: source.len(),
            replacement: "😀".repeat(20_000),
        }],
    )
    .unwrap();
    assert_eq!(text, "😀".repeat(20_000));
    assert_eq!(
        changes
            .iter()
            .map(|c| c.old_text.chars().count() + c.new_text.chars().count())
            .sum::<usize>(),
        MAX_DIFF_CHARS
    );
    assert!(changes[0].truncated);
}

#[test]
fn batch_positions_refer_to_original_source_and_duplicate_insertions_fail() {
    let (text, _) = apply_replacements(
        "abc",
        vec![
            SourceReplacement {
                index: 0,
                start: 0,
                end: 0,
                replacement: "prefix".into(),
            },
            SourceReplacement {
                index: 1,
                start: 2,
                end: 3,
                replacement: "tail".into(),
            },
        ],
    )
    .unwrap();
    assert_eq!(text, "prefixabtail");
    assert!(apply_replacements(
        "abc",
        vec![
            SourceReplacement {
                index: 0,
                start: 0,
                end: 0,
                replacement: "x".into()
            },
            SourceReplacement {
                index: 1,
                start: 0,
                end: 0,
                replacement: "y".into()
            }
        ]
    )
    .is_err());
}

fn code_record(source: &str) -> domain::frontstage::FrontstageBlockCodeRecord {
    domain::frontstage::FrontstageBlockCodeRecord {
        workspace_id: Uuid::nil(),
        page_id: Uuid::nil(),
        code_ref: "source".into(),
        source_code: source.into(),
        source_sha256: Some("a".repeat(64)),
        created_at: time::OffsetDateTime::UNIX_EPOCH,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn search_pages_overlapping_hits_with_bounded_context_and_original_coordinates() {
    let source = format!("{}\r\n中😀aaa\n", "x".repeat(20000));
    let first = search_source("block".into(), code_record(&source), "aa", 1, 1, 1).unwrap();
    assert_eq!(first.matches.len(), 1);
    assert_eq!(
        (first.matches[0].start_line, first.matches[0].start_column),
        (2, 3)
    );
    assert!(first.matches[0].context.chars().count() <= 320);
    assert!(first.matches[0].context_truncated);
    assert_eq!((first.next_line, first.next_column), (Some(2), Some(4)));
    let second = search_source(
        "block".into(),
        code_record(&source),
        "aa",
        first.next_line.unwrap(),
        first.next_column.unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(second.matches[0].start_column, 4);
    assert!(!second.truncated);
    assert!(search_source("block".into(), code_record(""), "x", 1, 1, 1)
        .unwrap()
        .matches
        .is_empty());
    assert!(search_source("block".into(), code_record("abc"), "", 1, 1, 1).is_err());
    assert!(search_source("block".into(), code_record("abc"), "a", 1, 1, 101).is_err());
}
