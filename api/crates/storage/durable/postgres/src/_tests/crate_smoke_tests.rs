#[test]
fn crate_name_matches_storage_durable_postgres() {
    assert_eq!(
        storage_durable_postgres::crate_name(),
        "storage-durable-postgres"
    );
}
