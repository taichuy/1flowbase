use crate::{DEFAULT_SCOPE_ID, SYSTEM_SCOPE_ID};

#[test]
fn default_scope_id_is_stable_and_distinct_from_system_scope() {
    assert_eq!(
        DEFAULT_SCOPE_ID.to_string(),
        "00000000-0000-0000-0000-000000000001"
    );
    assert_ne!(DEFAULT_SCOPE_ID, SYSTEM_SCOPE_ID);
}
