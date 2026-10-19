use super::*;

#[derive(Clone, Copy)]
pub(super) enum AfterCommitBehavior {
    Success,
    Error,
    ErrorOnce,
    Timeout,
}

struct RecordingAfterCommitSubscriber {
    behavior: AfterCommitBehavior,
    calls: Arc<Mutex<Vec<RuntimeEventAfterCommitIdempotencyKey>>>,
}

#[async_trait::async_trait]
impl RuntimeEventAfterCommitSubscriber for RecordingAfterCommitSubscriber {
    async fn deliver(&self, delivery: RuntimeEventAfterCommitDelivery) -> anyhow::Result<()> {
        let call_number = {
            let mut calls = self.calls.lock().expect("after-commit calls lock poisoned");
            calls.push(delivery.idempotency_key);
            calls.len()
        };
        match self.behavior {
            AfterCommitBehavior::Success => Ok(()),
            AfterCommitBehavior::Error => anyhow::bail!("subscriber details stay private"),
            AfterCommitBehavior::ErrorOnce if call_number == 1 => {
                anyhow::bail!("subscriber details stay private")
            }
            AfterCommitBehavior::ErrorOnce => Ok(()),
            AfterCommitBehavior::Timeout => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(())
            }
        }
    }
}

pub(super) fn after_commit_graph() -> Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>
{
    let core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(RUNTIME_EVENT_LANE_OWNER_MODULE_ID).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![ExtensionPointDescriptor {
            point_id: ExtensionPointId::new(RUNTIME_EVENT_AFTER_COMMIT_POINT_ID).unwrap(),
            owner_module_id: ModuleId::new(RUNTIME_EVENT_LANE_OWNER_MODULE_ID).unwrap(),
            point_kind: ExtensionPointKind::EventStream,
            contract: ContractDescriptor::new(
                RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID,
                RUNTIME_EVENT_LANE_CONTRACT_VERSION,
            )
            .unwrap(),
            scope: ScopeSemantics::Global,
            cardinality: Cardinality::Many,
            ordering: OrderingSemantics::Dependency,
            failure: FailureSemantics::IsolateContribution,
            delivery: DeliverySemantics::AfterCommitDurable,
            lifecycle: LifecycleSemantics::Invocation,
            allowed_permissions: BTreeSet::new(),
            override_policy: OverridePolicy::Sealed,
        }],
        contributions: Vec::new(),
    };
    let subscriber = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("fixture.runtime-event-subscriber").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::TrustedHost,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new("fixture.runtime-event.after-commit").unwrap(),
            contributor_module_id: ModuleId::new("fixture.runtime-event-subscriber").unwrap(),
            point_id: ExtensionPointId::new(RUNTIME_EVENT_AFTER_COMMIT_POINT_ID).unwrap(),
            contract_version: ContractVersion::new(RUNTIME_EVENT_LANE_CONTRACT_VERSION).unwrap(),
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        }],
    };
    Arc::new(compile_extension_graph(vec![core, subscriber]).unwrap())
}

pub(super) fn after_commit_lane(
    behavior: AfterCommitBehavior,
    calls: Arc<Mutex<Vec<RuntimeEventAfterCommitIdempotencyKey>>>,
    timeout: Duration,
    max_attempts: u32,
) -> RuntimeEventAfterCommitLane {
    RuntimeEventAfterCommitLane::from_graph(
        after_commit_graph(),
        vec![TrustedRuntimeEventAfterCommitRegistration {
            contribution_id: "fixture.runtime-event.after-commit".to_string(),
            subscriber_id: RuntimeEventAfterCommitSubscriberId::new("fixture.subscriber").unwrap(),
            timeout,
            max_attempts,
            subscriber: Arc::new(RecordingAfterCommitSubscriber { behavior, calls }),
        }],
    )
    .unwrap()
}

pub(super) fn durable_lane_event(run_id: Uuid, sequence: i64) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        sequence,
        RuntimeEventPayload {
            event_type: "node_finished".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({ "type": "node_finished" }),
        },
    )
}
