use super::*;
use serde_json::{json, Value};

#[test]
fn exact_schema_interning_preserves_registry_fingerprint_bytes_and_order() {
    let a: Value = serde_json::from_str(r#"{"type":"boolean","const":true}"#).unwrap();
    let b: Value = serde_json::from_str(r#"{"const":true,"type":"boolean"}"#).unwrap();
    assert_eq!(a, b);
    let mut pool = BTreeMap::new();
    let first = intern_schema(&mut pool, Arc::new(a.clone()));
    let same = intern_schema(&mut pool, Arc::new(a.clone()));
    let reordered = intern_schema(&mut pool, Arc::new(b.clone()));
    assert!(Arc::ptr_eq(&first, &same));
    assert!(!Arc::ptr_eq(&first, &reordered));
    // This is the production registry schema-identity serialization before/after.
    let identity = |id: &str, schema: &Value| json!({"id":id,"version":"1","rust_type":"test","schema":schema});
    let old = vec![identity("a", &a), identity("b", &a), identity("c", &b)];
    let new = vec![
        identity("a", &first),
        identity("b", &same),
        identity("c", &reordered),
    ];
    assert_eq!(
        serde_json::to_vec(&old).unwrap(),
        serde_json::to_vec(&new).unwrap()
    );
    let weak = Arc::downgrade(&first);
    drop(pool);
    drop(first);
    assert!(weak.upgrade().is_some());
    drop(same);
    assert!(weak.upgrade().is_none());
}
