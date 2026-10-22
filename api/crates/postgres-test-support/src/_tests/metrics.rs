use std::{path::Path, time::Duration};

use crate::{metrics::write_phase, PostgresTestPhase};

#[test]
fn phase_records_are_process_owned_and_contain_no_connection_details() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .join("tmp/test-governance/postgres-test-support")
        .join(uuid::Uuid::now_v7().simple().to_string());
    write_phase(
        &directory,
        PostgresTestPhase::Clone,
        Duration::from_nanos(73),
        true,
    )
    .unwrap();
    write_phase(
        &directory,
        PostgresTestPhase::Migration,
        Duration::from_nanos(91),
        false,
    )
    .unwrap();
    let files: Vec<_> = std::fs::read_dir(&directory).unwrap().collect();
    assert_eq!(files.len(), 1);
    let path = directory.join(format!("postgres-test-{}.jsonl", std::process::id()));
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content.lines().count(), 2);
    assert!(content.contains("\"phase\":\"clone\",\"elapsed_ns\":73,\"succeeded\":true"));
    assert!(content.contains("\"phase\":\"migration\",\"elapsed_ns\":91,\"succeeded\":false"));
    assert!(!content.contains("postgres://"));
    assert!(!content.contains("password"));
    std::fs::remove_dir_all(directory).unwrap();
}
