use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ExtensionPlanFingerprint, HandlerReference, InterfaceId, InterfaceScope, PluginIdentity,
    TargetReference,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceExtensionTier {
    BuiltIn,
    HostExtension,
    RuntimeExtension,
    CapabilityPlugin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceExtensionPoint {
    Definition,
    AuthenticationAdapter,
    Authorization,
    Admission,
    Before,
    Handler,
    After,
    Failure,
    Completion,
}

impl InterfaceExtensionPoint {
    pub fn identity(self) -> &'static str {
        match self {
            Self::Definition => "interface.definition",
            Self::AuthenticationAdapter => "interface.authentication_adapter",
            Self::Authorization => "interface.authorization",
            Self::Admission => "interface.admission",
            Self::Before => "interface.before",
            Self::Handler => "interface.handler",
            Self::After => "interface.after",
            Self::Failure => "interface.failure",
            Self::Completion => "interface.completion",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceExtensionPermission {
    Define,
    Authenticate,
    Authorize,
    Admit,
    ObserveInput,
    MutateInput,
    Handle,
    ObserveOutput,
    ObserveFailure,
    ObserveCompletion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceExtensionIsolation {
    TrustedInProcess,
    ProcessWire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceExtensionFact {
    DefinitionIdentity,
    BindingIdentity,
    PrincipalSummary,
    AuthorizationDecision,
    AdmissionDecision,
    TypedInput,
    TypedOutput,
    FailureClassification,
    Terminal,
    InvocationIdentity,
    AttemptIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceExtensionRegistration {
    plugin: PluginIdentity,
    tier: InterfaceExtensionTier,
    point: InterfaceExtensionPoint,
    permission: InterfaceExtensionPermission,
    scope: InterfaceScope,
    isolation: InterfaceExtensionIsolation,
    facts: BTreeSet<InterfaceExtensionFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedInterfaceExtensionRegistration {
    order: u32,
    registration: InterfaceExtensionRegistration,
}

impl OrderedInterfaceExtensionRegistration {
    pub fn order(&self) -> u32 {
        self.order
    }

    pub fn registration(&self) -> &InterfaceExtensionRegistration {
        &self.registration
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledInterfaceExtensionPlan {
    registrations: Vec<OrderedInterfaceExtensionRegistration>,
    fingerprint: ExtensionPlanFingerprint,
}

impl CompiledInterfaceExtensionPlan {
    pub(crate) fn compile(
        registrations: impl IntoIterator<Item = (u32, InterfaceExtensionRegistration)>,
    ) -> Result<Self, InterfaceExtensionCompilationError> {
        let mut registrations = registrations
            .into_iter()
            .map(|(order, registration)| OrderedInterfaceExtensionRegistration {
                order,
                registration,
            })
            .collect::<Vec<_>>();
        registrations.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.registration.point.cmp(&right.registration.point))
                .then_with(|| left.registration.plugin.cmp(&right.registration.plugin))
        });
        for pair in registrations.windows(2) {
            if pair[0].order == pair[1].order
                && pair[0].registration.point == pair[1].registration.point
            {
                return Err(InterfaceExtensionCompilationError::DuplicateOrder {
                    order: pair[0].order,
                    point: pair[0].registration.point,
                });
            }
        }
        let mut digest = Sha256::new();
        for entry in &registrations {
            for part in [
                entry.order.to_string(),
                entry.registration.plugin.as_str().to_owned(),
                entry.registration.point.identity().to_owned(),
                format!("{:?}", entry.registration.tier),
                format!("{:?}", entry.registration.permission),
                format!("{:?}", entry.registration.scope),
                format!("{:?}", entry.registration.isolation),
            ] {
                digest.update([0]);
                digest.update(part.as_bytes());
            }
            for fact in &entry.registration.facts {
                digest.update([0]);
                digest.update(format!("{fact:?}").as_bytes());
            }
        }
        let fingerprint = ExtensionPlanFingerprint::new(format!("sha256:{:x}", digest.finalize()))
            .expect("SHA-256 extension plan fingerprint must be a valid identity");
        Ok(Self {
            registrations,
            fingerprint,
        })
    }

    pub fn registrations(&self) -> &[OrderedInterfaceExtensionRegistration] {
        &self.registrations
    }

    pub fn fingerprint(&self) -> &ExtensionPlanFingerprint {
        &self.fingerprint
    }
}

impl InterfaceExtensionRegistration {
    pub fn new(
        plugin: PluginIdentity,
        tier: InterfaceExtensionTier,
        point: InterfaceExtensionPoint,
        permission: InterfaceExtensionPermission,
        scope: InterfaceScope,
        isolation: InterfaceExtensionIsolation,
        facts: impl IntoIterator<Item = InterfaceExtensionFact>,
    ) -> Result<Self, InterfaceExtensionCompilationError> {
        let facts = facts.into_iter().collect::<BTreeSet<_>>();
        validate_registration(tier, point, permission, isolation, &facts)?;
        Ok(Self {
            plugin,
            tier,
            point,
            permission,
            scope,
            isolation,
            facts,
        })
    }

    pub fn plugin(&self) -> &PluginIdentity {
        &self.plugin
    }

    pub fn tier(&self) -> InterfaceExtensionTier {
        self.tier
    }

    pub fn point(&self) -> InterfaceExtensionPoint {
        self.point
    }

    pub fn permission(&self) -> InterfaceExtensionPermission {
        self.permission
    }

    pub fn scope(&self) -> InterfaceScope {
        self.scope
    }

    pub fn isolation(&self) -> InterfaceExtensionIsolation {
        self.isolation
    }

    pub fn facts(&self) -> &BTreeSet<InterfaceExtensionFact> {
        &self.facts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceHandlerCandidate {
    plugin: PluginIdentity,
    handler: HandlerReference,
    target: TargetReference,
}

impl InterfaceHandlerCandidate {
    pub fn new(plugin: PluginIdentity, handler: HandlerReference, target: TargetReference) -> Self {
        Self {
            plugin,
            handler,
            target,
        }
    }

    pub fn plugin(&self) -> &PluginIdentity {
        &self.plugin
    }

    pub fn handler(&self) -> &HandlerReference {
        &self.handler
    }

    pub fn target(&self) -> &TargetReference {
        &self.target
    }
}

pub fn compile_effective_handler(
    interface_id: &InterfaceId,
    candidates: impl IntoIterator<Item = InterfaceHandlerCandidate>,
) -> Result<InterfaceHandlerCandidate, InterfaceExtensionCompilationError> {
    let mut candidates = candidates.into_iter();
    let candidate = candidates.next().ok_or_else(|| {
        InterfaceExtensionCompilationError::MissingEffectiveHandler(interface_id.clone())
    })?;
    if candidates.next().is_some() {
        return Err(
            InterfaceExtensionCompilationError::MultipleEffectiveHandlers(interface_id.clone()),
        );
    }
    Ok(candidate)
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InterfaceExtensionCompilationError {
    #[error("extension tier {tier:?} cannot register {point:?}")]
    IllegalPoint {
        tier: InterfaceExtensionTier,
        point: InterfaceExtensionPoint,
    },
    #[error("extension point {point:?} requires permission {required:?}")]
    PermissionMismatch {
        point: InterfaceExtensionPoint,
        required: InterfaceExtensionPermission,
    },
    #[error("extension tier {tier:?} requires process/wire isolation")]
    IsolationMismatch { tier: InterfaceExtensionTier },
    #[error("extension point {point:?} requested facts outside its typed fact set")]
    IllegalFacts { point: InterfaceExtensionPoint },
    #[error("interface {0} has no effective handler")]
    MissingEffectiveHandler(InterfaceId),
    #[error("interface {0} has more than one effective handler")]
    MultipleEffectiveHandlers(InterfaceId),
    #[error("extension order {order} is duplicated for point {point:?}")]
    DuplicateOrder {
        order: u32,
        point: InterfaceExtensionPoint,
    },
}

fn validate_registration(
    tier: InterfaceExtensionTier,
    point: InterfaceExtensionPoint,
    permission: InterfaceExtensionPermission,
    isolation: InterfaceExtensionIsolation,
    facts: &BTreeSet<InterfaceExtensionFact>,
) -> Result<(), InterfaceExtensionCompilationError> {
    if point == InterfaceExtensionPoint::AuthenticationAdapter
        && !matches!(
            tier,
            InterfaceExtensionTier::BuiltIn | InterfaceExtensionTier::HostExtension
        )
    {
        return Err(InterfaceExtensionCompilationError::IllegalPoint { tier, point });
    }
    if matches!(
        tier,
        InterfaceExtensionTier::RuntimeExtension | InterfaceExtensionTier::CapabilityPlugin
    ) && isolation != InterfaceExtensionIsolation::ProcessWire
    {
        return Err(InterfaceExtensionCompilationError::IsolationMismatch { tier });
    }
    let required = required_permission(point, permission);
    if permission != required {
        return Err(InterfaceExtensionCompilationError::PermissionMismatch { point, required });
    }
    if !facts.is_subset(&allowed_facts(point)) {
        return Err(InterfaceExtensionCompilationError::IllegalFacts { point });
    }
    Ok(())
}

fn required_permission(
    point: InterfaceExtensionPoint,
    requested: InterfaceExtensionPermission,
) -> InterfaceExtensionPermission {
    match point {
        InterfaceExtensionPoint::Definition => InterfaceExtensionPermission::Define,
        InterfaceExtensionPoint::AuthenticationAdapter => {
            InterfaceExtensionPermission::Authenticate
        }
        InterfaceExtensionPoint::Authorization => InterfaceExtensionPermission::Authorize,
        InterfaceExtensionPoint::Admission => InterfaceExtensionPermission::Admit,
        InterfaceExtensionPoint::Before => {
            if requested == InterfaceExtensionPermission::MutateInput {
                InterfaceExtensionPermission::MutateInput
            } else {
                InterfaceExtensionPermission::ObserveInput
            }
        }
        InterfaceExtensionPoint::Handler => InterfaceExtensionPermission::Handle,
        InterfaceExtensionPoint::After => InterfaceExtensionPermission::ObserveOutput,
        InterfaceExtensionPoint::Failure => InterfaceExtensionPermission::ObserveFailure,
        InterfaceExtensionPoint::Completion => InterfaceExtensionPermission::ObserveCompletion,
    }
}

fn allowed_facts(point: InterfaceExtensionPoint) -> BTreeSet<InterfaceExtensionFact> {
    use InterfaceExtensionFact as Fact;
    let facts: &[Fact] = match point {
        InterfaceExtensionPoint::Definition => &[Fact::DefinitionIdentity, Fact::BindingIdentity],
        InterfaceExtensionPoint::AuthenticationAdapter => &[],
        InterfaceExtensionPoint::Authorization => {
            &[Fact::DefinitionIdentity, Fact::PrincipalSummary]
        }
        InterfaceExtensionPoint::Admission => &[
            Fact::DefinitionIdentity,
            Fact::PrincipalSummary,
            Fact::AuthorizationDecision,
        ],
        InterfaceExtensionPoint::Before => &[Fact::PrincipalSummary, Fact::TypedInput],
        InterfaceExtensionPoint::Handler => &[
            Fact::DefinitionIdentity,
            Fact::PrincipalSummary,
            Fact::TypedInput,
            Fact::InvocationIdentity,
            Fact::AttemptIdentity,
        ],
        InterfaceExtensionPoint::After => &[Fact::PrincipalSummary, Fact::TypedOutput],
        InterfaceExtensionPoint::Failure => &[
            Fact::FailureClassification,
            Fact::InvocationIdentity,
            Fact::AttemptIdentity,
        ],
        InterfaceExtensionPoint::Completion => &[Fact::Terminal, Fact::InvocationIdentity],
    };
    facts.iter().copied().collect()
}
