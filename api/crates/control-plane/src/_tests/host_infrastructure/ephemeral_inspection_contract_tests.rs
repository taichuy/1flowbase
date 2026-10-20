use serde_json::json;

use crate::ports::{
    paginate_ephemeral_entries, EphemeralEntrySnapshot, EphemeralInspectionPageRequest,
};

fn entry(entry_ref: &str, inspection_path: &[&str]) -> EphemeralEntrySnapshot {
    EphemeralEntrySnapshot {
        contract_code: "cache-store".to_string(),
        group_code: inspection_path.first().map(|value| value.to_string()),
        entry_ref: entry_ref.to_string(),
        key: entry_ref.to_string(),
        inspection_path: inspection_path
            .iter()
            .map(|segment| segment.to_string())
            .collect(),
        entry_kind: "cache_entry".to_string(),
        status: "active".to_string(),
        owner: None,
        value_size_bytes: 1,
        metadata_size_bytes: 2,
        ttl_seconds: None,
        created_at_unix: None,
        expires_at_unix: None,
        sensitive: false,
        metadata: json!({}),
    }
}

#[test]
fn ephemeral_entry_cursor_follows_path_then_entry_ref_sort_order() {
    let entries = vec![entry("z-entry", &["a"]), entry("a-entry", &["b"])];
    let first_page = paginate_ephemeral_entries(
        entries.clone(),
        EphemeralInspectionPageRequest::new(Vec::new(), None, Some(1), None),
    );

    assert_eq!(first_page.entries[0].entry_ref, "z-entry");
    let second_page = paginate_ephemeral_entries(
        entries,
        EphemeralInspectionPageRequest::new(Vec::new(), first_page.next_cursor, Some(1), None),
    );

    assert_eq!(second_page.entries[0].entry_ref, "a-entry");
    assert!(second_page.next_cursor.is_none());
}
