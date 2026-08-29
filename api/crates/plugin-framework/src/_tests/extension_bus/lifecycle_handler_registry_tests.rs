use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use extension_contracts::{
    AfterCommitFact, LifecycleContract, LifecycleFactId, LifecycleTransactionId,
};
use plugin_framework::extension_bus::{
    compile_extension_graph, compile_lifecycle_handler_registry, compile_lifecycle_subscriber_plan,
    Cardinality, ContractDescriptor, ContractVersion, ContributionDescriptor, ContributionId,
    ContributionMode, ContributionOrdering, DeliverySemantics, ExtensionBusVersion,
    ExtensionPointDescriptor, ExtensionPointId, ExtensionPointKind, FailureSemantics,
    LifecycleHandlerBinding, LifecycleHandlerError, LifecycleHandlerFuture, LifecycleSemantics,
    LifecycleSubscriberBinding, LifecycleSubscriberPlanError, ModuleActivationDeclaration,
    ModuleDescriptor, ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy,
    ScopeSemantics, TypedLifecycleSubscriberHandler,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FixtureCommittedFact {
    value: String,
}

impl LifecycleContract for FixtureCommittedFact {
    const CONTRACT_ID: &'static str = "fixture.committed";
    const CONTRACT_VERSION: &'static str = "v1";
}

struct RecordingPluginHandler {
    received: Arc<Mutex<Vec<String>>>,
    fail: bool,
}

impl TypedLifecycleSubscriberHandler<FixtureCommittedFact> for RecordingPluginHandler {
    fn handle<'a>(
        &'a self,
        fact: &'a AfterCommitFact<FixtureCommittedFact>,
    ) -> LifecycleHandlerFuture<'a> {
        Box::pin(async move {
            if self.fail {
                return Err(LifecycleHandlerError::new("fixture plugin rejected fact"));
            }
            self.received
                .lock()
                .expect("received facts mutex poisoned")
                .push(fact.payload().value.clone());
            Ok(())
        })
    }
}

#[tokio::test]
async fn activated_typed_plugin_handler_receives_only_its_frozen_contract() {
    let plan = plan(ModuleKind::TrustedHost, LifecycleSemantics::Invocation).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let registry = compile_lifecycle_handler_registry(
        &plan,
        vec![LifecycleHandlerBinding::typed::<FixtureCommittedFact, _>(
            "acme.lifecycle-handler",
            "v1",
            Arc::new(RecordingPluginHandler {
                received: Arc::clone(&received),
                fail: false,
            }),
        )],
    )
    .unwrap();
    let fact = AfterCommitFact::new(
        LifecycleFactId::new("fact-1").unwrap(),
        LifecycleTransactionId::new("transaction-1").unwrap(),
        1_700_000_000_000,
        FixtureCommittedFact {
            value: "committed".to_string(),
        },
    );

    registry
        .deliver(
            plan.graph_fingerprint(),
            "acme.lifecycle-handler",
            "v1",
            FixtureCommittedFact::CONTRACT_ID,
            FixtureCommittedFact::CONTRACT_VERSION,
            &serde_json::to_vec(&fact).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        received
            .lock()
            .expect("received facts mutex poisoned")
            .as_slice(),
        ["committed"]
    );

    let error = registry
        .deliver(
            plan.graph_fingerprint(),
            "acme.lifecycle-handler",
            "v1",
            "other.contract",
            "v1",
            &serde_json::to_vec(&fact).unwrap(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("does not match frozen handler"));
}

#[test]
fn lifecycle_contribution_admission_is_bounded_by_plugin_kind() {
    for (kind, lifecycle) in [
        (ModuleKind::TrustedHost, LifecycleSemantics::BootSnapshot),
        (ModuleKind::TrustedHost, LifecycleSemantics::Invocation),
    ] {
        plan(kind, lifecycle).unwrap();
    }

    for (kind, lifecycle) in [
        (ModuleKind::TrustedHost, LifecycleSemantics::RuntimeWorker),
        (ModuleKind::TrustedHost, LifecycleSemantics::UiMount),
        (ModuleKind::Runtime, LifecycleSemantics::BootSnapshot),
        (ModuleKind::Runtime, LifecycleSemantics::RuntimeWorker),
        (ModuleKind::Runtime, LifecycleSemantics::Invocation),
        (ModuleKind::Runtime, LifecycleSemantics::WorkspaceAssignment),
        (ModuleKind::Capability, LifecycleSemantics::BootSnapshot),
        (ModuleKind::Capability, LifecycleSemantics::RuntimeWorker),
        (
            ModuleKind::Capability,
            LifecycleSemantics::WorkspaceAssignment,
        ),
        (ModuleKind::Capability, LifecycleSemantics::UiMount),
        (ModuleKind::Capability, LifecycleSemantics::Invocation),
        (ModuleKind::User, LifecycleSemantics::Invocation),
    ] {
        assert!(matches!(
            plan(kind, lifecycle),
            Err(LifecycleSubscriberPlanError::LifecycleEscalation { .. })
        ));
    }
}

fn plan(
    plugin_kind: ModuleKind,
    lifecycle: LifecycleSemantics,
) -> Result<
    plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan,
    LifecycleSubscriberPlanError,
> {
    let point_id = ExtensionPointId::new("core.lifecycle.after-commit").unwrap();
    let contribution_id = ContributionId::new("acme.lifecycle.subscription").unwrap();
    let core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("core").unwrap(),
        module_version: ModuleVersion::new("1.0.0").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![ExtensionPointDescriptor {
            point_id: point_id.clone(),
            owner_module_id: ModuleId::new("core").unwrap(),
            point_kind: ExtensionPointKind::EventStream,
            contract: ContractDescriptor::new("lifecycle.lane", "v1").unwrap(),
            scope: ScopeSemantics::Global,
            cardinality: Cardinality::Many,
            ordering: OrderingSemantics::Dependency,
            failure: FailureSemantics::IsolateContribution,
            delivery: DeliverySemantics::AfterCommitDurable,
            lifecycle,
            allowed_permissions: BTreeSet::new(),
            override_policy: OverridePolicy::Sealed,
        }],
        contributions: Vec::new(),
    };
    let plugin = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("acme.lifecycle-plugin").unwrap(),
        module_version: ModuleVersion::new("1.0.0").unwrap(),
        module_kind: plugin_kind,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: contribution_id.clone(),
            contributor_module_id: ModuleId::new("acme.lifecycle-plugin").unwrap(),
            point_id: point_id.clone(),
            contract_version: ContractVersion::new("v1").unwrap(),
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        }],
    };
    let graph = compile_extension_graph(vec![core, plugin]).unwrap();
    compile_lifecycle_subscriber_plan(
        &graph,
        vec![LifecycleSubscriberBinding {
            contribution_id,
            subscription_id: "acme.lifecycle-subscriber".to_string(),
            point_id,
            fact_contract_id: FixtureCommittedFact::CONTRACT_ID.to_string(),
            fact_contract_version: FixtureCommittedFact::CONTRACT_VERSION.to_string(),
            handler_id: "acme.lifecycle-handler".to_string(),
            handler_version: "v1".to_string(),
        }],
    )
}
