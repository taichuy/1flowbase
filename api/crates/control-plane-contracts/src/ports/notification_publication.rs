//! Explicit optional integration notifications. Monetary/model effects precede this boundary.
use super::*;
use extension_contracts::LifecycleContract;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CreditNotificationEventType {
    CreditReserved,
    CreditSettled,
    CreditReleased,
    CreditGranted,
    CreditCharged,
    CreditRefunded,
    CreditAdjusted,
    CreditChargeSettingChanged,
    CreditCommandRejected,
}

impl CreditNotificationEventType {
    pub fn from_event_type(value: &str) -> anyhow::Result<Self> {
        serde_json::from_value(serde_json::Value::String(value.to_owned()))
            .map_err(anyhow::Error::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditNotificationFact {
    pub workspace_id: Uuid,
    pub account_id: Option<Uuid>,
    pub event_type: CreditNotificationEventType,
    pub payload: serde_json::Value,
}
impl LifecycleContract for CreditNotificationFact {
    const CONTRACT_ID: &'static str = "credit.integration_notification";
    const CONTRACT_VERSION: &'static str = "v1";
}

/// A forwarding stage with its own completion boundary, distinct from model commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDefinitionIntegrationNotification {
    pub model_definition_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
}
impl LifecycleContract for ModelDefinitionIntegrationNotification {
    const CONTRACT_ID: &'static str = "model_definition.integration_notification";
    const CONTRACT_VERSION: &'static str = "v1";
}

/// Only these named, versioned contracts permit publication without a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalNotificationContract {
    Credit,
    ModelDefinition,
}
impl OptionalNotificationContract {
    pub fn contract_id(self) -> &'static str {
        match self {
            Self::Credit => CreditNotificationFact::CONTRACT_ID,
            Self::ModelDefinition => ModelDefinitionIntegrationNotification::CONTRACT_ID,
        }
    }
    pub fn contract_version(self) -> &'static str {
        match self {
            Self::Credit => CreditNotificationFact::CONTRACT_VERSION,
            Self::ModelDefinition => ModelDefinitionIntegrationNotification::CONTRACT_VERSION,
        }
    }
}

pub fn validate_optional_notification_contract(
    contract_id: &str,
    contract_version: &str,
) -> anyhow::Result<OptionalNotificationContract> {
    [OptionalNotificationContract::Credit, OptionalNotificationContract::ModelDefinition]
        .into_iter()
        .find(|contract| contract.contract_id() == contract_id && contract.contract_version() == contract_version)
        .ok_or_else(|| anyhow::anyhow!("contract does not permit optional notification publication: {contract_id}@{contract_version}"))
}

#[derive(Debug, Clone)]
pub struct PublishOptionalNotificationInput {
    pub source_event_id: Uuid,
    pub transaction_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub occurred_at: OffsetDateTime,
    pub contract: OptionalNotificationContract,
    /// One canonical AfterCommitFact of the typed contract, retained by the lifecycle owner.
    pub canonical_payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalNotificationPublicationOutcome {
    NoTargets,
    /// Exact original v1 forwarding obligation, now retained for explicit recovery.
    LegacyRecoveryHandoff {
        sequence: i64,
    },
    /// Durable stage handoff only; target business effects remain independently pending.
    Enqueued {
        event_id: Uuid,
        target_count: usize,
    },
}

#[async_trait]
pub trait OptionalNotificationPublicationPort: Send + Sync {
    /// NoTargets is legal only after resolving the actual effective publication plan.
    /// An unavailable declared target or publication source must propagate an error.
    async fn publish_optional_notification(
        &self,
        input: &PublishOptionalNotificationInput,
    ) -> anyhow::Result<OptionalNotificationPublicationOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_zero_is_scoped_to_named_contracts_and_versions() {
        assert_eq!(
            validate_optional_notification_contract("credit.integration_notification", "v1")
                .unwrap(),
            OptionalNotificationContract::Credit,
        );
        assert_eq!(
            validate_optional_notification_contract(
                "model_definition.integration_notification",
                "v1"
            )
            .unwrap(),
            OptionalNotificationContract::ModelDefinition,
        );
        for (id, version) in [
            ("credit.integration_notification", "v2"),
            ("model_definition.integration_notification", "v2"),
            ("model_definition.committed", "v1"),
            ("unknown.notification", "v1"),
        ] {
            assert!(validate_optional_notification_contract(id, version).is_err());
        }
    }

    #[test]
    fn credit_notification_accepts_the_nine_source_kinds_without_changing_wire_names() {
        for name in [
            "CreditReserved",
            "CreditSettled",
            "CreditReleased",
            "CreditGranted",
            "CreditCharged",
            "CreditRefunded",
            "CreditAdjusted",
            "CreditChargeSettingChanged",
            "CreditCommandRejected",
        ] {
            let kind = CreditNotificationEventType::from_event_type(name).unwrap();
            assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(name));
        }
        assert!(CreditNotificationEventType::from_event_type("CreditUnknown").is_err());
        assert!(CreditNotificationEventType::from_event_type("credit_settled").is_err());
    }
}
