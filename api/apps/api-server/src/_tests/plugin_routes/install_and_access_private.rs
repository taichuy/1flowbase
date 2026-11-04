use crate::routes::plugins::runtime_slot_for_contract;
use plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;

#[test]
fn ac_002_plugin_route_classifies_only_current_provider_contract() {
    assert_eq!(
        runtime_slot_for_contract(CURRENT_PROVIDER_CONTRACT).as_deref(),
        Some("model_provider")
    );
    assert_eq!(runtime_slot_for_contract("1flowbase.provider/v1"), None);
    assert_eq!(
        runtime_slot_for_contract("1flowbase.data_source/v1").as_deref(),
        Some("data_source")
    );
}
