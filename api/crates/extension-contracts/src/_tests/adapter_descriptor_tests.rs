use serde_json::json;

use crate::{EgressAvailability, EgressDescriptor, TrustedPublicKey};

#[test]
fn root_1894_adapter_descriptors_are_canonical_and_wire_stable() {
    let descriptor: EgressDescriptor = serde_json::from_value(json!({
        "provider_egress_key": "egress-eu-1",
        "display_name": "Europe 1",
        "region": "eu-west",
        "tags": ["eu", "shared"],
        "availability": "available"
    }))
    .unwrap();

    assert_eq!(descriptor.availability, EgressAvailability::Available);
    assert_eq!(
        serde_json::to_value(&descriptor).unwrap(),
        json!({
            "provider_egress_key": "egress-eu-1",
            "display_name": "Europe 1",
            "region": "eu-west",
            "tags": ["eu", "shared"],
            "availability": "available"
        })
    );

    let trusted_key = TrustedPublicKey {
        key_id: "release-key".to_string(),
        algorithm: "ed25519".to_string(),
        public_key_pem: "public-key-pem".to_string(),
    };
    assert_eq!(trusted_key.key_id, "release-key");
    assert_eq!(trusted_key.algorithm, "ed25519");
    assert_eq!(trusted_key.public_key_pem, "public-key-pem");
}
