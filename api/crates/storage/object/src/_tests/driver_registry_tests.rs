use storage_object::{builtin_driver_registry, FileStorageError};

#[test]
fn builtin_registry_lists_local_and_rustfs() {
    let registry = builtin_driver_registry();
    assert_eq!(
        registry.driver_types(),
        vec!["local".to_string(), "rustfs".to_string()]
    );
}

#[test]
fn builtin_registry_returns_driver_by_type() {
    let registry = builtin_driver_registry();
    assert_eq!(registry.get("local").unwrap().driver_type(), "local");
    assert_eq!(registry.get("rustfs").unwrap().driver_type(), "rustfs");
    assert!(registry.get("oss").is_none());
}

#[test]
fn unsupported_driver_error_formats_cleanly() {
    let error = FileStorageError::unsupported_driver("oss");
    assert_eq!(error.to_string(), "unsupported file storage driver: oss");
}
