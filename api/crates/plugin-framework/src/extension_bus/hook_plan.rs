use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use super::{
    ContributionId, EffectiveExtensionGraph, ExtensionGraphFingerprint, ExtensionPointId,
    ExtensionPointKind, HookContractError, HookHandlerContract, HookPhase, HookPointContract,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookPointBinding {
    point_id: ExtensionPointId,
    contract: HookPointContract,
}

impl HookPointBinding {
    pub fn new(point_id: ExtensionPointId, contract: HookPointContract) -> Self {
        Self { point_id, contract }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookHandlerBinding {
    contribution_id: ContributionId,
    contract: HookHandlerContract,
}

impl HookHandlerBinding {
    pub fn new(contribution_id: ContributionId, contract: HookHandlerContract) -> Self {
        Self {
            contribution_id,
            contract,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveHookHandler {
    contribution_id: ContributionId,
    contract: HookHandlerContract,
}

impl EffectiveHookHandler {
    pub fn contribution_id(&self) -> &ContributionId {
        &self.contribution_id
    }

    pub fn contract(&self) -> &HookHandlerContract {
        &self.contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveHookPlan {
    point_id: ExtensionPointId,
    phase: HookPhase,
    graph_fingerprint: ExtensionGraphFingerprint,
    before_handlers: Vec<EffectiveHookHandler>,
    after_handlers: Vec<EffectiveHookHandler>,
}

impl EffectiveHookPlan {
    pub fn point_id(&self) -> &ExtensionPointId {
        &self.point_id
    }

    pub fn phase(&self) -> HookPhase {
        self.phase
    }

    pub fn graph_fingerprint(&self) -> &ExtensionGraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn before_handlers(&self) -> &[EffectiveHookHandler] {
        &self.before_handlers
    }

    pub fn after_handlers(&self) -> &[EffectiveHookHandler] {
        &self.after_handlers
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HookPlanCompilationError {
    #[error("duplicate hook point binding for {point_id:?}")]
    DuplicatePointBinding { point_id: ExtensionPointId },
    #[error("hook point {point_id:?} is not active in the effective graph")]
    MissingPoint { point_id: ExtensionPointId },
    #[error("extension point {point_id:?} is not a pipeline hook point")]
    InvalidPointKind { point_id: ExtensionPointId },
    #[error("hook point {point_id:?} has an invalid contract: {source}")]
    InvalidPointContract {
        point_id: ExtensionPointId,
        source: HookContractError,
    },
    #[error("duplicate hook handler binding for {contribution_id:?}")]
    DuplicateHandlerBinding { contribution_id: ContributionId },
    #[error("hook handler binding {contribution_id:?} is not active in the effective graph")]
    InactiveHandlerBinding { contribution_id: ContributionId },
    #[error("active hook contribution {contribution_id:?} has no typed handler binding")]
    MissingHandlerBinding { contribution_id: ContributionId },
    #[error("hook handler {contribution_id:?} contract does not match its point")]
    HandlerContractMismatch { contribution_id: ContributionId },
}

pub fn compile_hook_plans(
    graph: &EffectiveExtensionGraph,
    point_bindings: Vec<HookPointBinding>,
    handler_bindings: Vec<HookHandlerBinding>,
) -> Result<Vec<EffectiveHookPlan>, HookPlanCompilationError> {
    let mut points = BTreeMap::new();
    for binding in point_bindings {
        let point_id = binding.point_id.clone();
        if points.insert(point_id.clone(), binding).is_some() {
            return Err(HookPlanCompilationError::DuplicatePointBinding { point_id });
        }
    }

    let mut handlers = BTreeMap::new();
    for binding in handler_bindings {
        let contribution_id = binding.contribution_id.clone();
        if handlers.insert(contribution_id.clone(), binding).is_some() {
            return Err(HookPlanCompilationError::DuplicateHandlerBinding { contribution_id });
        }
    }

    let active_contributions = graph
        .points()
        .iter()
        .flat_map(|point| point.contributions())
        .map(|entry| entry.descriptor().contribution_id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(contribution_id) = handlers
        .keys()
        .find(|contribution_id| !active_contributions.contains(*contribution_id))
    {
        return Err(HookPlanCompilationError::InactiveHandlerBinding {
            contribution_id: contribution_id.clone(),
        });
    }

    let mut plans = Vec::with_capacity(points.len());
    for (point_id, binding) in points {
        binding.contract.validate().map_err(|source| {
            HookPlanCompilationError::InvalidPointContract {
                point_id: point_id.clone(),
                source,
            }
        })?;
        let point = graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id == point_id)
            .ok_or_else(|| HookPlanCompilationError::MissingPoint {
                point_id: point_id.clone(),
            })?;
        if point.descriptor().point_kind != ExtensionPointKind::Pipeline {
            return Err(HookPlanCompilationError::InvalidPointKind { point_id });
        }

        let mut before_handlers = Vec::with_capacity(point.contributions().len());
        for contribution in point.contributions() {
            let contribution_id = contribution.descriptor().contribution_id.clone();
            let handler = handlers.get(&contribution_id).ok_or_else(|| {
                HookPlanCompilationError::MissingHandlerBinding {
                    contribution_id: contribution_id.clone(),
                }
            })?;
            if !handler.contract.matches_point(&binding.contract) {
                return Err(HookPlanCompilationError::HandlerContractMismatch { contribution_id });
            }
            before_handlers.push(EffectiveHookHandler {
                contribution_id: handler.contribution_id.clone(),
                contract: handler.contract.clone(),
            });
        }
        let mut after_handlers = before_handlers.clone();
        after_handlers.reverse();
        plans.push(EffectiveHookPlan {
            point_id,
            phase: binding.contract.phase,
            graph_fingerprint: graph.fingerprint().clone(),
            before_handlers,
            after_handlers,
        });
    }
    Ok(plans)
}
