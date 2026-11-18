use api_server::test_support::crate_split_contract_baseline::{capture, compare, write};

#[tokio::test]
async fn complete_host_contract_matches_frozen_crate_split_baseline() {
    let documents = capture()
        .await
        .expect("capture complete frozen host contract");
    if let Some(directory) = std::env::var_os("CRATE_SPLIT_CONTRACT_OUTPUT") {
        write(std::path::Path::new(&directory), &documents).expect("write full contract export");
    }
    if let Some(directory) = std::env::var_os("CRATE_SPLIT_CONTRACT_GOLDEN") {
        compare(std::path::Path::new(&directory), &documents)
            .expect("compare frozen host contract");
    }
    assert_eq!(documents.len(), 9, "every contract plane must be exported");
}
