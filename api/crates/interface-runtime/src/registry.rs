use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use crate::contribution::ErasedDefinitionContribution;
use crate::decision::ErasedInterfaceDecisionPlan;
use crate::hook::ErasedInterfaceHookPlan;
use crate::{
    compile_effective_handler, ActivatedAuthenticationAdapter, AdmissionAdapterReference,
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingFingerprint, BindingId, CompiledInterfaceExtensionPlan, ContractIdentity,
    ExecutionAttempt, GraphFingerprint, HandlerReference, InterfaceExtensionPoint,
    InterfaceExtensionRegistration, InterfaceHandlerCandidate, InterfaceId, InterfaceOwner,
    InterfaceStreamHandler, InterfaceTargetFailure, InterfaceVersion, InvocationId,
    InvocationPrincipal, PlanFingerprint, PluginIdentity, PrincipalProfile, PrincipalSummary,
    RegistryFingerprint, RouteIdentity, TargetReference, TypedInterfaceAdmissionPlan,
    TypedInterfaceAuthorizationPlan, TypedInterfaceDefinitionContribution, TypedInterfaceHookPlan,
    TypedInterfaceStreamDefinitionContribution, UserPrincipal,
};
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceIdentity {
    interface_id: InterfaceId,
    version: InterfaceVersion,
}

impl InterfaceIdentity {
    pub fn new(interface_id: InterfaceId, version: InterfaceVersion) -> Self {
        Self {
            interface_id,
            version,
        }
    }

    pub fn interface_id(&self) -> &InterfaceId {
        &self.interface_id
    }

    pub fn version(&self) -> &InterfaceVersion {
        &self.version
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceContracts {
    input: ContractIdentity,
    result: InterfaceResultContracts,
}

impl InterfaceContracts {
    pub fn unary(
        input: ContractIdentity,
        output: ContractIdentity,
        target_error: ContractIdentity,
    ) -> Self {
        Self {
            input,
            result: InterfaceResultContracts::Unary {
                output,
                target_error,
            },
        }
    }

    pub fn server_stream(
        input: ContractIdentity,
        event: ContractIdentity,
        output: ContractIdentity,
        target_error: ContractIdentity,
    ) -> Self {
        Self {
            input,
            result: InterfaceResultContracts::ServerStream {
                output,
                event,
                target_error,
            },
        }
    }

    pub fn async_ack(
        input: ContractIdentity,
        ack: ContractIdentity,
        target_error: ContractIdentity,
    ) -> Self {
        Self {
            input,
            result: InterfaceResultContracts::AsyncAck { ack, target_error },
        }
    }

    pub fn input(&self) -> &ContractIdentity {
        &self.input
    }

    pub fn output(&self) -> &ContractIdentity {
        self.result.output()
    }

    pub fn stream_event(&self) -> Option<&ContractIdentity> {
        self.result.stream_event()
    }

    pub fn target_error(&self) -> &ContractIdentity {
        self.result.target_error()
    }

    pub fn mode(&self) -> InterfaceExecutionMode {
        self.result.mode()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceResultContracts {
    Unary {
        output: ContractIdentity,
        target_error: ContractIdentity,
    },
    ServerStream {
        output: ContractIdentity,
        event: ContractIdentity,
        target_error: ContractIdentity,
    },
    AsyncAck {
        ack: ContractIdentity,
        target_error: ContractIdentity,
    },
}

impl InterfaceResultContracts {
    fn output(&self) -> &ContractIdentity {
        match self {
            Self::Unary { output, .. } | Self::ServerStream { output, .. } => output,
            Self::AsyncAck { ack, .. } => ack,
        }
    }

    fn stream_event(&self) -> Option<&ContractIdentity> {
        match self {
            Self::ServerStream { event, .. } => Some(event),
            Self::Unary { .. } | Self::AsyncAck { .. } => None,
        }
    }

    fn target_error(&self) -> &ContractIdentity {
        match self {
            Self::Unary { target_error, .. }
            | Self::ServerStream { target_error, .. }
            | Self::AsyncAck { target_error, .. } => target_error,
        }
    }

    fn mode(&self) -> InterfaceExecutionMode {
        match self {
            Self::Unary { .. } => InterfaceExecutionMode::Unary,
            Self::ServerStream { .. } => InterfaceExecutionMode::ServerStream,
            Self::AsyncAck { .. } => InterfaceExecutionMode::AsyncAck,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterfaceExecutionMode {
    Unary,
    ServerStream,
    AsyncAck,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceAccess {
    principal_profile: PrincipalProfile,
    authentication: InterfaceAuthenticationPolicy,
    authorization_operation: AuthorizationOperation,
    scope: InterfaceScope,
}

impl InterfaceAccess {
    pub fn new(
        principal_profile: PrincipalProfile,
        authentication: InterfaceAuthenticationPolicy,
        authorization_operation: AuthorizationOperation,
        scope: InterfaceScope,
    ) -> Self {
        Self {
            principal_profile,
            authentication,
            authorization_operation,
            scope,
        }
    }

    pub fn principal_profile(&self) -> PrincipalProfile {
        self.principal_profile
    }

    pub fn authentication(&self) -> InterfaceAuthenticationPolicy {
        self.authentication
    }

    pub fn authorization_operation(&self) -> &AuthorizationOperation {
        &self.authorization_operation
    }

    pub fn scope(&self) -> InterfaceScope {
        self.scope
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceExecution {
    mode: InterfaceExecutionMode,
    handler_reference: HandlerReference,
    target_reference: TargetReference,
}

impl InterfaceExecution {
    pub fn new(
        mode: InterfaceExecutionMode,
        handler_reference: HandlerReference,
        target_reference: TargetReference,
    ) -> Self {
        Self {
            mode,
            handler_reference,
            target_reference,
        }
    }

    pub fn mode(&self) -> InterfaceExecutionMode {
        self.mode
    }

    pub fn handler_reference(&self) -> &HandlerReference {
        &self.handler_reference
    }

    pub fn target_reference(&self) -> &TargetReference {
        &self.target_reference
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceDefinition {
    identity: InterfaceIdentity,
    contracts: InterfaceContracts,
    access: InterfaceAccess,
    execution: InterfaceExecution,
    audit: InterfaceAuditPolicy,
    error: InterfaceErrorPolicy,
    lifecycle: InterfaceLifecycle,
    owner: InterfaceOwner,
}

impl InterfaceDefinition {
    #[expect(
        clippy::too_many_arguments,
        reason = "the stable definition constructor preserves the explicit compiled contract"
    )]
    pub fn new(
        identity: InterfaceIdentity,
        contracts: InterfaceContracts,
        access: InterfaceAccess,
        execution: InterfaceExecution,
        audit: InterfaceAuditPolicy,
        error: InterfaceErrorPolicy,
        lifecycle: InterfaceLifecycle,
        owner: InterfaceOwner,
    ) -> Self {
        Self {
            identity,
            contracts,
            access,
            execution,
            audit,
            error,
            lifecycle,
            owner,
        }
    }

    pub fn identity(&self) -> &InterfaceIdentity {
        &self.identity
    }

    pub fn interface_id(&self) -> &InterfaceId {
        self.identity.interface_id()
    }

    pub fn version(&self) -> &InterfaceVersion {
        self.identity.version()
    }

    pub fn input_contract(&self) -> &ContractIdentity {
        self.contracts.input()
    }

    pub fn contracts(&self) -> &InterfaceContracts {
        &self.contracts
    }

    pub fn output_contract(&self) -> &ContractIdentity {
        self.contracts.output()
    }

    pub fn stream_event_contract(&self) -> Option<&ContractIdentity> {
        self.contracts.stream_event()
    }

    pub fn target_error_contract(&self) -> &ContractIdentity {
        self.contracts.target_error()
    }

    pub fn execution_mode(&self) -> InterfaceExecutionMode {
        self.execution.mode()
    }

    pub fn authorization_operation(&self) -> &AuthorizationOperation {
        self.access.authorization_operation()
    }

    pub fn authentication(&self) -> InterfaceAuthenticationPolicy {
        self.access.authentication()
    }

    pub fn principal_profile(&self) -> PrincipalProfile {
        self.access.principal_profile()
    }

    pub fn audit(&self) -> InterfaceAuditPolicy {
        self.audit
    }

    pub fn error(&self) -> InterfaceErrorPolicy {
        self.error
    }

    pub fn scope(&self) -> InterfaceScope {
        self.access.scope()
    }

    pub fn lifecycle(&self) -> InterfaceLifecycle {
        self.lifecycle
    }

    pub fn handler_reference(&self) -> &HandlerReference {
        self.execution.handler_reference()
    }

    pub fn target_reference(&self) -> &TargetReference {
        self.execution.target_reference()
    }

    pub fn owner(&self) -> &InterfaceOwner {
        &self.owner
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolProjection {
    Http(RouteIdentity),
    HttpVariant {
        route: RouteIdentity,
        variant: Arc<str>,
    },
    Mcp {
        tool: Arc<str>,
    },
    Internal {
        operation: Arc<str>,
    },
    Worker {
        operation: Arc<str>,
    },
}

impl ProtocolProjection {
    pub fn http(route: RouteIdentity) -> Self {
        Self::Http(route)
    }

    pub fn mcp(tool: impl Into<Arc<str>>) -> Self {
        Self::Mcp { tool: tool.into() }
    }

    pub fn http_variant(route: RouteIdentity, variant: impl Into<Arc<str>>) -> Self {
        Self::HttpVariant {
            route,
            variant: variant.into(),
        }
    }

    pub fn internal(operation: impl Into<Arc<str>>) -> Self {
        Self::Internal {
            operation: operation.into(),
        }
    }

    pub fn worker(operation: impl Into<Arc<str>>) -> Self {
        Self::Worker {
            operation: operation.into(),
        }
    }

    pub fn http_route(&self) -> Option<&RouteIdentity> {
        match self {
            Self::Http(route) | Self::HttpVariant { route, .. } => Some(route),
            Self::Mcp { .. } | Self::Internal { .. } | Self::Worker { .. } => None,
        }
    }

    pub fn protocol(&self) -> crate::InterfaceProtocol {
        match self {
            Self::Http(_) | Self::HttpVariant { .. } => crate::InterfaceProtocol::Http,
            Self::Mcp { .. } => crate::InterfaceProtocol::Mcp,
            Self::Internal { .. } => crate::InterfaceProtocol::Internal,
            Self::Worker { .. } => crate::InterfaceProtocol::Worker,
        }
    }

    fn fingerprint_parts(&self) -> (&'static str, &str, Option<&str>, Option<&str>) {
        match self {
            Self::Http(route) => ("http", route.method(), Some(route.path()), None),
            Self::HttpVariant { route, variant } => (
                "http-variant",
                route.method(),
                Some(route.path()),
                Some(variant),
            ),
            Self::Mcp { tool } => ("mcp", tool, None, None),
            Self::Internal { operation } => ("internal", operation, None, None),
            Self::Worker { operation } => ("worker", operation, None, None),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolBinding {
    binding_id: BindingId,
    interface_identity: InterfaceIdentity,
    contracts: InterfaceContracts,
    projection: ProtocolProjection,
}

impl ProtocolBinding {
    pub fn new(
        binding_id: BindingId,
        interface_identity: InterfaceIdentity,
        contracts: InterfaceContracts,
        projection: ProtocolProjection,
    ) -> Self {
        Self {
            binding_id,
            interface_identity,
            contracts,
            projection,
        }
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn interface_identity(&self) -> &InterfaceIdentity {
        &self.interface_identity
    }

    pub fn input_contract(&self) -> &ContractIdentity {
        self.contracts.input()
    }

    pub fn output_contract(&self) -> &ContractIdentity {
        self.contracts.output()
    }

    pub fn contracts(&self) -> &InterfaceContracts {
        &self.contracts
    }

    pub fn projection(&self) -> &ProtocolProjection {
        &self.projection
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationAdapterPlan {
    authentication: AuthenticationAdapterReference,
    authorization: AuthorizationAdapterReference,
    admission: Option<AdmissionAdapterReference>,
}

impl InvocationAdapterPlan {
    pub fn new(
        authentication: AuthenticationAdapterReference,
        authorization: AuthorizationAdapterReference,
        admission: Option<AdmissionAdapterReference>,
    ) -> Self {
        Self {
            authentication,
            authorization,
            admission,
        }
    }

    pub fn authentication(&self) -> &AuthenticationAdapterReference {
        &self.authentication
    }

    pub fn authorization(&self) -> &AuthorizationAdapterReference {
        &self.authorization
    }

    pub fn admission(&self) -> Option<&AdmissionAdapterReference> {
        self.admission.as_ref()
    }
}

#[derive(Clone)]
pub struct CompiledInvocationPlan {
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    binding_fingerprint: BindingFingerprint,
    adapter_plan: InvocationAdapterPlan,
    authentication: ActivatedAuthenticationAdapter,
    extension_plan: CompiledInterfaceExtensionPlan,
    executable_extensions: Option<Arc<dyn ErasedInterfaceHookPlan>>,
    authorization_extensions: Option<Arc<dyn ErasedInterfaceDecisionPlan>>,
    admission_extensions: Option<Arc<dyn ErasedInterfaceDecisionPlan>>,
    effective_handler: InterfaceHandlerCandidate,
    fingerprint: PlanFingerprint,
}

impl std::fmt::Debug for CompiledInvocationPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledInvocationPlan")
            .field("definition", &self.definition)
            .field("binding", &self.binding)
            .field("binding_fingerprint", &self.binding_fingerprint)
            .field("adapter_plan", &self.adapter_plan)
            .field("authentication", &self.authentication)
            .field("extension_plan", &self.extension_plan)
            .field("effective_handler", &self.effective_handler)
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl CompiledInvocationPlan {
    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }

    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }

    pub fn binding_fingerprint(&self) -> &BindingFingerprint {
        &self.binding_fingerprint
    }

    pub fn adapter_plan(&self) -> &InvocationAdapterPlan {
        &self.adapter_plan
    }

    pub fn authentication(&self) -> &ActivatedAuthenticationAdapter {
        &self.authentication
    }

    pub fn extension_plan(&self) -> &CompiledInterfaceExtensionPlan {
        &self.extension_plan
    }

    pub fn effective_handler(&self) -> &InterfaceHandlerCandidate {
        &self.effective_handler
    }

    pub fn has_executable_extensions(&self) -> bool {
        self.executable_extensions.is_some()
    }

    pub(crate) fn hook_plan<I, O>(&self) -> Option<&TypedInterfaceHookPlan<I, O>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.executable_extensions
            .as_ref()?
            .as_any()
            .downcast_ref::<TypedInterfaceHookPlan<I, O>>()
    }

    pub(crate) fn authorization_plan<I, O>(&self) -> Option<&TypedInterfaceAuthorizationPlan<I, O>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.authorization_extensions
            .as_ref()?
            .as_any()
            .downcast_ref()
    }

    pub(crate) fn admission_plan<I, O>(&self) -> Option<&TypedInterfaceAdmissionPlan<I, O>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.admission_extensions.as_ref()?.as_any().downcast_ref()
    }

    pub fn fingerprint(&self) -> &PlanFingerprint {
        &self.fingerprint
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceHandlerContext<P = UserPrincipal>
where
    P: InvocationPrincipal,
{
    principal: P,
    principal_summary: PrincipalSummary,
    attempt: ExecutionAttempt,
    invocation_id: InvocationId,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
}

impl<P> InterfaceHandlerContext<P>
where
    P: InvocationPrincipal,
{
    pub(crate) fn new(
        principal: P,
        attempt: ExecutionAttempt,
        invocation_id: InvocationId,
        graph_fingerprint: GraphFingerprint,
        registry_fingerprint: RegistryFingerprint,
    ) -> Self {
        Self {
            principal_summary: principal.summary(),
            principal,
            attempt,
            invocation_id,
            graph_fingerprint,
            registry_fingerprint,
        }
    }

    pub fn principal(&self) -> &P {
        &self.principal
    }

    pub fn principal_summary(&self) -> &PrincipalSummary {
        &self.principal_summary
    }

    pub fn attempt(&self) -> &ExecutionAttempt {
        &self.attempt
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

pub type InterfaceHandlerFuture<O, E> =
    Pin<Box<dyn Future<Output = Result<O, InterfaceTargetFailure<E>>> + Send + 'static>>;

pub trait InterfaceHandler<I, O, E, P = UserPrincipal>: Send + Sync + 'static
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn invoke(&self, context: InterfaceHandlerContext<P>, input: I)
        -> InterfaceHandlerFuture<O, E>;
}

trait ErasedInterfaceBinding: Send + Sync {
    fn contracts(&self) -> &InterfaceContracts;
    fn handler_reference(&self) -> &HandlerReference;
    fn principal_profile(&self) -> PrincipalProfile;
    fn as_any(&self) -> &dyn Any;
}

struct TypedInterfaceBinding<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    contracts: InterfaceContracts,
    handler_reference: HandlerReference,
    handler: Arc<dyn InterfaceHandler<I, O, E, P>>,
}

struct TypedInterfaceStreamBinding<I, S, O, E, P>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    contracts: InterfaceContracts,
    handler_reference: HandlerReference,
    handler: Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>,
}

#[derive(Clone)]
struct ContributedInterfaceHandlerBinding {
    plugin: PluginIdentity,
    target: TargetReference,
    binding: Arc<dyn ErasedInterfaceBinding>,
}

impl<I, S, O, E, P> ErasedInterfaceBinding for TypedInterfaceStreamBinding<I, S, O, E, P>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn contracts(&self) -> &InterfaceContracts {
        &self.contracts
    }

    fn handler_reference(&self) -> &HandlerReference {
        &self.handler_reference
    }

    fn principal_profile(&self) -> PrincipalProfile {
        P::PROFILE
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<I, O, E, P> ErasedInterfaceBinding for TypedInterfaceBinding<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn contracts(&self) -> &InterfaceContracts {
        &self.contracts
    }

    fn handler_reference(&self) -> &HandlerReference {
        &self.handler_reference
    }

    fn principal_profile(&self) -> PrincipalProfile {
        P::PROFILE
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryCompilationError {
    #[error("duplicate interface identity {0}")]
    DuplicateInterface(InterfaceId),
    #[error("duplicate binding identity {0}")]
    DuplicateBinding(BindingId),
    #[error("duplicate protocol projection {0}")]
    DuplicateProjection(BindingId),
    #[error("interface {0} has no bound handler")]
    MissingHandler(InterfaceId),
    #[error("handler is bound for unknown interface {0}")]
    UnknownInterface(InterfaceId),
    #[error("interface {0} uses unknown authorization operation")]
    UnknownAuthorizationOperation(InterfaceId),
    #[error("interface {0} owner is inactive")]
    InactiveOwner(InterfaceId),
    #[error("binding {0} references an unknown interface")]
    BindingUnknownInterface(BindingId),
    #[error("binding {0} interface version does not match its definition")]
    BindingVersionMismatch(BindingId),
    #[error("binding {0} contract does not match its definition")]
    BindingContractMismatch(BindingId),
    #[error("interface {0} has no protocol binding")]
    MissingBinding(InterfaceId),
    #[error("interface {0} contract does not match its typed handler")]
    ContractMismatch(InterfaceId),
    #[error("interface {0} execution mode does not match its result contracts")]
    ExecutionModeMismatch(InterfaceId),
    #[error("interface {0} handler reference does not match its binding")]
    HandlerReferenceMismatch(InterfaceId),
    #[error("interface {0} principal profile does not match its typed handler")]
    PrincipalProfileMismatch(InterfaceId),
    #[error("interface {0} already has a bound handler")]
    DuplicateHandler(InterfaceId),
    #[error("interface {0} already has a bound executable hook plan")]
    DuplicateHookPlan(InterfaceId),
    #[error("interface {0} extension {1} point {2:?} has no executable binding")]
    MissingExecutableExtension(InterfaceId, PluginIdentity, InterfaceExtensionPoint),
    #[error("interface {0} extension {1} point {2:?} has an unregistered executable binding")]
    UnexpectedExecutableExtension(InterfaceId, PluginIdentity, InterfaceExtensionPoint),
    #[error("interface {0} extension handler {1} is bound more than once")]
    DuplicateExtensionHandler(InterfaceId, PluginIdentity),
    #[error("interface {0} executable extension graph fingerprint does not match the compiler")]
    ExtensionGraphFingerprintMismatch(InterfaceId),
    #[error("interface {0} executable hook input/output contract does not match its definition")]
    HookContractMismatch(InterfaceId),
    #[error("interface {0} executable {1:?} decision contract does not match its definition")]
    DecisionContractMismatch(InterfaceId, InterfaceExtensionPoint),
    #[error("interface {0} definition registration has no typed contribution binding")]
    MissingDefinitionContribution(InterfaceId),
    #[error("interface {0} has an unregistered typed definition contribution binding")]
    UnexpectedDefinitionContribution(InterfaceId),
    #[error("interface {0} authentication registration has no activated adapter binding")]
    MissingAuthenticationActivation(InterfaceId),
    #[error("interface {0} has an unregistered authentication activation binding")]
    UnexpectedAuthenticationActivation(InterfaceId),
    #[error("interface {0} authentication adapter is activated more than once")]
    DuplicateAuthenticationActivation(InterfaceId),
    #[error("interface {0} authentication activation identity does not match its registration or binding")]
    AuthenticationActivationMismatch(InterfaceId),
    #[error(transparent)]
    Extension(#[from] crate::InterfaceExtensionCompilationError),
}

pub struct RegistryCompiler {
    graph_fingerprint: GraphFingerprint,
    known_operations: BTreeSet<AuthorizationOperation>,
    active_owners: BTreeSet<InterfaceOwner>,
    definition_contributions: Vec<(
        u32,
        InterfaceExtensionRegistration,
        Arc<dyn ErasedDefinitionContribution>,
    )>,
    compiled_definition_contributions: BTreeMap<InterfaceId, PluginIdentity>,
    definitions: BTreeMap<InterfaceId, InterfaceDefinition>,
    protocol_bindings: BTreeMap<BindingId, (ProtocolBinding, InvocationAdapterPlan)>,
    routes: BTreeMap<RouteIdentity, BindingId>,
    handler_bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceBinding>>,
    extension_handler_bindings:
        BTreeMap<InterfaceId, BTreeMap<PluginIdentity, ContributedInterfaceHandlerBinding>>,
    hook_bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceHookPlan>>,
    authorization_bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceDecisionPlan>>,
    admission_bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceDecisionPlan>>,
    extensions: BTreeMap<InterfaceId, Vec<(u32, InterfaceExtensionRegistration)>>,
    authentication_activations:
        BTreeMap<(InterfaceId, AuthenticationAdapterReference), ActivatedAuthenticationAdapter>,
}

impl RegistryCompiler {
    fn has_interface_input(&self, interface_id: &InterfaceId) -> bool {
        self.definitions.contains_key(interface_id)
            || self
                .definition_contributions
                .iter()
                .any(|(_, _, binding)| binding.definition().interface_id() == interface_id)
    }

    pub fn new(
        graph_fingerprint: GraphFingerprint,
        known_operations: impl IntoIterator<Item = AuthorizationOperation>,
        active_owners: impl IntoIterator<Item = InterfaceOwner>,
    ) -> Self {
        Self {
            graph_fingerprint,
            known_operations: known_operations.into_iter().collect(),
            active_owners: active_owners.into_iter().collect(),
            definition_contributions: Vec::new(),
            compiled_definition_contributions: BTreeMap::new(),
            definitions: BTreeMap::new(),
            protocol_bindings: BTreeMap::new(),
            routes: BTreeMap::new(),
            handler_bindings: BTreeMap::new(),
            extension_handler_bindings: BTreeMap::new(),
            hook_bindings: BTreeMap::new(),
            authorization_bindings: BTreeMap::new(),
            admission_bindings: BTreeMap::new(),
            extensions: BTreeMap::new(),
            authentication_activations: BTreeMap::new(),
        }
    }

    pub fn register_definition_contribution<I, O, E, P>(
        &mut self,
        order: u32,
        registration: InterfaceExtensionRegistration,
        contribution: Arc<TypedInterfaceDefinitionContribution<I, O, E, P>>,
    ) where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.definition_contributions
            .push((order, registration, contribution));
    }

    pub fn register_stream_definition_contribution<I, S, O, E, P>(
        &mut self,
        order: u32,
        registration: InterfaceExtensionRegistration,
        contribution: Arc<TypedInterfaceStreamDefinitionContribution<I, S, O, E, P>>,
    ) where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.definition_contributions
            .push((order, registration, contribution));
    }

    pub fn bind_authentication_activation(
        &mut self,
        interface_id: InterfaceId,
        activation: ActivatedAuthenticationAdapter,
    ) -> Result<(), RegistryCompilationError> {
        let key = (interface_id.clone(), activation.adapter().clone());
        if self.authentication_activations.contains_key(&key) {
            return Err(RegistryCompilationError::DuplicateAuthenticationActivation(
                interface_id,
            ));
        }
        self.authentication_activations.insert(key, activation);
        Ok(())
    }

    pub fn register_authentication_adapter(
        &mut self,
        interface_id: &InterfaceId,
        order: u32,
        registration: InterfaceExtensionRegistration,
        activation: ActivatedAuthenticationAdapter,
    ) -> Result<(), RegistryCompilationError> {
        self.register_extension(interface_id, order, registration)?;
        self.bind_authentication_activation(interface_id.clone(), activation)
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
        self.definitions
            .insert(definition.interface_id().clone(), definition);
        Ok(())
    }

    pub fn absorb_snapshot(
        &mut self,
        snapshot: &CompiledInterfaceRegistry,
    ) -> Result<(), RegistryCompilationError> {
        for interface_id in snapshot.definitions.keys() {
            self.absorb_interface(snapshot, interface_id)?;
        }
        Ok(())
    }

    pub fn absorb_interface(
        &mut self,
        snapshot: &CompiledInterfaceRegistry,
        interface_id: &InterfaceId,
    ) -> Result<(), RegistryCompilationError> {
        let definition = snapshot
            .definitions
            .get(interface_id)
            .ok_or_else(|| RegistryCompilationError::UnknownInterface(interface_id.clone()))?;
        self.register_definition(definition.clone())?;
        if let Some(plugin) = snapshot.definition_contributions.get(interface_id) {
            self.compiled_definition_contributions
                .insert(interface_id.clone(), plugin.clone());
        }
        for (binding_id, binding) in snapshot
            .protocol_bindings
            .iter()
            .filter(|(_, binding)| binding.interface_identity().interface_id() == interface_id)
        {
            let adapter_plan = snapshot
                .plans
                .get(binding_id)
                .expect("compiled binding must own an invocation plan")
                .adapter_plan()
                .clone();
            self.register_binding(binding.clone(), adapter_plan)?;
            let authentication = snapshot
                .plans
                .get(binding_id)
                .expect("compiled binding must own an invocation plan")
                .authentication()
                .clone();
            self.authentication_activations.insert(
                (interface_id.clone(), authentication.adapter().clone()),
                authentication,
            );
        }
        let plan = snapshot
            .plan_for_interface(interface_id)
            .ok_or_else(|| RegistryCompilationError::MissingBinding(interface_id.clone()))?;
        let handler = snapshot
            .handler_bindings
            .get(interface_id)
            .ok_or_else(|| RegistryCompilationError::MissingHandler(interface_id.clone()))?;
        if self
            .handler_bindings
            .insert(interface_id.clone(), Arc::clone(handler))
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        if let Some(hooks) = &plan.executable_extensions {
            self.hook_bindings
                .insert(interface_id.clone(), Arc::clone(hooks));
        }
        if let Some(authorization) = &plan.authorization_extensions {
            self.authorization_bindings
                .insert(interface_id.clone(), Arc::clone(authorization));
        }
        if let Some(admission) = &plan.admission_extensions {
            self.admission_bindings
                .insert(interface_id.clone(), Arc::clone(admission));
        }
        if plan.effective_handler().plugin().as_str() != "builtin.interface-handler" {
            self.extension_handler_bindings
                .entry(interface_id.clone())
                .or_default()
                .insert(
                    plan.effective_handler().plugin().clone(),
                    ContributedInterfaceHandlerBinding {
                        plugin: plan.effective_handler().plugin().clone(),
                        target: plan.effective_handler().target().clone(),
                        binding: Arc::clone(handler),
                    },
                );
        }
        for entry in plan.extension_plan().registrations() {
            self.register_extension(interface_id, entry.order(), entry.registration().clone())?;
        }
        Ok(())
    }

    pub fn register_binding(
        &mut self,
        binding: ProtocolBinding,
        adapter_plan: InvocationAdapterPlan,
    ) -> Result<(), RegistryCompilationError> {
        if self.protocol_bindings.contains_key(binding.binding_id()) {
            return Err(RegistryCompilationError::DuplicateBinding(
                binding.binding_id().clone(),
            ));
        }
        if let ProtocolProjection::Http(route) = binding.projection() {
            if let Some(existing) = self.routes.get(route) {
                return Err(RegistryCompilationError::DuplicateProjection(
                    existing.clone(),
                ));
            }
            self.routes
                .insert(route.clone(), binding.binding_id().clone());
        }
        self.protocol_bindings
            .insert(binding.binding_id().clone(), (binding, adapter_plan));
        Ok(())
    }

    pub fn register_extension(
        &mut self,
        interface_id: &InterfaceId,
        order: u32,
        registration: InterfaceExtensionRegistration,
    ) -> Result<(), RegistryCompilationError> {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        self.extensions
            .entry(interface_id.clone())
            .or_default()
            .push((order, registration));
        Ok(())
    }

    pub fn bind_hook_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        hooks: Arc<TypedInterfaceHookPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.hook_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        self.hook_bindings.insert(interface_id.clone(), hooks);
        Ok(())
    }

    pub fn bind_authorization_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        plan: Arc<TypedInterfaceAuthorizationPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self
            .authorization_bindings
            .insert(interface_id.clone(), plan)
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        Ok(())
    }

    pub fn bind_admission_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        plan: Arc<TypedInterfaceAdmissionPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self
            .admission_bindings
            .insert(interface_id.clone(), plan)
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        Ok(())
    }

    pub fn bind_extension_handler<I, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        handler_reference: HandlerReference,
        target: TargetReference,
        handler: Arc<dyn InterfaceHandler<I, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        let binding: Arc<dyn ErasedInterfaceBinding> =
            Arc::new(TypedInterfaceBinding::<I, O, E, P> {
                contracts: InterfaceContracts::unary(
                    contract_identity::<I>(),
                    contract_identity::<O>(),
                    contract_identity::<E>(),
                ),
                handler_reference,
                handler,
            });
        self.insert_extension_handler(interface_id, plugin, target, binding)
    }

    pub fn bind_extension_stream_handler<I, S, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        handler_reference: HandlerReference,
        target: TargetReference,
        handler: Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        let binding: Arc<dyn ErasedInterfaceBinding> =
            Arc::new(TypedInterfaceStreamBinding::<I, S, O, E, P> {
                contracts: InterfaceContracts::server_stream(
                    contract_identity::<I>(),
                    contract_identity::<S>(),
                    contract_identity::<O>(),
                    contract_identity::<E>(),
                ),
                handler_reference,
                handler,
            });
        self.insert_extension_handler(interface_id, plugin, target, binding)
    }

    fn insert_extension_handler(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        target: TargetReference,
        binding: Arc<dyn ErasedInterfaceBinding>,
    ) -> Result<(), RegistryCompilationError> {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        let bindings = self
            .extension_handler_bindings
            .entry(interface_id.clone())
            .or_default();
        if bindings.contains_key(&plugin) {
            return Err(RegistryCompilationError::DuplicateExtensionHandler(
                interface_id.clone(),
                plugin,
            ));
        }
        bindings.insert(
            plugin.clone(),
            ContributedInterfaceHandlerBinding {
                plugin,
                target,
                binding,
            },
        );
        Ok(())
    }

    pub fn bind_handler<I, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        handler_reference: HandlerReference,
        handler: Arc<dyn InterfaceHandler<I, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.handler_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        let contracts = InterfaceContracts::unary(
            contract_identity::<I>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        self.handler_bindings.insert(
            interface_id.clone(),
            Arc::new(TypedInterfaceBinding::<I, O, E, P> {
                contracts,
                handler_reference,
                handler,
            }),
        );
        Ok(())
    }

    pub fn bind_stream_handler<I, S, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        handler_reference: HandlerReference,
        handler: Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.handler_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        let contracts = InterfaceContracts::server_stream(
            contract_identity::<I>(),
            contract_identity::<S>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        self.handler_bindings.insert(
            interface_id.clone(),
            Arc::new(TypedInterfaceStreamBinding::<I, S, O, E, P> {
                contracts,
                handler_reference,
                handler,
            }),
        );
        Ok(())
    }

    pub fn compile(mut self) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
        for (order, registration, contribution) in
            std::mem::take(&mut self.definition_contributions)
        {
            let definition = contribution.definition().clone();
            let interface_id = definition.interface_id().clone();
            if registration.point() != InterfaceExtensionPoint::Definition {
                return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                    interface_id,
                    registration.plugin().clone(),
                    registration.point(),
                ));
            }
            self.register_definition(definition)?;
            for binding in contribution.bindings() {
                self.register_binding(binding.binding().clone(), binding.adapter_plan().clone())?;
            }
            self.compiled_definition_contributions
                .insert(interface_id.clone(), registration.plugin().clone());
            self.extensions
                .entry(interface_id)
                .or_default()
                .push((order, registration));
        }
        for (interface_id, definition) in &self.definitions {
            if !self
                .known_operations
                .contains(definition.authorization_operation())
            {
                return Err(RegistryCompilationError::UnknownAuthorizationOperation(
                    interface_id.clone(),
                ));
            }
            if !self.active_owners.contains(definition.owner()) {
                return Err(RegistryCompilationError::InactiveOwner(
                    interface_id.clone(),
                ));
            }
            if definition.execution_mode() != definition.contracts.mode() {
                return Err(RegistryCompilationError::ExecutionModeMismatch(
                    interface_id.clone(),
                ));
            }
            let has_contributed_handler =
                self.extensions
                    .get(interface_id)
                    .is_some_and(|registrations| {
                        registrations.iter().any(|(_, registration)| {
                            registration.point() == InterfaceExtensionPoint::Handler
                        })
                    });
            if !has_contributed_handler {
                let binding = self.handler_bindings.get(interface_id).ok_or_else(|| {
                    RegistryCompilationError::MissingHandler(interface_id.clone())
                })?;
                if binding.contracts() != definition.contracts() {
                    return Err(RegistryCompilationError::ContractMismatch(
                        interface_id.clone(),
                    ));
                }
                if binding.handler_reference() != definition.handler_reference() {
                    return Err(RegistryCompilationError::HandlerReferenceMismatch(
                        interface_id.clone(),
                    ));
                }
                if binding.principal_profile() != definition.principal_profile() {
                    return Err(RegistryCompilationError::PrincipalProfileMismatch(
                        interface_id.clone(),
                    ));
                }
            }
            if !self
                .protocol_bindings
                .values()
                .any(|(binding, _)| binding.interface_identity().interface_id() == interface_id)
            {
                return Err(RegistryCompilationError::MissingBinding(
                    interface_id.clone(),
                ));
            }
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let definition_registration = registrations
                .iter()
                .find(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::Definition
                })
                .map(|(_, registration)| registration);
            match (
                definition_registration,
                self.compiled_definition_contributions.get(interface_id),
            ) {
                (Some(registration), Some(plugin)) if registration.plugin() == plugin => {}
                (Some(_), None) => {
                    return Err(RegistryCompilationError::MissingDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (None, Some(_)) => {
                    return Err(RegistryCompilationError::UnexpectedDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(RegistryCompilationError::UnexpectedDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (None, None) => {}
            }
            let authentication_registrations = registrations
                .iter()
                .filter(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::AuthenticationAdapter
                })
                .map(|(_, registration)| registration)
                .collect::<Vec<_>>();
            if authentication_registrations.len() != 1 {
                return Err(RegistryCompilationError::MissingAuthenticationActivation(
                    interface_id.clone(),
                ));
            }
            let registration = authentication_registrations[0];
            let referenced_adapters = self
                .protocol_bindings
                .values()
                .filter(|(binding, _)| binding.interface_identity().interface_id() == interface_id)
                .map(|(_, plan)| plan.authentication())
                .collect::<BTreeSet<_>>();
            for adapter in &referenced_adapters {
                let activation = self
                    .authentication_activations
                    .get(&(interface_id.clone(), (*adapter).clone()))
                    .ok_or_else(|| {
                        RegistryCompilationError::MissingAuthenticationActivation(
                            interface_id.clone(),
                        )
                    })?;
                if activation.plugin() != registration.plugin()
                    || activation.tier() != registration.tier()
                    || activation.principal_profile() != definition.principal_profile()
                {
                    return Err(RegistryCompilationError::AuthenticationActivationMismatch(
                        interface_id.clone(),
                    ));
                }
            }
            if self
                .authentication_activations
                .keys()
                .any(|(candidate, adapter)| {
                    candidate == interface_id && !referenced_adapters.contains(adapter)
                })
            {
                return Err(
                    RegistryCompilationError::UnexpectedAuthenticationActivation(
                        interface_id.clone(),
                    ),
                );
            }
        }
        if let Some(interface_id) = self
            .handler_bindings
            .keys()
            .find(|interface_id| !self.definitions.contains_key(*interface_id))
        {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        let mut effective_handler_bindings = BTreeMap::new();
        let mut effective_handlers = BTreeMap::new();
        for (interface_id, definition) in &self.definitions {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let handler_plugins = registrations
                .iter()
                .filter(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::Handler
                })
                .map(|(_, registration)| registration.plugin().clone())
                .collect::<Vec<_>>();
            let contributed = self.extension_handler_bindings.get(interface_id);
            let (effective, binding) = if handler_plugins.is_empty() {
                if let Some((plugin, _)) = contributed.and_then(|bindings| bindings.iter().next()) {
                    return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                        interface_id.clone(),
                        plugin.clone(),
                        InterfaceExtensionPoint::Handler,
                    ));
                }
                let binding = self
                    .handler_bindings
                    .get(interface_id)
                    .expect("definition handler was validated above");
                (
                    InterfaceHandlerCandidate::new(
                        PluginIdentity::new("builtin.interface-handler")
                            .expect("built-in handler identity must be valid"),
                        binding.handler_reference().clone(),
                        definition.target_reference().clone(),
                    ),
                    Arc::clone(binding),
                )
            } else {
                let mut candidates = Vec::with_capacity(handler_plugins.len());
                for plugin in &handler_plugins {
                    let executable = contributed
                        .and_then(|bindings| bindings.get(plugin))
                        .ok_or_else(|| {
                            RegistryCompilationError::MissingExecutableExtension(
                                interface_id.clone(),
                                plugin.clone(),
                                InterfaceExtensionPoint::Handler,
                            )
                        })?;
                    if executable.binding.contracts() != definition.contracts() {
                        return Err(RegistryCompilationError::ContractMismatch(
                            interface_id.clone(),
                        ));
                    }
                    if executable.binding.principal_profile() != definition.principal_profile() {
                        return Err(RegistryCompilationError::PrincipalProfileMismatch(
                            interface_id.clone(),
                        ));
                    }
                    candidates.push(InterfaceHandlerCandidate::new(
                        executable.plugin.clone(),
                        executable.binding.handler_reference().clone(),
                        executable.target.clone(),
                    ));
                }
                if let Some((plugin, _)) = contributed.and_then(|bindings| {
                    bindings
                        .iter()
                        .find(|(plugin, _)| !handler_plugins.contains(plugin))
                }) {
                    return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                        interface_id.clone(),
                        plugin.clone(),
                        InterfaceExtensionPoint::Handler,
                    ));
                }
                let effective = compile_effective_handler(interface_id, candidates)?;
                let executable = contributed
                    .and_then(|bindings| bindings.get(effective.plugin()))
                    .expect("effective contributed handler was validated above");
                (effective, Arc::clone(&executable.binding))
            };
            effective_handlers.insert(interface_id.clone(), effective);
            effective_handler_bindings.insert(interface_id.clone(), binding);
        }

        for interface_id in self.definitions.keys() {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let hooks = self.hook_bindings.get(interface_id);
            if let Some(hooks) = hooks {
                if hooks.graph_fingerprint() != &self.graph_fingerprint {
                    return Err(RegistryCompilationError::ExtensionGraphFingerprintMismatch(
                        interface_id.clone(),
                    ));
                }
                let definition = self
                    .definitions
                    .get(interface_id)
                    .expect("hook binding interface definition was registered");
                if hooks.input_contract() != definition.input_contract()
                    || hooks.output_contract() != definition.output_contract()
                {
                    return Err(RegistryCompilationError::HookContractMismatch(
                        interface_id.clone(),
                    ));
                }
                for point in [
                    InterfaceExtensionPoint::Before,
                    InterfaceExtensionPoint::After,
                    InterfaceExtensionPoint::Failure,
                    InterfaceExtensionPoint::Completion,
                ] {
                    let mut registered = registrations
                        .iter()
                        .filter(|(_, registration)| registration.point() == point)
                        .map(|(order, registration)| (*order, registration.plugin().clone()))
                        .collect::<Vec<_>>();
                    registered.sort();
                    let registered = registered
                        .into_iter()
                        .map(|(_, plugin)| plugin)
                        .collect::<Vec<_>>();
                    let bound = hooks
                        .bindings()
                        .iter()
                        .filter(|(_, bound_point)| *bound_point == point)
                        .map(|(plugin, _)| plugin.clone())
                        .collect::<Vec<_>>();
                    if let Some(plugin) = registered
                        .iter()
                        .find(|plugin| !bound.contains(plugin))
                        .or_else(|| registered.get(bound.len()))
                    {
                        return Err(RegistryCompilationError::MissingExecutableExtension(
                            interface_id.clone(),
                            plugin.clone(),
                            point,
                        ));
                    }
                    if let Some(plugin) = bound
                        .iter()
                        .find(|plugin| !registered.contains(plugin))
                        .or_else(|| bound.get(registered.len()))
                    {
                        return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                            interface_id.clone(),
                            plugin.clone(),
                            point,
                        ));
                    }
                    if registered != bound {
                        let plugin = registered
                            .iter()
                            .zip(&bound)
                            .find_map(|(registered, bound)| {
                                (registered != bound).then(|| registered.clone())
                            })
                            .expect("different hook order must have a mismatched entry");
                        return Err(RegistryCompilationError::MissingExecutableExtension(
                            interface_id.clone(),
                            plugin,
                            point,
                        ));
                    }
                }
            } else if let Some((_, registration)) =
                registrations.iter().find(|(_, registration)| {
                    matches!(
                        registration.point(),
                        InterfaceExtensionPoint::Before
                            | InterfaceExtensionPoint::After
                            | InterfaceExtensionPoint::Failure
                            | InterfaceExtensionPoint::Completion
                    )
                })
            {
                return Err(RegistryCompilationError::MissingExecutableExtension(
                    interface_id.clone(),
                    registration.plugin().clone(),
                    registration.point(),
                ));
            }
        }
        for (interface_id, definition) in &self.definitions {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            validate_decision_binding(
                interface_id,
                definition,
                registrations,
                InterfaceExtensionPoint::Authorization,
                self.authorization_bindings.get(interface_id),
                &self.graph_fingerprint,
            )?;
            validate_decision_binding(
                interface_id,
                definition,
                registrations,
                InterfaceExtensionPoint::Admission,
                self.admission_bindings.get(interface_id),
                &self.graph_fingerprint,
            )?;
        }
        let mut plans = BTreeMap::new();
        for (binding, adapter_plan) in self.protocol_bindings.values() {
            let Some(definition) = self
                .definitions
                .get(binding.interface_identity().interface_id())
            else {
                return Err(RegistryCompilationError::BindingUnknownInterface(
                    binding.binding_id().clone(),
                ));
            };
            if binding.interface_identity().version() != definition.version() {
                return Err(RegistryCompilationError::BindingVersionMismatch(
                    binding.binding_id().clone(),
                ));
            }
            if binding.contracts() != definition.contracts() {
                return Err(RegistryCompilationError::BindingContractMismatch(
                    binding.binding_id().clone(),
                ));
            }
            let effective_handler = effective_handlers
                .get(definition.interface_id())
                .expect("effective handler was compiled above")
                .clone();
            let authentication = self
                .authentication_activations
                .get(&(
                    definition.interface_id().clone(),
                    adapter_plan.authentication().clone(),
                ))
                .expect("authentication activation was validated above")
                .clone();
            let extension_plan = CompiledInterfaceExtensionPlan::compile(
                self.extensions
                    .get(definition.interface_id())
                    .cloned()
                    .unwrap_or_default(),
            )?;
            let binding_fingerprint = binding_fingerprint(binding);
            let fingerprint = plan_fingerprint(
                &self.graph_fingerprint,
                definition,
                &binding_fingerprint,
                adapter_plan,
                &authentication,
                extension_plan.fingerprint(),
                &effective_handler,
            );
            plans.insert(
                binding.binding_id().clone(),
                CompiledInvocationPlan {
                    definition: definition.clone(),
                    binding: binding.clone(),
                    binding_fingerprint,
                    adapter_plan: adapter_plan.clone(),
                    authentication,
                    extension_plan,
                    executable_extensions: self
                        .hook_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    authorization_extensions: self
                        .authorization_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    admission_extensions: self
                        .admission_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    effective_handler,
                    fingerprint,
                },
            );
        }
        let fingerprint = registry_fingerprint(
            &self.graph_fingerprint,
            &self.definitions,
            &self
                .protocol_bindings
                .iter()
                .map(|(id, (binding, _))| (id.clone(), binding.clone()))
                .collect(),
            &plans,
        );
        Ok(Arc::new(CompiledInterfaceRegistry {
            graph_fingerprint: self.graph_fingerprint,
            fingerprint,
            definitions: self.definitions,
            definition_contributions: self.compiled_definition_contributions,
            protocol_bindings: self
                .protocol_bindings
                .into_iter()
                .map(|(id, (binding, _))| (id, binding))
                .collect(),
            plans,
            routes: self.routes,
            handler_bindings: effective_handler_bindings,
        }))
    }
}

pub struct CompiledInterfaceRegistry {
    graph_fingerprint: GraphFingerprint,
    fingerprint: RegistryFingerprint,
    definitions: BTreeMap<InterfaceId, InterfaceDefinition>,
    definition_contributions: BTreeMap<InterfaceId, PluginIdentity>,
    protocol_bindings: BTreeMap<BindingId, ProtocolBinding>,
    plans: BTreeMap<BindingId, CompiledInvocationPlan>,
    routes: BTreeMap<RouteIdentity, BindingId>,
    handler_bindings: BTreeMap<InterfaceId, Arc<dyn ErasedInterfaceBinding>>,
}

impl std::fmt::Debug for CompiledInterfaceRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledInterfaceRegistry")
            .field("graph_fingerprint", &self.graph_fingerprint)
            .field("fingerprint", &self.fingerprint)
            .field("definitions", &self.definitions)
            .finish_non_exhaustive()
    }
}

impl CompiledInterfaceRegistry {
    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }

    pub fn authentication(
        &self,
        binding_id: &BindingId,
    ) -> Option<&ActivatedAuthenticationAdapter> {
        self.plans
            .get(binding_id)
            .map(CompiledInvocationPlan::authentication)
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
            .and_then(|binding_id| self.plans.get(binding_id))
            .map(CompiledInvocationPlan::definition)
    }

    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &ProtocolBinding> {
        self.protocol_bindings.values()
    }

    pub fn binding(&self, binding_id: &BindingId) -> Option<&ProtocolBinding> {
        self.protocol_bindings.get(binding_id)
    }

    pub fn binding_by_route(&self, route: &RouteIdentity) -> Option<&ProtocolBinding> {
        self.routes
            .get(route)
            .and_then(|binding_id| self.protocol_bindings.get(binding_id))
    }

    pub fn plan(&self, binding_id: &BindingId) -> Option<&CompiledInvocationPlan> {
        self.plans.get(binding_id)
    }

    pub fn plan_for_interface(
        &self,
        interface_id: &InterfaceId,
    ) -> Option<&CompiledInvocationPlan> {
        self.plans
            .values()
            .find(|plan| plan.definition().interface_id() == interface_id)
    }

    pub(crate) fn handler<I, O, E, P>(
        &self,
        interface_id: &InterfaceId,
    ) -> Option<Arc<dyn InterfaceHandler<I, O, E, P>>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.handler_bindings
            .get(interface_id)?
            .as_any()
            .downcast_ref::<TypedInterfaceBinding<I, O, E, P>>()
            .map(|binding| Arc::clone(&binding.handler))
    }

    pub(crate) fn stream_handler<I, S, O, E, P>(
        &self,
        interface_id: &InterfaceId,
    ) -> Option<Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.handler_bindings
            .get(interface_id)?
            .as_any()
            .downcast_ref::<TypedInterfaceStreamBinding<I, S, O, E, P>>()
            .map(|binding| Arc::clone(&binding.handler))
    }
}

pub struct DynamicInterfaceRegistry {
    current: RwLock<Arc<CompiledInterfaceRegistry>>,
}

impl std::fmt::Debug for DynamicInterfaceRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DynamicInterfaceRegistry")
            .field("current", &self.snapshot())
            .finish()
    }
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

fn validate_decision_binding(
    interface_id: &InterfaceId,
    definition: &InterfaceDefinition,
    registrations: &[(u32, InterfaceExtensionRegistration)],
    point: InterfaceExtensionPoint,
    executable: Option<&Arc<dyn ErasedInterfaceDecisionPlan>>,
    graph: &GraphFingerprint,
) -> Result<(), RegistryCompilationError> {
    let mut registered = registrations
        .iter()
        .filter(|(_, registration)| registration.point() == point)
        .map(|(order, registration)| (*order, registration.plugin().clone()))
        .collect::<Vec<_>>();
    registered.sort();
    let registered = registered
        .into_iter()
        .map(|(_, plugin)| plugin)
        .collect::<Vec<_>>();
    let Some(executable) = executable else {
        if let Some(plugin) = registered.first() {
            return Err(RegistryCompilationError::MissingExecutableExtension(
                interface_id.clone(),
                plugin.clone(),
                point,
            ));
        }
        return Ok(());
    };
    if executable.graph_fingerprint() != graph {
        return Err(RegistryCompilationError::ExtensionGraphFingerprintMismatch(
            interface_id.clone(),
        ));
    }
    if executable.input_contract() != definition.input_contract()
        || executable.output_contract() != definition.output_contract()
    {
        return Err(RegistryCompilationError::DecisionContractMismatch(
            interface_id.clone(),
            point,
        ));
    }
    let bound = executable.plugin_bindings();
    if let Some(plugin) = registered
        .iter()
        .find(|plugin| !bound.contains(plugin))
        .or_else(|| registered.get(bound.len()))
    {
        return Err(RegistryCompilationError::MissingExecutableExtension(
            interface_id.clone(),
            plugin.clone(),
            point,
        ));
    }
    if let Some(plugin) = bound
        .iter()
        .find(|plugin| !registered.contains(plugin))
        .or_else(|| bound.get(registered.len()))
    {
        return Err(RegistryCompilationError::UnexpectedExecutableExtension(
            interface_id.clone(),
            plugin.clone(),
            point,
        ));
    }
    if registered != bound {
        return Err(RegistryCompilationError::MissingExecutableExtension(
            interface_id.clone(),
            registered
                .first()
                .expect("mismatched non-empty decision bindings")
                .clone(),
            point,
        ));
    }
    Ok(())
}

fn registry_fingerprint(
    graph_fingerprint: &GraphFingerprint,
    definitions: &BTreeMap<InterfaceId, InterfaceDefinition>,
    bindings: &BTreeMap<BindingId, ProtocolBinding>,
    plans: &BTreeMap<BindingId, CompiledInvocationPlan>,
) -> RegistryFingerprint {
    let mut digest = Sha256::new();
    digest.update(graph_fingerprint.as_str().as_bytes());
    for definition in definitions.values() {
        for part in [
            definition.interface_id().as_str(),
            definition.version().as_str(),
            definition.input_contract().contract_id(),
            definition.input_contract().version(),
            definition.output_contract().contract_id(),
            definition.output_contract().version(),
            definition.target_error_contract().contract_id(),
            definition.target_error_contract().version(),
            definition.authorization_operation().as_str(),
            definition.handler_reference().as_str(),
            definition.target_reference().as_str(),
            definition.owner().as_str(),
            match definition.authentication() {
                InterfaceAuthenticationPolicy::Anonymous => "authn:anonymous",
                InterfaceAuthenticationPolicy::Authenticated => "authn:authenticated",
            },
            match definition.principal_profile() {
                PrincipalProfile::Public => "principal:public",
                PrincipalProfile::User => "principal:user",
                PrincipalProfile::Application => "principal:application",
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
            match definition.execution_mode() {
                InterfaceExecutionMode::Unary => "mode:unary",
                InterfaceExecutionMode::ServerStream => "mode:server-stream",
                InterfaceExecutionMode::AsyncAck => "mode:async-ack",
            },
        ] {
            digest.update([0]);
            digest.update(part.as_bytes());
        }
        if let Some(event) = definition.stream_event_contract() {
            digest.update([0]);
            digest.update(event.contract_id().as_bytes());
            digest.update([0]);
            digest.update(event.version().as_bytes());
        }
    }
    for binding in bindings.values() {
        digest.update([0]);
        digest.update(binding.binding_id().as_str().as_bytes());
        digest.update([0]);
        digest.update(binding_fingerprint(binding).as_str().as_bytes());
        digest.update([0]);
        digest.update(
            plans
                .get(binding.binding_id())
                .expect("every binding must have a compiled plan")
                .fingerprint()
                .as_str()
                .as_bytes(),
        );
    }
    RegistryFingerprint::new(format!("sha256:{:x}", digest.finalize()))
        .expect("SHA-256 registry fingerprint must be a valid identity")
}

fn binding_fingerprint(binding: &ProtocolBinding) -> BindingFingerprint {
    let mut digest = Sha256::new();
    for part in [
        binding.binding_id().as_str(),
        binding.interface_identity().interface_id().as_str(),
        binding.interface_identity().version().as_str(),
        binding.input_contract().contract_id(),
        binding.input_contract().version(),
        binding.output_contract().contract_id(),
        binding.output_contract().version(),
        binding.contracts().target_error().contract_id(),
        binding.contracts().target_error().version(),
    ] {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    if let Some(event) = binding.contracts().stream_event() {
        digest.update([0]);
        digest.update(event.contract_id().as_bytes());
        digest.update([0]);
        digest.update(event.version().as_bytes());
    }
    digest.update([0]);
    digest.update(match binding.contracts().mode() {
        InterfaceExecutionMode::Unary => b"mode:unary".as_slice(),
        InterfaceExecutionMode::ServerStream => b"mode:server-stream".as_slice(),
        InterfaceExecutionMode::AsyncAck => b"mode:async-ack".as_slice(),
    });
    let (kind, first, second, third) = binding.projection().fingerprint_parts();
    for part in [Some(kind), Some(first), second, third]
        .into_iter()
        .flatten()
    {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    BindingFingerprint::new(format!("sha256:{:x}", digest.finalize()))
        .expect("SHA-256 binding fingerprint must be a valid identity")
}

fn plan_fingerprint(
    graph_fingerprint: &GraphFingerprint,
    definition: &InterfaceDefinition,
    binding_fingerprint: &BindingFingerprint,
    adapter_plan: &InvocationAdapterPlan,
    authentication: &ActivatedAuthenticationAdapter,
    extension_plan: &crate::ExtensionPlanFingerprint,
    effective_handler: &InterfaceHandlerCandidate,
) -> PlanFingerprint {
    let mut digest = Sha256::new();
    for part in [
        graph_fingerprint.as_str(),
        definition.interface_id().as_str(),
        definition.version().as_str(),
        binding_fingerprint.as_str(),
        definition.authorization_operation().as_str(),
        definition.handler_reference().as_str(),
        definition.target_reference().as_str(),
        adapter_plan.authentication().as_str(),
        authentication.activation().as_str(),
        adapter_plan.authorization().as_str(),
        effective_handler.plugin().as_str(),
        effective_handler.handler().as_str(),
        effective_handler.target().as_str(),
    ] {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    if let Some(admission) = adapter_plan.admission() {
        digest.update([0]);
        digest.update(admission.as_str().as_bytes());
    }
    digest.update([0]);
    digest.update(extension_plan.as_str().as_bytes());
    PlanFingerprint::new(format!("sha256:{:x}", digest.finalize()))
        .expect("SHA-256 plan fingerprint must be a valid identity")
}
