use std::{collections::BTreeMap, sync::Arc};

use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::ports::EventBus;
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleSubscriberTarget};
use plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan;

pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_ID: &str =
    "official.plugin-host.model-definition-committed";
pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_VERSION: &str = "v1";
const MODEL_DEFINITION_COMMITTED_TOPIC: &str =
    "official.plugin-host.lifecycle.model-definition-committed";

#[async_trait]
trait LifecycleSubscriberHandler: Send + Sync {
    async fn handle(&self, fact: &LifecycleOutboxRecord) -> Result<()>;
}

struct ModelDefinitionCommittedHandler {
    event_bus: Arc<dyn EventBus>,
}

#[async_trait]
impl LifecycleSubscriberHandler for ModelDefinitionCommittedHandler {
    async fn handle(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        let typed: extension_contracts::AfterCommitFact<
            control_plane_contracts::ports::ModelDefinitionCommittedFact,
        > = serde_json::from_slice(&fact.canonical_payload)?;
        self.event_bus
            .publish(
                MODEL_DEFINITION_COMMITTED_TOPIC,
                serde_json::to_value(typed)?,
            )
            .await
    }
}

pub(crate) struct ApiLifecycleFactDelivery {
    graph_fingerprint: String,
    handlers: BTreeMap<String, (String, Arc<dyn LifecycleSubscriberHandler>)>,
}

impl ApiLifecycleFactDelivery {
    pub(crate) fn bind(
        plan: &EffectiveLifecycleSubscriberPlan,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<(
        Self,
        control_plane_contracts::ports::LifecyclePublicationCatalog,
    )> {
        let mut handlers = BTreeMap::new();
        let mut publications: BTreeMap<(String, String), Vec<LifecycleSubscriberTarget>> =
            BTreeMap::new();
        for subscriber in plan.subscribers() {
            let handler: Arc<dyn LifecycleSubscriberHandler> = match (
                subscriber.handler_id.as_str(),
                subscriber.handler_version.as_str(),
            ) {
                (
                    MODEL_DEFINITION_COMMITTED_HANDLER_ID,
                    MODEL_DEFINITION_COMMITTED_HANDLER_VERSION,
                ) => Arc::new(ModelDefinitionCommittedHandler {
                    event_bus: Arc::clone(&event_bus),
                }),
                _ => bail!(
                    "lifecycle subscriber handler {}@{} is not bound",
                    subscriber.handler_id,
                    subscriber.handler_version
                ),
            };
            if handlers
                .insert(
                    subscriber.handler_id.clone(),
                    (subscriber.handler_version.clone(), handler),
                )
                .is_some()
            {
                bail!(
                    "duplicate lifecycle handler binding {}",
                    subscriber.handler_id
                );
            }
            publications
                .entry((
                    subscriber.fact_contract_id.clone(),
                    subscriber.fact_contract_version.clone(),
                ))
                .or_default()
                .push(LifecycleSubscriberTarget {
                    subscriber_id: subscriber.subscriber_id.clone(),
                    handler_id: subscriber.handler_id.clone(),
                    handler_version: subscriber.handler_version.clone(),
                });
        }
        let catalog = control_plane_contracts::ports::LifecyclePublicationCatalog::new(
            publications.into_iter().map(|(contract, subscribers)| {
                (
                    contract,
                    control_plane_contracts::ports::LifecyclePublicationPlan {
                        graph_fingerprint: plan.graph_fingerprint().to_string(),
                        subscribers,
                    },
                )
            }),
        )?;
        Ok((
            Self {
                graph_fingerprint: plan.graph_fingerprint().to_string(),
                handlers,
            },
            catalog,
        ))
    }
}

#[async_trait]
impl control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort
    for ApiLifecycleFactDelivery
{
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        if fact.graph_fingerprint != self.graph_fingerprint {
            bail!("frozen lifecycle graph fingerprint is not available");
        }
        let (version, handler) = self
            .handlers
            .get(&fact.handler_id)
            .ok_or_else(|| anyhow::anyhow!("frozen lifecycle handler is not available"))?;
        if version != &fact.handler_version {
            bail!("frozen lifecycle handler version is not available");
        }
        handler.handle(fact).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort;
    use control_plane_contracts::ports::{LifecycleOutboxStatus, ModelDefinitionCommittedFact};
    use extension_contracts::{AfterCommitFact, LifecycleFactId, LifecycleTransactionId};
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[tokio::test]
    async fn real_host_extension_subscriber_receives_typed_after_commit_fact() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            &root,
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let event_bus = Arc::new(storage_ephemeral::MemoryEventBus::new());
        let (delivery, _) =
            ApiLifecycleFactDelivery::bind(&plan, event_bus.clone() as Arc<dyn EventBus>).unwrap();
        let event_id = Uuid::now_v7();
        let transaction_id = Uuid::now_v7();
        let fact = AfterCommitFact::new(
            LifecycleFactId::new(event_id.to_string()).unwrap(),
            LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
            1_700_000_000_000,
            ModelDefinitionCommittedFact {
                model_definition_id: Uuid::now_v7(),
                scope_kind: domain::DataModelScopeKind::System,
                scope_id: Uuid::nil(),
            },
        );
        let subscriber = &plan.subscribers()[0];
        let record = LifecycleOutboxRecord {
            event_id,
            transaction_id,
            contract_id: subscriber.fact_contract_id.clone(),
            contract_version: subscriber.fact_contract_version.clone(),
            canonical_payload: serde_json::to_vec(&fact).unwrap(),
            occurred_at: OffsetDateTime::now_utc(),
            graph_fingerprint: plan.graph_fingerprint().to_string(),
            subscriber_id: subscriber.subscriber_id.clone(),
            handler_id: subscriber.handler_id.clone(),
            handler_version: subscriber.handler_version.clone(),
            status: LifecycleOutboxStatus::Claimed,
            attempt_count: 1,
            available_at: OffsetDateTime::now_utc(),
            claimed_by: Some(Uuid::now_v7()),
            claimed_at: Some(OffsetDateTime::now_utc()),
            delivered_at: None,
        };
        delivery.deliver(&record).await.unwrap();
        assert!(event_bus
            .poll(MODEL_DEFINITION_COMMITTED_TOPIC)
            .await
            .unwrap()
            .is_some());
    }
}
