use control_plane_contracts::ports::{LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason};
use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleSubscriberTarget};
use control_plane_contracts::ports::{
    ModelDefinitionIntegrationNotification, OptionalNotificationContract,
    OptionalNotificationPublicationPort, PublishOptionalNotificationInput,
};
use plugin_framework::extension_bus::{
    compile_lifecycle_handler_registry, EffectiveLifecycleHandlerRegistry,
    EffectiveLifecycleSubscriber, EffectiveLifecycleSubscriberPlan, LifecycleHandlerBinding,
    LifecycleHandlerError, LifecycleHandlerFuture, TypedLifecycleSubscriberHandler,
};

use super::lifecycle_activation::HostExtensionLifecycleFactoryCatalog;

pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_ID: &str =
    "official.plugin-host.model-definition-committed";
pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_VERSION: &str = "v1";
struct ModelDefinitionCommittedHandler {
    publication: Arc<dyn OptionalNotificationPublicationPort>,
}

impl TypedLifecycleSubscriberHandler<control_plane_contracts::ports::ModelDefinitionCommittedFact>
    for ModelDefinitionCommittedHandler
{
    fn handle<'a>(
        &'a self,
        fact: &'a extension_contracts::AfterCommitFact<
            control_plane_contracts::ports::ModelDefinitionCommittedFact,
        >,
    ) -> LifecycleHandlerFuture<'a> {
        Box::pin(async move {
            let publish = async {
                // Preserve the source's timestamp; retry must not create a new fact body.
                let source = serde_json::to_value(fact)?;
                let occurred_at_unix_ms = source["occurred_at_unix_ms"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("source fact timestamp is missing"))?;
                let payload = fact.payload();
                let notification = extension_contracts::AfterCommitFact::new(
                    fact.fact_id().clone(),
                    fact.transaction_id().clone(),
                    occurred_at_unix_ms,
                    ModelDefinitionIntegrationNotification {
                        model_definition_id: payload.model_definition_id,
                        scope_kind: payload.scope_kind,
                        scope_id: payload.scope_id,
                    },
                );
                self.publication
                    .publish_optional_notification(&PublishOptionalNotificationInput {
                        source_event_id: uuid::Uuid::parse_str(fact.fact_id().as_str())?,
                        transaction_id: uuid::Uuid::parse_str(fact.transaction_id().as_str())?,
                        workspace_id: (payload.scope_kind == domain::DataModelScopeKind::Workspace)
                            .then_some(payload.scope_id),
                        occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
                            i128::from(occurred_at_unix_ms) * 1_000_000,
                        )?,
                        contract: OptionalNotificationContract::ModelDefinition,
                        canonical_payload: serde_json::to_vec(&notification)?,
                    })
                    .await?;
                // Completion here means optional publication adjudication or durable
                // stage handoff, never completion of downstream target effects.
                Ok::<(), anyhow::Error>(())
            }
            .await;
            publish.map_err(|error| LifecycleHandlerError::new(error.to_string()))
        })
    }
}

fn official_plugin_host_lifecycle_bindings(
    publication: Arc<dyn OptionalNotificationPublicationPort>,
) -> Vec<LifecycleHandlerBinding> {
    vec![LifecycleHandlerBinding::typed::<
        control_plane_contracts::ports::ModelDefinitionCommittedFact,
        _,
    >(
        MODEL_DEFINITION_COMMITTED_HANDLER_ID,
        MODEL_DEFINITION_COMMITTED_HANDLER_VERSION,
        Arc::new(ModelDefinitionCommittedHandler { publication }),
    )]
}

pub fn production_lifecycle_handler_factories(
    publication: Arc<dyn OptionalNotificationPublicationPort>,
) -> Result<HostExtensionLifecycleFactoryCatalog> {
    let mut factories = HostExtensionLifecycleFactoryCatalog::default();
    factories.register(
        "builtin://official.plugin-host",
        "builtin_plugin_host",
        Arc::new(move || {
            Ok(official_plugin_host_lifecycle_bindings(Arc::clone(
                &publication,
            )))
        }),
    )?;
    Ok(factories)
}

pub struct ApiLifecycleFactDelivery {
    registry: EffectiveLifecycleHandlerRegistry,
    native_targets: Vec<EffectiveLifecycleSubscriber>,
    managed: Option<std::sync::Weak<dyn crate::extension_bus::ManagedCompositionPort>>,
}

impl ApiLifecycleFactDelivery {
    pub fn with_managed(
        mut self,
        composition: &Arc<dyn crate::extension_bus::ManagedCompositionPort>,
    ) -> Self {
        self.managed = Some(Arc::downgrade(composition));
        self
    }

    pub fn bind(
        plan: &EffectiveLifecycleSubscriberPlan,
        handler_bindings: Vec<LifecycleHandlerBinding>,
    ) -> Result<(
        Self,
        control_plane_contracts::ports::LifecyclePublicationCatalog,
    )> {
        let mut publications: BTreeMap<(String, String), Vec<LifecycleSubscriberTarget>> =
            BTreeMap::new();
        for subscriber in plan.subscribers() {
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
        let native_targets = plan.subscribers().to_vec();
        let registry = compile_lifecycle_handler_registry(plan, handler_bindings)?;
        Ok((
            Self {
                registry,
                native_targets,
                managed: None,
            },
            catalog,
        ))
    }
}

#[async_trait]
impl control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort
    for ApiLifecycleFactDelivery
{
    async fn terminate_command(
        &self,
        fact: &LifecycleOutboxRecord,
        termination: control_plane_contracts::ports::CommunicationCommandTermination,
    ) -> Result<()> {
        let owner = self
            .managed
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| anyhow::anyhow!("command termination composition unavailable"))?;
        owner.terminate_command_delivery(fact, termination).await
    }
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        control_plane_contracts::ports::with_lifecycle_workflow_ancestors(
            fact.observation_context
                .as_ref()
                .map(|context| context.workflow_ancestor_ids.clone())
                .unwrap_or_default(),
            self.deliver_owned(fact),
        )
        .await
    }
}

impl ApiLifecycleFactDelivery {
    async fn deliver_owned(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        if let Some(owner) = &self.managed {
            let owner = owner.upgrade().ok_or({
                LifecycleDeliveryBlocked(LifecycleDeliveryPauseReason::FrozenGraphUnavailable)
            })?;
            if let Some(snapshot) = owner.event_snapshot_for_graph(fact).await? {
                if owner.deliver_event_record(&snapshot, fact).await? {
                    return Ok(());
                }
                // Exact native target was validated against the combined frozen plan above.
                return self
                    .registry
                    .deliver(
                        self.registry.graph_fingerprint(),
                        &fact.handler_id,
                        &fact.handler_version,
                        &fact.contract_id,
                        &fact.contract_version,
                        &fact.canonical_payload,
                    )
                    .await
                    .map_err(anyhow::Error::from);
            }
        }
        if fact.graph_fingerprint != self.registry.graph_fingerprint() {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenGraphUnavailable,
            )
            .into());
        }
        if !self.native_targets.iter().any(|target| {
            target.subscriber_id == fact.subscriber_id
                && target.handler_id == fact.handler_id
                && target.handler_version == fact.handler_version
                && target.fact_contract_id == fact.contract_id
                && target.fact_contract_version == fact.contract_version
        }) {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
            )
            .into());
        }
        self.registry
            .deliver(
                &fact.graph_fingerprint,
                &fact.handler_id,
                &fact.handler_version,
                &fact.contract_id,
                &fact.contract_version,
                &fact.canonical_payload,
            )
            .await
            .map_err(anyhow::Error::from)
    }
}

/// Pure typed definition projection. It does not create a lease or a subscriber queue.
pub(crate) fn native_lifecycle_source_descriptors(
    _plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) -> Vec<domain::event_center::EventSourceDescriptor> {
    use control_plane::lifecycle_outbox_dispatcher::lifecycle_source_id;
    use control_plane_contracts::ports::{CreditNotificationFact, ModelDefinitionCommittedFact};
    use domain::event_center::*;
    use extension_contracts::LifecycleContract;
    [
        (ModelDefinitionCommittedFact::CONTRACT_ID, ModelDefinitionCommittedFact::event_source_schema(), extension_contracts::MANAGED_CREATE_EVENT_POINT),
        (CreditNotificationFact::CONTRACT_ID, CreditNotificationFact::event_source_schema(), "1flowbase.credit.integration-notification"),
        (ModelDefinitionIntegrationNotification::CONTRACT_ID, ModelDefinitionIntegrationNotification::event_source_schema(), "1flowbase.model-definition.integration-notification"),
    ].into_iter().filter(|(contract, _, _)| scope != EventSourceScope::System || *contract != CreditNotificationFact::CONTRACT_ID)
    .map(|(contract, schema, point)| EventSourceDescriptor {
        source_id: lifecycle_source_id(&scope, contract), owner_id: "control-plane.lifecycle".into(),
        point_id: point.into(), contract_id: contract.into(),
        name: contract.into(),
        description: "Native committed lifecycle fact. Scoped queue counts cover trusted metadata only; historical unbound deliveries are listed separately and cannot be assigned to a workspace.".into(),
        payload_schema: schema, origin: EventDefinitionOrigin::Core, scope: scope.clone(),
        binding: if scope == EventSourceScope::Unbound { EventSourceBinding::Declaration } else { EventSourceBinding::Active },
        coverage: if scope == EventSourceScope::Unbound { EventSourceCoverage::Unavailable } else { EventSourceCoverage::Live },
        supported_phases: if scope == EventSourceScope::Unbound { Vec::new() } else { vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed] },
    }).collect()
}

/// Install native templates once at boot; never instantiate every possible workspace/queue.
/// The scope argument filters the definition family only. Actual leases are activated from
/// trusted publication/consumption context or from existing scoped durable queue rows.
pub fn register_native_lifecycle_sources(
    observer: &control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver,
    plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) {
    use control_plane::lifecycle_outbox_dispatcher::lifecycle_source_id;
    use domain::event_center::*;
    for mut descriptor in native_lifecycle_source_descriptors(plan, scope) {
        descriptor.scope = EventSourceScope::Unbound;
        descriptor.source_id = lifecycle_source_id(&descriptor.scope, &descriptor.contract_id);
        descriptor.binding = EventSourceBinding::Declaration;
        descriptor.coverage = EventSourceCoverage::Unavailable;
        descriptor.supported_phases.clear();
        use control_plane_contracts::ports::{
            CreditNotificationFact, ModelDefinitionCommittedFact,
        };
        use extension_contracts::LifecycleContract;
        let version = match descriptor.contract_id.as_str() {
            ModelDefinitionCommittedFact::CONTRACT_ID => {
                ModelDefinitionCommittedFact::CONTRACT_VERSION
            }
            CreditNotificationFact::CONTRACT_ID => CreditNotificationFact::CONTRACT_VERSION,
            ModelDefinitionIntegrationNotification::CONTRACT_ID => {
                ModelDefinitionIntegrationNotification::CONTRACT_VERSION
            }
            _ => continue,
        };
        let contract = descriptor.contract_id.clone();
        observer.register(descriptor, (&contract, version), false, &[]);
    }
}

/// Logical subtype leaves select one existing fact; they do not create publications or leases.
pub fn native_lifecycle_catalog_entries(
    plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) -> Vec<domain::event_center::EventCatalogEntry> {
    use control_plane_contracts::ports::{CreditNotificationEventType, CreditNotificationFact};
    use domain::event_center::EventCatalogEntry;
    use extension_contracts::LifecycleContract;
    let mut entries = Vec::new();
    for descriptor in native_lifecycle_source_descriptors(plan, scope) {
        let entry: EventCatalogEntry = descriptor.into();
        if entry.contract_id == CreditNotificationFact::CONTRACT_ID {
            let mut parent = entry.clone();
            parent.group_path = vec!["credits".into()];
            entries.push(parent);
            for kind in CreditNotificationEventType::ALL {
                let mut leaf = entry.clone();
                leaf.group_path =
                    vec!["credits".into(), CreditNotificationFact::CONTRACT_ID.into()];
                leaf.name = kind.wire_name().into();
                leaf.source
                    .as_mut()
                    .expect("source projection")
                    .selection
                    .filters
                    .insert("/payload/event_type".into(), kind.wire_name().into());
                leaf.payload_schema["properties"]["payload"]["properties"]["event_type"] =
                    serde_json::json!({"type":"string","const":kind.wire_name()});
                entries.push(leaf);
            }
        } else {
            entries.push(entry);
        }
    }
    entries
}
