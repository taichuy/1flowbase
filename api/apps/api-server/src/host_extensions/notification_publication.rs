use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    OptionalNotificationPublicationOutcome, OptionalNotificationPublicationPort,
    PublishOptionalNotificationInput,
};

/// Factories bind before the final publication catalog exists. Activation installs the
/// actual final store once, so no handler retains a pre-catalog store clone.
#[derive(Default)]
pub(crate) struct HostOptionalNotificationPublication {
    port: OnceLock<Arc<dyn OptionalNotificationPublicationPort>>,
}
impl HostOptionalNotificationPublication {
    pub(crate) fn activate(
        &self,
        port: Arc<dyn OptionalNotificationPublicationPort>,
    ) -> anyhow::Result<()> {
        self.port
            .set(port)
            .map_err(|_| anyhow::anyhow!("optional notification publication already activated"))
    }
}
#[async_trait]
impl OptionalNotificationPublicationPort for HostOptionalNotificationPublication {
    async fn publish_optional_notification(
        &self,
        input: &PublishOptionalNotificationInput,
    ) -> anyhow::Result<OptionalNotificationPublicationOutcome> {
        self.port
            .get()
            .ok_or_else(|| anyhow::anyhow!("optional notification publication not activated"))?
            .publish_optional_notification(input)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane_contracts::ports::OptionalNotificationContract;
    use uuid::Uuid;

    struct UnavailablePublication;
    #[async_trait]
    impl OptionalNotificationPublicationPort for UnavailablePublication {
        async fn publish_optional_notification(
            &self,
            _: &PublishOptionalNotificationInput,
        ) -> anyhow::Result<OptionalNotificationPublicationOutcome> {
            anyhow::bail!("declared publication source unavailable")
        }
    }

    #[tokio::test]
    async fn activation_is_once_only_and_unavailability_never_becomes_optional_zero() {
        let owner = HostOptionalNotificationPublication::default();
        let input = PublishOptionalNotificationInput {
            source_event_id: Uuid::now_v7(),
            transaction_id: Uuid::now_v7(),
            workspace_id: None,
            occurred_at: time::OffsetDateTime::now_utc(),
            contract: OptionalNotificationContract::ModelDefinition,
            canonical_payload: Vec::new(),
        };
        assert!(
            owner
                .publish_optional_notification(&input)
                .await
                .unwrap_err()
                .to_string()
                .contains("not activated")
        );
        owner.activate(Arc::new(UnavailablePublication)).unwrap();
        assert!(
            owner
                .publish_optional_notification(&input)
                .await
                .unwrap_err()
                .to_string()
                .contains("source unavailable")
        );
        assert!(owner.activate(Arc::new(UnavailablePublication)).is_err());
    }
}
