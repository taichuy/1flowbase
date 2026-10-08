//! Client-executed collector distributions retained by the platform, never runtime slots.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientCollectorDistributionKind {
    ClientCollector,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientCollectorDescriptor {
    pub collector_code: String,
    pub source_client: String,
    pub display_name: String,
    pub execution_target: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCollectorAsset {
    pub name: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCollectorManifest {
    pub schema_version: String,
    pub distribution_kind: ClientCollectorDistributionKind,
    pub organization: String,
    pub artifact_id: String,
    pub collector_code: String,
    pub source_client: String,
    pub display_name: String,
    pub version: String,
    pub execution_target: String,
    pub protocol_version: String,
    pub entry: String,
    pub minimum_host_version: String,
    pub description: BTreeMap<String, String>,
    pub assets: Vec<ClientCollectorAsset>,
}
impl ClientCollectorManifest {
    pub fn descriptor(&self) -> ClientCollectorDescriptor {
        ClientCollectorDescriptor {
            collector_code: self.collector_code.clone(),
            source_client: self.source_client.clone(),
            display_name: self.display_name.clone(),
            execution_target: self.execution_target.clone(),
            protocol_version: self.protocol_version.clone(),
        }
    }
}
pub fn client_collector_flat_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
pub fn is_client_collector_receipt(receipt: &serde_json::Value) -> bool {
    let kind = receipt
        .get("distribution_kind")
        .cloned()
        .and_then(|value| serde_json::from_value::<ClientCollectorDistributionKind>(value).ok());
    let manifest = receipt
        .get("client_collector")
        .cloned()
        .and_then(|value| serde_json::from_value::<ClientCollectorManifest>(value).ok());
    kind.is_some()
        && manifest.is_some_and(|manifest| {
            manifest.schema_version == "1flowbase.client-collector/v1"
                && manifest.execution_target == "client"
                && manifest.protocol_version == "1flowbase.agent-logs/v1"
        })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCollectorReleaseArtifact {
    pub os: String,
    pub arch: String,
    pub rust_target: String,
    pub name: String,
    pub sha256: String,
    pub size: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCollectorReleaseManifest {
    pub schema_version: String,
    pub collector_code: String,
    pub version: String,
    pub source_sha: String,
    pub signature_algorithm: String,
    pub signing_key_id: String,
    pub artifacts: Vec<ClientCollectorReleaseArtifact>,
}
