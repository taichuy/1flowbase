use super::{
    core_interface::NetworkCenterOutput, pools_interface::NetworkPoolsOutput,
    NetworkEgressProjectionResponse, NetworkEgressProviderResponse,
};
use extension_contracts::{ManagedInterfaceView, ManagedProjectionContract};
use interface_runtime::{InterfaceContract, ManagedInterfaceProjection};
use serde_json::{json, Value};

const PRIVATE: &str = "私密-provider-canary";

fn provider(egress_count: usize) -> NetworkEgressProviderResponse {
    NetworkEgressProviderResponse {
        id: "provider-id".into(),
        extension_category: Some(PRIVATE.into()),
        extension_organization: None,
        extension_artifact_id: Some("artifact-id".into()),
        provider_code: "proxy".into(),
        display_name: PRIVATE.into(),
        description: PRIVATE.into(),
        lifecycle: "enabled".into(),
        health_status: PRIVATE.into(),
        secret_configured: true,
        last_sync_error: Some(PRIVATE.into()),
        last_synced_at: None,
        egresses: (0..egress_count)
            .map(|_| NetworkEgressProjectionResponse {
                provider_egress_key: PRIVATE.into(),
                display_name: PRIVATE.into(),
                region: Some(PRIVATE.into()),
                tags: vec![PRIVATE.into(), PRIVATE.into()],
                availability: PRIVATE.into(),
                synced_at: PRIVATE.into(),
            })
            .collect(),
    }
}

fn validated_view<T: InterfaceContract>(output: &T) -> ManagedInterfaceView {
    let projection = ManagedInterfaceProjection::from_contract(output).unwrap();
    let view = ManagedInterfaceView {
        contract: ManagedProjectionContract {
            contract_id: projection.contract().contract_id().into(),
            contract_version: projection.contract().version().into(),
            schema: projection.schema().clone(),
        },
        value: projection.value().clone(),
    };
    view.validate().unwrap();
    view
}

fn variant_schema<'a>(schema: &'a Value, variant: &str) -> &'a Value {
    &schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|schema| schema["properties"]["variant"]["const"] == variant)
        .unwrap()["properties"]["0"]
}

#[test]
fn provider_outputs_share_the_existing_closed_observation_contract() {
    let center = validated_view(&NetworkCenterOutput::Provider(provider(1)));
    let pools = validated_view(&NetworkPoolsOutput::Provider(provider(1)));
    let list = validated_view(&NetworkCenterOutput::Providers(vec![provider(1)]));
    assert_eq!(center.contract.contract_id, "console-network-center-output");
    assert_eq!(pools.contract.contract_id, "console-network-pools-output");
    assert_eq!(center.contract.contract_version, "1");
    assert_eq!(pools.contract.contract_version, "1");

    let bytes = json!({"byte_count": PRIVATE.len()});
    let expected = json!({
        "id": "provider-id", "extension_category": bytes,
        "extension_organization": null, "extension_artifact_id": "artifact-id",
        "provider_code": "proxy", "display_name": bytes, "description": bytes,
        "lifecycle": "enabled", "health_status": bytes,
        "last_sync_error": bytes, "last_synced_at": null,
        "egresses": [{
            "provider_egress_key": bytes, "display_name": bytes, "region": bytes,
            "tags": {"item_count": 2}, "availability": bytes, "synced_at": bytes
        }]
    });
    assert_eq!(center.value, json!({"variant": "Provider", "0": expected}));
    assert_eq!(pools.value, center.value);
    assert_eq!(list.value, json!({"variant": "Providers", "0": [expected]}));
    assert!(!center.value.to_string().contains(PRIVATE));
    assert!(center.value["0"].get("secret_configured").is_none());
    assert_eq!(
        serde_json::to_value(provider(0)).unwrap()["secret_configured"],
        true
    );

    let schema = variant_schema(&center.contract.schema, "Provider");
    assert_eq!(schema, variant_schema(&pools.contract.schema, "Provider"));
    assert_eq!(
        schema,
        &variant_schema(&list.contract.schema, "Providers")["items"]
    );

    let mut absent = provider(1);
    absent.extension_category = None;
    absent.extension_artifact_id = None;
    absent.last_sync_error = None;
    absent.egresses[0].region = None;
    let absent = validated_view(&NetworkCenterOutput::Provider(absent));
    for field in [
        "extension_category",
        "extension_artifact_id",
        "last_sync_error",
    ] {
        assert_eq!(absent.value["0"][field], Value::Null);
    }
    assert_eq!(absent.value["0"]["egresses"][0]["region"], Value::Null);
}

#[test]
fn provider_outputs_keep_observation_bounds_without_truncating_values() {
    for count in [0, 32, 33] {
        let expected = count <= 32;
        assert_eq!(
            NetworkCenterOutput::Provider(provider(count))
                .project_for_managed_hook()
                .is_some(),
            expected
        );
        assert_eq!(
            NetworkPoolsOutput::Provider(provider(count))
                .project_for_managed_hook()
                .is_some(),
            expected
        );
        assert_eq!(
            NetworkCenterOutput::Providers(vec![provider(count)])
                .project_for_managed_hook()
                .is_some(),
            expected
        );
        assert_eq!(
            NetworkCenterOutput::Providers((0..count).map(|_| provider(0)).collect())
                .project_for_managed_hook()
                .is_some(),
            expected
        );
    }
    for text in [
        "x".repeat(256),
        "x".repeat(257),
        "é".repeat(128),
        "é".repeat(129),
    ] {
        let mut value = provider(0);
        value.id = text.clone();
        assert_eq!(
            NetworkCenterOutput::Provider(value)
                .project_for_managed_hook()
                .is_some(),
            text.len() <= 256
        );
    }
    let mut value = provider(0);
    value.description = PRIVATE.repeat(100);
    let view = validated_view(&NetworkPoolsOutput::Provider(value));
    assert_eq!(
        view.value["0"]["description"],
        json!({"byte_count": PRIVATE.len() * 100})
    );
}
