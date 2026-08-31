use interface_runtime::GraphFingerprint;

use crate::extension_bus::{production_interface_contributions, InterfaceContributionCollector};

#[test]
fn eil_f03_production_modules_publish_additive_registry_contributions() {
    let mut collector = InterfaceContributionCollector::new(
        GraphFingerprint::new("eil-f03-production-contributions").unwrap(),
    );
    for contribution in production_interface_contributions() {
        collector.add(contribution).unwrap();
    }

    let registry = collector.compile(std::sync::Weak::new()).unwrap();

    assert!(registry.definitions().len() >= 6);
    assert!(registry.bindings().len() >= 6);
}

#[test]
fn eil_f03_duplicate_module_contribution_fails_before_registry_publish() {
    let contribution = production_interface_contributions()[0];
    let mut collector = InterfaceContributionCollector::new(
        GraphFingerprint::new("eil-f03-duplicate-contribution").unwrap(),
    );
    collector.add(contribution).unwrap();

    let error = collector.add(contribution).unwrap_err();

    assert!(error
        .to_string()
        .contains("duplicate interface registry contribution"));
}
