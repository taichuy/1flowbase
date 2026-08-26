#[test]
fn durable_backend_kind_and_crate_name_are_stable() {
    assert_eq!(
        crate::DurableBackendKind::from_env_value("postgres")
            .unwrap()
            .as_str(),
        "postgres"
    );
    assert_eq!(crate::crate_name(), "storage-durable");
}
