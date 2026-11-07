//! Adapter for existing durable owners; it neither reads nor drains the original outbox.
use control_plane_contracts::ports::{
    EventSourceFact, EventSourceObservation, EventSourceObserver, EventSourceRegistration,
    LifecycleFactObserver, LifecycleObservationContext, LifecycleOutboxRecord,
    LifecycleSubscriberTarget, RecordLifecycleFactInput,
};
use domain::event_center::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct RegisteredSource {
    lease: Arc<dyn EventSourceRegistration>,
    definition_fingerprint: [u8; 32],
    managed_owned: bool,
    phases: Vec<EventTriggerPhase>,
    native_template: Option<EventSourceDescriptor>,
}

// Registration-only comparison evidence. The center owns live scoped descriptors and
// charges their schemas to its observation budget; this adapter retains only 32 bytes.
fn definition_fingerprint(descriptor: &EventSourceDescriptor) -> anyhow::Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    // The workspace enables preserve_order. Canonicalize the temporary schema so equal
    // objects with different insertion order remain the same current definition.
    let mut schema = descriptor.payload_schema.clone();
    schema.sort_all_objects();
    Ok(Sha256::digest(serde_json::to_vec(&(
        &descriptor.source_id,
        &descriptor.contract_id,
        &descriptor.scope,
        &descriptor.owner_id,
        &descriptor.point_id,
        &schema,
        descriptor.origin,
    ))?)
    .into())
}

pub struct LifecycleSourceObserver {
    observer: Arc<dyn EventSourceObserver>,
    sources: Mutex<BTreeMap<(EventSourceScope, String, String), RegisteredSource>>,
}

impl LifecycleSourceObserver {
    pub fn new(observer: Arc<dyn EventSourceObserver>) -> Self {
        Self {
            observer,
            sources: Mutex::new(BTreeMap::new()),
        }
    }

    /// Source owner supplies actual schema/point and frozen subscriber bindings before publish.
    /// Refresh restores queue declarations only; it never fabricates lifecycle observations.
    /// Returns whether the definition was accepted; observation rejection never changes delivery.
    pub fn register(
        &self,
        descriptor: EventSourceDescriptor,
        protocol_identity: (&str, &str),
        managed_owned: bool,
        targets: &[LifecycleSubscriberTarget],
    ) -> bool {
        let key = (
            descriptor.scope.clone(),
            protocol_identity.0.to_owned(),
            protocol_identity.1.to_owned(),
        );
        let definition_fingerprint = match definition_fingerprint(&descriptor) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(contract_id = %descriptor.contract_id, %error, "lifecycle source definition fingerprint unavailable");
                return false;
            }
        };
        let Ok(mut sources) = self.sources.lock() else {
            return false;
        };
        if let Some(existing) = sources.get(&key) {
            if existing.definition_fingerprint != definition_fingerprint
                || existing.managed_owned != managed_owned
            {
                tracing::error!(contract_id = %descriptor.contract_id, "conflicting lifecycle observation definition");
                return false;
            }
        }
        if sources
            .get(&key)
            .is_none_or(|source| source.phases != descriptor.supported_phases)
        {
            let native_template = (descriptor.scope == EventSourceScope::Unbound
                && native_scope_supported(
                    protocol_identity.0,
                    protocol_identity.1,
                    &EventSourceScope::Unbound,
                )
                && !descriptor.payload_schema.is_null())
            .then(|| descriptor.clone());
            let phases = descriptor.supported_phases.clone();
            let lease = match self.observer.register_source(descriptor) {
                Ok(lease) => lease,
                Err(error) => {
                    tracing::warn!(contract_id = %key.1, %error, "lifecycle source observation registration rejected");
                    return false;
                }
            };
            sources.insert(
                key.clone(),
                RegisteredSource {
                    lease,
                    definition_fingerprint,
                    managed_owned,
                    phases,
                    native_template,
                },
            );
        }
        if key.0 != EventSourceScope::Unbound {
            if let Some(source) = sources.get(&key) {
                source.lease.set_queue_snapshots(
                    targets
                        .iter()
                        .map(|target| EventSourceQueueSnapshot {
                            queue_id: format!("lifecycle-outbox/{}", target.subscriber_id),
                            subscriber_id: target.subscriber_id.clone(),
                            consumer_ids: vec![target.handler_id.clone()],
                            buffered: None,
                            in_flight: None,
                            accepted: None,
                            dequeued: None,
                            dropped: None,
                            observed_at_unix_ms: now_ms(),
                        })
                        .collect(),
                );
            }
        }
        true
    }

    /// Root calls this after the repository's read-only grouped snapshot, including on GET.
    /// Counts come from existing durable delivery rows, including retry/restart backlog.
    pub fn refresh_queue_snapshots(
        &self,
        facts: Vec<control_plane_contracts::ports::LifecycleSourceQueueFacts>,
    ) {
        // This is a complete repository snapshot, so omitted/removed rows clear old counts.
        let keys = facts
            .iter()
            .map(|source| {
                (
                    source.scope.clone(),
                    source.contract_id.clone(),
                    source.contract_version.clone(),
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        if let Ok(mut sources) = self.sources.lock() {
            for source in sources.values() {
                source.lease.set_queue_snapshots(Vec::new());
            }
            // Queue-recovered unknown declarations have no owner after the last durable row.
            sources.retain(|key, source| {
                source.native_template.is_some() || !source.phases.is_empty() || keys.contains(key)
            });
        }
        for source in facts {
            let context = LifecycleObservationContext {
                scope: source.scope.clone(),
                producer_id: "durable-queue-owner".into(),
                workflow_ancestor_ids: Vec::new(),
            };
            let lease = self.source(&context, &source.contract_id, &source.contract_version);
            let lease = if lease.is_none() {
                // An existing row proves queue ownership, but never proves a missing schema or
                // executable frozen graph. Such queues remain visible with no trigger capability.
                self.register(EventSourceDescriptor {
                    source_id: lifecycle_source_id(&source.scope, &source.contract_id),
                    owner_id: "control-plane.lifecycle.historical".into(), point_id: "lifecycle-outbox.historical".into(),
                    contract_id: source.contract_id.clone(),
                    name: source.contract_id.clone(),
                    description: if source.scope == EventSourceScope::Unbound {
                        "Historical durable deliveries lack trusted source scope. Their backlog is unbound; scoped queue counts exclude these rows and cannot describe the complete legacy backlog."
                    } else {
                        "Existing durable deliveries retain trusted scope, but their historical schema or frozen source graph is unavailable. Queue backlog is visible; workflow triggers are unavailable."
                    }.into(),
                    payload_schema: serde_json::Value::Null, origin: EventDefinitionOrigin::Core, scope: source.scope.clone(),
                    binding: EventSourceBinding::Declaration, coverage: EventSourceCoverage::Unavailable, supported_phases: Vec::new(),
                }, (&source.contract_id, &source.contract_version), false, &[]);
                self.source(&context, &source.contract_id, &source.contract_version)
            } else {
                lease
            };
            if let Some(lease) = lease {
                lease.set_queue_snapshots(source.queues);
            }
        }
    }

    /// Current and retained frozen graph ownership controls managed source lifetime.
    pub fn retain_managed_sources(
        &self,
        keys: &std::collections::BTreeSet<(EventSourceScope, String, String)>,
    ) {
        if let Ok(mut sources) = self.sources.lock() {
            sources.retain(|key, source| !source.managed_owned || keys.contains(key));
        }
    }

    pub fn remove_scope(&self, scope: &EventSourceScope) {
        if let Ok(mut sources) = self.sources.lock() {
            sources.retain(|(s, _, _), _| s != scope);
        }
    }

    fn source(
        &self,
        context: &LifecycleObservationContext,
        contract: &str,
        version: &str,
    ) -> Option<Arc<dyn EventSourceRegistration>> {
        let mut sources = self.sources.lock().ok()?;
        let key = (
            context.scope.clone(),
            contract.to_owned(),
            version.to_owned(),
        );
        if let Some(source) = sources.get(&key) {
            return Some(source.lease.clone());
        }
        if !native_scope_supported(contract, version, &context.scope) {
            return None;
        }
        let mut descriptor = sources
            .get(&(EventSourceScope::Unbound, contract.into(), version.into()))?
            .native_template
            .clone()?;
        descriptor.scope = context.scope.clone();
        descriptor.source_id = lifecycle_source_id(&context.scope, contract);
        descriptor.binding = EventSourceBinding::Active;
        descriptor.coverage = EventSourceCoverage::Live;
        descriptor.supported_phases =
            vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed];
        let phases = descriptor.supported_phases.clone();
        let definition_fingerprint = match definition_fingerprint(&descriptor) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(contract_id = %contract, %error, "native source definition fingerprint unavailable");
                return None;
            }
        };
        let lease = match self.observer.register_source(descriptor) {
            Ok(lease) => lease,
            Err(error) => {
                tracing::warn!(contract_id = %contract, %error, "native source observation registration rejected");
                return None;
            }
        };
        sources.insert(
            key,
            RegisteredSource {
                lease: lease.clone(),
                definition_fingerprint,
                managed_owned: false,
                phases,
                native_template: None,
            },
        );
        Some(lease)
    }
}

impl LifecycleFactObserver for LifecycleSourceObserver {
    fn published(&self, input: &RecordLifecycleFactInput) {
        let Some(context) = &input.observation_context else {
            return;
        };
        let Some(lease) = self.source(context, &input.contract_id, &input.contract_version) else {
            return;
        };
        // Only the typed credit discriminator is decoded eagerly. The complete body is
        // projected only inside the callback for an actual workflow target.
        lease.observe_with_payload(
            EventSourceObservation {
                attributes: credit_attributes(&input.contract_id, &input.canonical_payload),

                original_event_id: Some(input.event_id.to_string()),
                producer_id: Some(context.producer_id.clone()),
                occurred_at_unix_ms: (input.occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
                workflow_ancestor_ids: context.workflow_ancestor_ids.clone(),
                fact: EventSourceFact::Published,
            },
            &mut || serde_json::from_slice(&input.canonical_payload).ok(),
        );
    }

    fn consumed(&self, record: &LifecycleOutboxRecord, consumer_id: &str) {
        let Some(context) = &record.observation_context else {
            return;
        };
        let Some(lease) = self.source(context, &record.contract_id, &record.contract_version)
        else {
            return;
        };
        lease.observe(EventSourceObservation {
            attributes: credit_attributes(&record.contract_id, &record.canonical_payload),

            original_event_id: Some(record.event_id.to_string()),
            producer_id: Some(context.producer_id.clone()),
            occurred_at_unix_ms: now_ms(),
            workflow_ancestor_ids: context.workflow_ancestor_ids.clone(),
            fact: EventSourceFact::Consumed {
                subscriber_id: record.subscriber_id.clone(),
                consumer_id: consumer_id.into(),
            },
        });
    }
}

fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}

/// Canonical source identity shared by host definitions, lazy activation and managed graphs.
pub fn lifecycle_source_id(scope: &EventSourceScope, contract: &str) -> String {
    let scope = match scope {
        EventSourceScope::Workspace { workspace_id } => format!("workspace/{workspace_id}"),
        EventSourceScope::System => "system".into(),
        EventSourceScope::Unbound => "unbound".into(),
    };
    fn component(value: &str) -> String {
        value
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"._-".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect()
    }
    format!("lifecycle/{scope}/contract/{}", component(contract))
}

fn native_scope_supported(contract: &str, version: &str, scope: &EventSourceScope) -> bool {
    use control_plane_contracts::ports::{
        CreditNotificationFact, ModelDefinitionCommittedFact,
        ModelDefinitionIntegrationNotification,
    };
    use extension_contracts::LifecycleContract;
    (contract == ModelDefinitionCommittedFact::CONTRACT_ID
        && version == ModelDefinitionCommittedFact::CONTRACT_VERSION)
        || (contract == ModelDefinitionIntegrationNotification::CONTRACT_ID
            && version == ModelDefinitionIntegrationNotification::CONTRACT_VERSION)
        || (contract == CreditNotificationFact::CONTRACT_ID
            && version == CreditNotificationFact::CONTRACT_VERSION
            && *scope != EventSourceScope::System)
}

/// Decode only the declared discriminator; unrelated payload fields are skipped by serde.
fn credit_attributes(contract: &str, payload: &[u8]) -> BTreeMap<String, String> {
    use control_plane_contracts::ports::{CreditNotificationEventType, CreditNotificationFact};
    use extension_contracts::LifecycleContract;
    #[derive(serde::Deserialize)]
    struct Envelope {
        payload: Kind,
    }
    #[derive(serde::Deserialize)]
    struct Kind {
        event_type: CreditNotificationEventType,
    }
    if contract != CreditNotificationFact::CONTRACT_ID {
        return BTreeMap::new();
    }
    serde_json::from_slice::<Envelope>(payload)
        .ok()
        .map(|value| {
            BTreeMap::from([(
                "/payload/event_type".into(),
                value.payload.event_type.wire_name().into(),
            )])
        })
        .unwrap_or_default()
}
