use interface_runtime::{BindingId, GraphFingerprint, InterfaceProtocol};

use crate::{
    extension_bus::{production_interface_contributions, InterfaceContributionCollector},
    routes::application_public_api::native_read_interface::{
        CANCEL_RUN_BINDING_ID, GET_RUN_BINDING_ID, MODELS_BINDING_ID, RESUME_RUN_BINDING_ID,
    },
};

#[tokio::test]
async fn eil_f05a_native_reads_publish_unique_frozen_plans() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut collector =
        InterfaceContributionCollector::new(GraphFingerprint::new("eil-f05a-native-read").unwrap());
    for contribution in production_interface_contributions(&state).unwrap() {
        collector.add(contribution).unwrap();
    }
    let registry = collector.compile().unwrap();

    for binding in [
        MODELS_BINDING_ID,
        GET_RUN_BINDING_ID,
        CANCEL_RUN_BINDING_ID,
        RESUME_RUN_BINDING_ID,
    ] {
        let plan = registry.plan(&BindingId::new(binding).unwrap()).unwrap();
        assert_eq!(
            plan.binding().projection().protocol(),
            InterfaceProtocol::Http
        );
        assert_eq!(
            plan.authentication().adapter().as_str(),
            "api-server.application-api-key"
        );
        assert_eq!(
            plan.effective_handler().handler(),
            plan.definition().handler_reference()
        );
    }
}
