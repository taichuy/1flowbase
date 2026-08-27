use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use crate::{
    ContractIdentity, GraphFingerprint, HandlerReference, InterfaceId, InterfaceOwner,
    InvocationId, PermissionIdentity, RegistryFingerprint, RouteIdentity, TargetReference,
};
use domain::ActorContext;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub trait InterfaceContract: Send + Sync + 'static {
    const CONTRACT_ID: &'static str;
    const CONTRACT_VERSION: &'static str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceAuthenticationPolicy {
    Anonymous,
    Authenticated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceAuditPolicy {
    ReadOnly,
    Mutating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceErrorPolicy {
    TypedTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceScope {
    System,
    Workspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceLifecycle {
    BootSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceDefinition {
    interface_id: InterfaceId,
    input_contract: ContractIdentity,
    output_contract: ContractIdentity,
    route: Option<RouteIdentity>,
    permission: PermissionIdentity,
    authentication: InterfaceAuthenticationPolicy,
    audit: InterfaceAuditPolicy,
    error: InterfaceErrorPolicy,
    scope: InterfaceScope,
    lifecycle: InterfaceLifecycle,
    handler_reference: HandlerReference,
    target_reference: TargetReference,
    owner: InterfaceOwner,
}

impl InterfaceDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        interface_id: InterfaceId,
        input_contract: ContractIdentity,
        output_contract: ContractIdentity,
        route: Option<RouteIdentity>,
        permission: PermissionIdentity,
        authentication: InterfaceAuthenticationPolicy,
        audit: InterfaceAuditPolicy,
        error: InterfaceErrorPolicy,
        scope: InterfaceScope,
        lifecycle: InterfaceLifecycle,
        handler_reference: HandlerReference,
        target_reference: TargetReference,
        owner: InterfaceOwner,
    ) -> Self {
        Self {
            interface_id,
            input_contract,
            output_contract,
            route,
            permission,
            authentication,
            audit,
            error,
            scope,
            lifecycle,
            handler_reference,
            target_reference,
            owner,
        }
    }

    pub fn interface_id(&self) -> &InterfaceId {
        &self.interface_id
    }

    pub fn input_contract(&self) -> &ContractIdentity {
        &self.input_contract
    }

    pub fn output_contract(&self) -> &ContractIdentity {
        &self.output_contract
    }

    pub fn route(&self) -> Option<&RouteIdentity> {
        self.route.as_ref()
    }

    pub fn permission(&self) -> &PermissionIdentity {
        &self.permission
    }

    pub fn authentication(&self) -> InterfaceAuthenticationPolicy {
        self.authentication
    }

    pub fn audit(&self) -> InterfaceAuditPolicy {
        self.audit
    }

    pub fn error(&self) -> InterfaceErrorPolicy {
        self.error
    }

    pub fn scope(&self) -> InterfaceScope {
        self.scope
    }

    pub fn lifecycle(&self) -> InterfaceLifecycle {
        self.lifecycle
    }

    pub fn handler_reference(&self) -> &HandlerReference {
        &self.handler_reference
    }

    pub fn target_reference(&self) -> &TargetReference {
        &self.target_reference
    }

    pub fn owner(&self) -> &InterfaceOwner {
        &self.owner
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceHandlerContext {
    actor: ActorContext,
    invocation_id: InvocationId,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
}

impl InterfaceHandlerContext {
    pub(crate) fn new(
        actor: ActorContext,
        invocation_id: InvocationId,
        graph_fingerprint: GraphFingerprint,
        registry_fingerprint: RegistryFingerprint,
    ) -> Self {
        Self {
            actor,
            invocation_id,
            graph_fingerprint,
            registry_fingerprint,
        }
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
    }

    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }
}

#[derive(Debug, Error)]
#[error("interface target failed with {classification}")]
pub struct InterfaceTargetError {
    classification: Arc<str>,
    payload: Option<Box<dyn Any + Send + Sync>>,
}

impl InterfaceTargetError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: None,
        }
    }

    pub fn with_source<T>(classification: impl AsRef<str>, source: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: Some(Box::new(source)),
        }
    }

    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }

    pub fn into_source<T>(self) -> Option<T>
    where
        T: Send + Sync + 'static,
    {
        self.payload?.downcast::<T>().ok().map(|source| *source)
    }
}

pub type InterfaceHandlerFuture<O> =
    Pin<Box<dyn Future<Output = Result<O, InterfaceTargetError>> + Send + 'static>>;

pub trait InterfaceHandler<I, O>: Send + Sync + 'static
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    fn invoke(&self, context: InterfaceHandlerContext, input: I) -> InterfaceHandlerFuture<O>;
}

trait ErasedInterfaceBinding: Send + Sync {
    fn input_contract(&self) -> &ContractIdentity;
    fn output_contract(&self) -> &ContractIdentity;
    fn handler_reference(&self) -> &HandlerReference;
    fn as_any(&self) -> &dyn Any;
}

struct TypedInterfaceBinding<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    input_contract: ContractIdentity,
    output_contract: ContractIdentity,
    handler_reference: HandlerReference,
    handler: Arc<dyn InterfaceHandler<I, O>>,
}

impl<I, O> ErasedInterfaceBinding for TypedInterfaceBinding<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    fn input_contract(&self) -> &ContractIdentity {
        &self.input_contract
    }

    fn output_contract(&self) -> &ContractIdentity {
        &self.output_contract
    }

    fn handler_reference(&self) -> &HandlerReference {
        &self.handler_reference
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryCompilationError {
    #[error("duplicate interface identity {0}")]
    DuplicateInterface(InterfaceId),
    #[error("duplicate route {method} {path}")]
    DuplicateRoute { method: String, path: String },
    #[error("interface {0} has no bound handler")]
    MissingHandler(InterfaceId),
    #[error("handler is bound for unknown interface {0}")]
    UnknownInterface(InterfaceId),
    #[error("interface {0} uses unknown permission")]
    UnknownPermission(InterfaceId),
    #[error("interface {0} contract does not match its typed handler")]
    ContractMismatch(InterfaceId),
    #[error("interface {0} handler reference does not match its binding")]
    HandlerReferenceMismatch(InterfaceId),
    #[error("interface {0} already has a bound handler")]
    DuplicateHandler(InterfaceId),
}

pub struct RegistryCompiler {
    graph_fingerprint: GraphFingerprint,
    known_permissions: BTreeSet<PermissionIdentity>,
    definitions: BTreeMap<InterfaceId, InterfaceDefinition>,
    routes: BTreeMap<RouteIdentity, InterfaceId>,
    bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceBinding>>,
}

impl RegistryCompiler {
    pub fn new(
        graph_fingerprint: GraphFingerprint,
        known_permissions: impl IntoIterator<Item = PermissionIdentity>,
    ) -> Self {
        Self {
            graph_fingerprint,
            known_permissions: known_permissions.into_iter().collect(),
            definitions: BTreeMap::new(),
            routes: BTreeMap::new(),
            bindings: BTreeMap::new(),
        }
    }

    pub fn register_definition(
        &mut self,
        definition: InterfaceDefinition,
    ) -> Result<(), RegistryCompilationError> {
        if self.definitions.contains_key(definition.interface_id()) {
            return Err(RegistryCompilationError::DuplicateInterface(
                definition.interface_id().clone(),
            ));
        }
        if let Some(route) = definition.route() {
            if self.routes.contains_key(route) {
                return Err(RegistryCompilationError::DuplicateRoute {
                    method: route.method().to_string(),
                    path: route.path().to_string(),
                });
            }
            self.routes
                .insert(route.clone(), definition.interface_id().clone());
        }
        self.definitions
            .insert(definition.interface_id().clone(), definition);
        Ok(())
    }

    pub fn bind_handler<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        handler_reference: HandlerReference,
        handler: Arc<dyn InterfaceHandler<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.definitions.contains_key(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        let input_contract = contract_identity::<I>();
        let output_contract = contract_identity::<O>();
        self.bindings.insert(
            interface_id.clone(),
            Arc::new(TypedInterfaceBinding::<I, O> {
                input_contract,
                output_contract,
                handler_reference,
                handler,
            }),
        );
        Ok(())
    }

    pub fn compile(self) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
        for (interface_id, definition) in &self.definitions {
            if !self.known_permissions.contains(definition.permission()) {
                return Err(RegistryCompilationError::UnknownPermission(
                    interface_id.clone(),
                ));
            }
            let binding = self
                .bindings
                .get(interface_id)
                .ok_or_else(|| RegistryCompilationError::MissingHandler(interface_id.clone()))?;
            if binding.input_contract() != definition.input_contract()
                || binding.output_contract() != definition.output_contract()
            {
                return Err(RegistryCompilationError::ContractMismatch(
                    interface_id.clone(),
                ));
            }
            if binding.handler_reference() != definition.handler_reference() {
                return Err(RegistryCompilationError::HandlerReferenceMismatch(
                    interface_id.clone(),
                ));
            }
        }
        if let Some(interface_id) = self
            .bindings
            .keys()
            .find(|interface_id| !self.definitions.contains_key(*interface_id))
        {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        let fingerprint = registry_fingerprint(&self.graph_fingerprint, &self.definitions);
        Ok(Arc::new(CompiledInterfaceRegistry {
            graph_fingerprint: self.graph_fingerprint,
            fingerprint,
            definitions: self.definitions,
            routes: self.routes,
            bindings: self.bindings,
        }))
    }
}

pub struct CompiledInterfaceRegistry {
    graph_fingerprint: GraphFingerprint,
    fingerprint: RegistryFingerprint,
    definitions: BTreeMap<InterfaceId, InterfaceDefinition>,
    routes: BTreeMap<RouteIdentity, InterfaceId>,
    bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceBinding>>,
}

impl CompiledInterfaceRegistry {
    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &InterfaceDefinition> {
        self.definitions.values()
    }

    pub fn definition(&self, interface_id: &InterfaceId) -> Option<&InterfaceDefinition> {
        self.definitions.get(interface_id)
    }

    pub fn definition_by_route(&self, route: &RouteIdentity) -> Option<&InterfaceDefinition> {
        self.routes
            .get(route)
            .and_then(|interface_id| self.definitions.get(interface_id))
    }

    pub(crate) fn handler<I, O>(
        &self,
        interface_id: &InterfaceId,
    ) -> Option<Arc<dyn InterfaceHandler<I, O>>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.bindings
            .get(interface_id)?
            .as_any()
            .downcast_ref::<TypedInterfaceBinding<I, O>>()
            .map(|binding| Arc::clone(&binding.handler))
    }
}

pub struct DynamicInterfaceRegistry {
    current: RwLock<Arc<CompiledInterfaceRegistry>>,
}

impl DynamicInterfaceRegistry {
    pub fn new(initial: Arc<CompiledInterfaceRegistry>) -> Self {
        Self {
            current: RwLock::new(initial),
        }
    }

    pub fn snapshot(&self) -> Arc<CompiledInterfaceRegistry> {
        self.current
            .read()
            .expect("dynamic interface registry read lock must not be poisoned")
            .clone()
    }

    pub fn publish(&self, candidate: Arc<CompiledInterfaceRegistry>) {
        *self
            .current
            .write()
            .expect("dynamic interface registry write lock must not be poisoned") = candidate;
    }
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed interface contract constants must be valid identities")
}

fn registry_fingerprint(
    graph_fingerprint: &GraphFingerprint,
    definitions: &BTreeMap<InterfaceId, InterfaceDefinition>,
) -> RegistryFingerprint {
    let mut digest = Sha256::new();
    digest.update(graph_fingerprint.as_str().as_bytes());
    for definition in definitions.values() {
        for part in [
            definition.interface_id().as_str(),
            definition.input_contract().contract_id(),
            definition.input_contract().version(),
            definition.output_contract().contract_id(),
            definition.output_contract().version(),
            definition.permission().as_str(),
            definition.handler_reference().as_str(),
            definition.target_reference().as_str(),
            definition.owner().as_str(),
            match definition.authentication() {
                InterfaceAuthenticationPolicy::Anonymous => "authn:anonymous",
                InterfaceAuthenticationPolicy::Authenticated => "authn:authenticated",
            },
            match definition.audit() {
                InterfaceAuditPolicy::ReadOnly => "audit:read-only",
                InterfaceAuditPolicy::Mutating => "audit:mutating",
            },
            match definition.error() {
                InterfaceErrorPolicy::TypedTarget => "error:typed-target",
            },
            match definition.scope() {
                InterfaceScope::System => "scope:system",
                InterfaceScope::Workspace => "scope:workspace",
            },
            match definition.lifecycle() {
                InterfaceLifecycle::BootSnapshot => "lifecycle:boot-snapshot",
            },
        ] {
            digest.update([0]);
            digest.update(part.as_bytes());
        }
        if let Some(route) = definition.route() {
            digest.update([0]);
            digest.update(route.method().as_bytes());
            digest.update([0]);
            digest.update(route.path().as_bytes());
        }
    }
    RegistryFingerprint::new(format!("sha256:{:x}", digest.finalize()))
        .expect("SHA-256 registry fingerprint must be a valid identity")
}
