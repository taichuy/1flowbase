use interface_runtime::GraphFingerprint;

use crate::extension_bus::{production_interface_contributions, InterfaceContributionCollector};

#[tokio::test]
async fn eil_f03_production_modules_publish_additive_registry_contributions() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut collector = InterfaceContributionCollector::new(
        GraphFingerprint::new("eil-f03-production-contributions").unwrap(),
    );
    for contribution in production_interface_contributions(&state).unwrap() {
        collector.add(contribution).unwrap();
    }

    let registry = collector.compile().unwrap();

    assert!(registry.definitions().len() >= 6);
    assert!(registry.bindings().len() >= 6);
}

#[tokio::test]
async fn eil_f03_duplicate_module_contribution_fails_before_registry_publish() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let contribution = production_interface_contributions(&state).unwrap()[0].clone();
    let mut collector = InterfaceContributionCollector::new(
        GraphFingerprint::new("eil-f03-duplicate-contribution").unwrap(),
    );
    collector.add(contribution.clone()).unwrap();

    let error = collector.add(contribution).unwrap_err();

    assert!(error
        .to_string()
        .contains("duplicate interface registry contribution"));
}

#[tokio::test]
async fn eil_f03_conflicting_compiled_snapshot_fails_closed() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let contribution = production_interface_contributions(&state).unwrap()[0].clone();
    let conflicting = contribution
        .clone()
        .with_test_contribution_id("fixture.conflicting-snapshot");
    let mut collector = InterfaceContributionCollector::new(
        GraphFingerprint::new("eil-f03-conflicting-snapshot").unwrap(),
    );
    collector.add(contribution).unwrap();
    collector.add(conflicting).unwrap();

    let error = collector.compile().unwrap_err();

    assert!(
        error.to_string().contains("duplicate") || error.to_string().contains("already"),
        "compiled snapshot conflict must fail closed: {error}"
    );
}
