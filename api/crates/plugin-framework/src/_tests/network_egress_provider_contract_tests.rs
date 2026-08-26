use plugin_framework::{
    AcquireHttpForwardProxyInput, CleanupReceipt, EgressAvailability, EgressDescriptor,
    ForwardProxyLease, NetworkEgressProviderStdioRequest, NetworkEgressProviderStdioResponse,
    ReleaseHttpForwardProxyInput, SyncEgressesInput, SyncEgressesResult,
    NETWORK_EGRESS_PROVIDER_CONTRACT,
};
use serde_json::json;

#[test]
fn ac_002_network_egress_stdio_contract_serializes_the_three_public_operations() {
    let sync = NetworkEgressProviderStdioRequest::SyncEgresses(SyncEgressesInput {});
    let acquire =
        NetworkEgressProviderStdioRequest::AcquireHttpForwardProxy(AcquireHttpForwardProxyInput {
            provider_egress_key: "egress-us-west-1".to_owned(),
        });
    let release =
        NetworkEgressProviderStdioRequest::ReleaseHttpForwardProxy(ReleaseHttpForwardProxyInput {
            lease_id: "lease-01".to_owned(),
            cleanup_token: "cleanup-opaque-token".to_owned(),
        });

    assert_eq!(
        NETWORK_EGRESS_PROVIDER_CONTRACT,
        "1flowbase.network_egress_provider/v1"
    );
    assert_eq!(
        NETWORK_EGRESS_PROVIDER_CONTRACT,
        extension_contracts::NETWORK_EGRESS_PROVIDER_CONTRACT
    );
    assert_eq!(
        serde_json::to_value(sync).unwrap(),
        json!({"operation": "sync_egresses", "input": {}})
    );
    assert_eq!(
        serde_json::to_value(acquire).unwrap(),
        json!({
            "operation": "acquire_http_forward_proxy",
            "input": {"provider_egress_key": "egress-us-west-1"}
        })
    );
    assert_eq!(
        serde_json::to_value(release).unwrap(),
        json!({
            "operation": "release_http_forward_proxy",
            "input": {"lease_id": "lease-01", "cleanup_token": "cleanup-opaque-token"}
        })
    );
}

#[test]
fn ac_003_ac_005_third_party_responses_are_typed_and_egress_descriptors_are_stable() {
    let response: NetworkEgressProviderStdioResponse = serde_json::from_value(json!({
        "operation": "sync_egresses",
        "result": {
            "egresses": [
                {
                    "provider_egress_key": "egress-eu-1",
                    "display_name": "Europe 1",
                    "region": "eu-west",
                    "tags": ["eu", "shared"],
                    "availability": "available"
                },
                {
                    "provider_egress_key": "egress-us-1",
                    "display_name": "US 1",
                    "availability": "unavailable"
                }
            ]
        }
    }))
    .unwrap();
    response.validate().unwrap();

    let expected = NetworkEgressProviderStdioResponse::SyncEgresses(SyncEgressesResult {
        egresses: vec![
            EgressDescriptor {
                provider_egress_key: "egress-eu-1".to_owned(),
                display_name: "Europe 1".to_owned(),
                region: Some("eu-west".to_owned()),
                tags: Some(vec!["eu".to_owned(), "shared".to_owned()]),
                availability: EgressAvailability::Available,
            },
            EgressDescriptor {
                provider_egress_key: "egress-us-1".to_owned(),
                display_name: "US 1".to_owned(),
                region: None,
                tags: None,
                availability: EgressAvailability::Unavailable,
            },
        ],
    });
    assert_eq!(response, expected);
    let encoded = serde_json::to_value(&response).unwrap();
    assert_eq!(
        serde_json::from_value::<NetworkEgressProviderStdioResponse>(encoded).unwrap(),
        response
    );

    let unsorted: NetworkEgressProviderStdioResponse = serde_json::from_value(json!({
        "operation": "sync_egresses",
        "result": {
            "egresses": [
                {
                    "provider_egress_key": "egress-us-1",
                    "display_name": "US 1",
                    "availability": "available"
                },
                {
                    "provider_egress_key": "egress-eu-1",
                    "display_name": "Europe 1",
                    "availability": "available"
                }
            ]
        }
    }))
    .unwrap();
    assert!(unsorted
        .validate()
        .unwrap_err()
        .to_string()
        .contains("sorted"));
}

#[test]
fn ac_004_ac_014_proxy_lease_and_cleanup_receipt_are_secret_and_config_free() {
    let acquire_response: NetworkEgressProviderStdioResponse = serde_json::from_value(json!({
        "operation": "acquire_http_forward_proxy",
        "result": {
            "lease_id": "lease-01",
            "http_proxy_url": "http://127.0.0.1:18080",
            "cleanup_token": "cleanup-opaque-token",
            "expires_at": 1_777_777_777_000i64
        }
    }))
    .unwrap();
    acquire_response.validate().unwrap();
    assert_eq!(
        acquire_response,
        NetworkEgressProviderStdioResponse::AcquireHttpForwardProxy(ForwardProxyLease {
            lease_id: "lease-01".to_owned(),
            http_proxy_url: "http://127.0.0.1:18080".to_owned(),
            cleanup_token: "cleanup-opaque-token".to_owned(),
            expires_at: 1_777_777_777_000,
        })
    );

    let release_response: NetworkEgressProviderStdioResponse = serde_json::from_value(json!({
        "operation": "release_http_forward_proxy",
        "result": {"lease_id": "lease-01"}
    }))
    .unwrap();
    release_response.validate().unwrap();
    assert_eq!(
        release_response,
        NetworkEgressProviderStdioResponse::ReleaseHttpForwardProxy(CleanupReceipt {
            lease_id: "lease-01".to_owned(),
        })
    );

    let secret_or_config = serde_json::from_value::<NetworkEgressProviderStdioRequest>(json!({
        "operation": "acquire_http_forward_proxy",
        "input": {
            "provider_egress_key": "egress-us-1",
            "provider_config": {"token": "must-not-cross-stdio"}
        }
    }))
    .unwrap_err();
    assert!(secret_or_config.to_string().contains("provider_config"));

    let encoded = serde_json::to_value(&acquire_response).unwrap();
    assert_eq!(
        encoded,
        json!({
            "operation": "acquire_http_forward_proxy",
            "result": {
                "lease_id": "lease-01",
                "http_proxy_url": "http://127.0.0.1:18080",
                "cleanup_token": "cleanup-opaque-token",
                "expires_at": 1_777_777_777_000i64
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<NetworkEgressProviderStdioResponse>(encoded).unwrap(),
        acquire_response
    );
}

#[test]
fn ac_016_stdio_contract_rejects_unknown_operations_and_missing_or_invalid_fields() {
    let unknown_operation = serde_json::from_value::<NetworkEgressProviderStdioRequest>(json!({
        "operation": "health",
        "input": {}
    }))
    .unwrap_err();
    assert!(unknown_operation.to_string().contains("health"));

    let missing_cleanup_token =
        serde_json::from_value::<NetworkEgressProviderStdioRequest>(json!({
            "operation": "release_http_forward_proxy",
            "input": {"lease_id": "lease-01"}
        }))
        .unwrap_err();
    assert!(missing_cleanup_token.to_string().contains("cleanup_token"));

    let invalid_proxy =
        NetworkEgressProviderStdioResponse::AcquireHttpForwardProxy(ForwardProxyLease {
            lease_id: "lease-01".to_owned(),
            http_proxy_url: "https://127.0.0.1:18080".to_owned(),
            cleanup_token: "cleanup-opaque-token".to_owned(),
            expires_at: 0,
        });
    assert!(invalid_proxy
        .validate()
        .unwrap_err()
        .to_string()
        .contains("http_proxy_url"));

    let missing_expiry =
        NetworkEgressProviderStdioResponse::AcquireHttpForwardProxy(ForwardProxyLease {
            lease_id: "lease-01".to_owned(),
            http_proxy_url: "http://127.0.0.1:18080".to_owned(),
            cleanup_token: "cleanup-opaque-token".to_owned(),
            expires_at: 0,
        });
    assert!(missing_expiry
        .validate()
        .unwrap_err()
        .to_string()
        .contains("expires_at"));

    let obsolete_host_port = serde_json::from_value::<NetworkEgressProviderStdioResponse>(json!({
        "operation": "acquire_http_forward_proxy",
        "result": {
            "lease_id": "lease-01",
            "http_proxy_host": "127.0.0.1",
            "http_proxy_port": 18080,
            "cleanup_token": "cleanup-opaque-token",
            "expires_at": 1_777_777_777_000i64
        }
    }))
    .unwrap_err();
    assert!(obsolete_host_port.to_string().contains("http_proxy_host"));

    let missing_availability =
        serde_json::from_value::<NetworkEgressProviderStdioResponse>(json!({
            "operation": "sync_egresses",
            "result": {
                "egresses": [{
                    "provider_egress_key": "egress-us-1",
                    "display_name": "US 1"
                }]
            }
        }))
        .unwrap_err();
    assert!(missing_availability.to_string().contains("availability"));
}
