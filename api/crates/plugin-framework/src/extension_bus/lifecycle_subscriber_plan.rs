use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::{
    ContributionId, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointId,
    ExtensionPointKind, LifecycleSemantics, ModuleKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleSubscriberBinding {
    pub contribution_id: ContributionId,
    pub subscription_id: String,
    pub point_id: ExtensionPointId,
    pub fact_contract_id: String,
    pub fact_contract_version: String,
    pub handler_id: String,
    pub handler_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveLifecycleSubscriber {
    pub subscriber_id: String,
    pub contributor_module_id: String,
    pub contributor_module_kind: ModuleKind,
    pub lifecycle: LifecycleSemantics,
    pub fact_contract_id: String,
    pub fact_contract_version: String,
    pub handler_id: String,
    pub handler_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveLifecycleSubscriberPlan {
    graph_fingerprint: String,
    subscribers: Vec<EffectiveLifecycleSubscriber>,
}

impl EffectiveLifecycleSubscriberPlan {
    pub fn graph_fingerprint(&self) -> &str {
        &self.graph_fingerprint
    }

    pub fn subscribers(&self) -> &[EffectiveLifecycleSubscriber] {
        &self.subscribers
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LifecycleSubscriberPlanError {
    #[error("duplicate lifecycle subscriber {0}")]
    DuplicateSubscriber(String),
    #[error("lifecycle subscriber {0} is not active in the effective graph")]
    InactiveSubscriber(String),
    #[error("lifecycle subscriber point {0} is missing")]
    MissingPoint(String),
    #[error("lifecycle subscriber point {0} is not an after-commit durable event stream")]
    InvalidPoint(String),
    #[error("active lifecycle contribution {0} has no typed subscriber binding")]
    MissingBinding(String),
    #[error("module kind {module_kind:?} cannot contribute lifecycle {lifecycle:?} through {subscription_id}")]
    LifecycleEscalation {
        subscription_id: String,
        module_kind: ModuleKind,
        lifecycle: LifecycleSemantics,
    },
}

pub fn compile_lifecycle_subscriber_plan(
    graph: &EffectiveExtensionGraph,
    bindings: Vec<LifecycleSubscriberBinding>,
) -> Result<EffectiveLifecycleSubscriberPlan, LifecycleSubscriberPlanError> {
    let mut indexed = BTreeMap::new();
    for binding in bindings {
        if indexed
            .insert(binding.contribution_id.clone(), binding)
            .is_some()
        {
            return Err(LifecycleSubscriberPlanError::DuplicateSubscriber(
                indexed
                    .keys()
                    .last()
                    .map(|id| id.as_str().to_string())
                    .unwrap_or_default(),
            ));
        }
    }

    let mut active = BTreeSet::new();
    let mut subscribers = Vec::new();
    for point in graph.points() {
        if point.descriptor().point_kind != ExtensionPointKind::EventStream
            || point.descriptor().delivery != DeliverySemantics::AfterCommitDurable
        {
            continue;
        }
        for contribution in point.contributions() {
            let contribution_id = &contribution.descriptor().contribution_id;
            active.insert(contribution_id.clone());
            let binding = indexed.get(contribution_id).ok_or_else(|| {
                LifecycleSubscriberPlanError::MissingBinding(contribution_id.as_str().to_string())
            })?;
            if binding.point_id != point.descriptor().point_id {
                return Err(LifecycleSubscriberPlanError::InvalidPoint(
                    binding.point_id.as_str().to_string(),
                ));
            }
            let module_kind = contribution.provenance().module_kind();
            let lifecycle = point.descriptor().lifecycle;
            if !allows_lifecycle(module_kind, lifecycle) {
                return Err(LifecycleSubscriberPlanError::LifecycleEscalation {
                    subscription_id: binding.subscription_id.clone(),
                    module_kind,
                    lifecycle,
                });
            }
            subscribers.push(EffectiveLifecycleSubscriber {
                subscriber_id: binding.subscription_id.clone(),
                contributor_module_id: contribution.provenance().module_id().as_str().to_string(),
                contributor_module_kind: module_kind,
                lifecycle,
                fact_contract_id: binding.fact_contract_id.clone(),
                fact_contract_version: binding.fact_contract_version.clone(),
                handler_id: binding.handler_id.clone(),
                handler_version: binding.handler_version.clone(),
            });
        }
    }

    if let Some(binding) = indexed
        .iter()
        .find_map(|(id, binding)| (!active.contains(id)).then_some(binding))
    {
        let point = graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id == binding.point_id)
            .ok_or_else(|| {
                LifecycleSubscriberPlanError::MissingPoint(binding.point_id.as_str().to_string())
            })?;
        if point.descriptor().point_kind != ExtensionPointKind::EventStream
            || point.descriptor().delivery != DeliverySemantics::AfterCommitDurable
        {
            return Err(LifecycleSubscriberPlanError::InvalidPoint(
                binding.point_id.as_str().to_string(),
            ));
        }
        return Err(LifecycleSubscriberPlanError::InactiveSubscriber(
            binding.subscription_id.clone(),
        ));
    }

    subscribers.sort_by(|left, right| left.subscriber_id.cmp(&right.subscriber_id));
    let mut ids = BTreeSet::new();
    if let Some(duplicate) = subscribers
        .iter()
        .find(|subscriber| !ids.insert(subscriber.subscriber_id.clone()))
    {
        return Err(LifecycleSubscriberPlanError::DuplicateSubscriber(
            duplicate.subscriber_id.clone(),
        ));
    }
    Ok(EffectiveLifecycleSubscriberPlan {
        graph_fingerprint: graph.fingerprint().as_str().to_string(),
        subscribers,
    })
}

fn allows_lifecycle(module_kind: ModuleKind, lifecycle: LifecycleSemantics) -> bool {
    match module_kind {
        ModuleKind::BootCore => true,
        ModuleKind::TrustedHost => matches!(
            lifecycle,
            LifecycleSemantics::BootSnapshot | LifecycleSemantics::Invocation
        ),
        ModuleKind::Runtime => matches!(
            lifecycle,
            LifecycleSemantics::RuntimeWorker | LifecycleSemantics::Invocation
        ),
        ModuleKind::Capability => matches!(
            lifecycle,
            LifecycleSemantics::WorkspaceAssignment
                | LifecycleSemantics::UiMount
                | LifecycleSemantics::Invocation
        ),
        ModuleKind::User => false,
    }
}
