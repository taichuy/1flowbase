use std::{any::Any, future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    AuthorizationDecisionFingerprint, ContractIdentity, GraphFingerprint, InterfaceContract,
    InterfaceDefinition, InterfaceProtocol, PluginIdentity, PrincipalSummary, ProtocolBinding,
};

#[derive(Clone, Debug)]
pub struct InterfaceAuthorizationContributionRequest {
    principal: PrincipalSummary,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
}

impl InterfaceAuthorizationContributionRequest {
    pub(crate) fn new(
        principal: PrincipalSummary,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            principal,
            definition,
            binding,
            protocol,
        }
    }
    pub fn principal(&self) -> &PrincipalSummary {
        &self.principal
    }
    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }
    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }
    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceAdmissionContributionRequest {
    principal: PrincipalSummary,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
    authorization: AuthorizationDecisionFingerprint,
}

impl InterfaceAdmissionContributionRequest {
    pub(crate) fn new(
        principal: PrincipalSummary,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
        authorization: AuthorizationDecisionFingerprint,
    ) -> Self {
        Self {
            principal,
            definition,
            binding,
            protocol,
            authorization,
        }
    }
    pub fn principal(&self) -> &PrincipalSummary {
        &self.principal
    }
    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }
    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }
    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }
    pub fn authorization(&self) -> &AuthorizationDecisionFingerprint {
        &self.authorization
    }
}

#[derive(Debug, Error)]
#[error("interface authorization contribution rejected with {classification}")]
pub struct InterfaceAuthorizationContributionError {
    classification: Arc<str>,
}

impl InterfaceAuthorizationContributionError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
        }
    }
    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }
}

#[derive(Debug, Error)]
#[error("interface admission contribution rejected with {classification}")]
pub struct InterfaceAdmissionContributionError {
    classification: Arc<str>,
}

impl InterfaceAdmissionContributionError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
        }
    }
    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }
}

pub type InterfaceAuthorizationContributionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), InterfaceAuthorizationContributionError>> + Send + 'a>>;
pub type InterfaceAdmissionContributionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), InterfaceAdmissionContributionError>> + Send + 'a>>;

pub trait InterfaceAuthorizationContribution: Send + Sync + 'static {
    fn authorize(
        &self,
        request: InterfaceAuthorizationContributionRequest,
    ) -> InterfaceAuthorizationContributionFuture<'_>;
}

pub trait InterfaceAdmissionContribution: Send + Sync + 'static {
    fn admit(
        &self,
        request: InterfaceAdmissionContributionRequest,
    ) -> InterfaceAdmissionContributionFuture<'_>;
}

pub struct TypedInterfaceAuthorizationPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    graph: GraphFingerprint,
    input: ContractIdentity,
    output: ContractIdentity,
    bindings: Vec<(PluginIdentity, Arc<dyn InterfaceAuthorizationContribution>)>,
    marker: PhantomData<fn(I, O)>,
}

pub struct TypedInterfaceAdmissionPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    graph: GraphFingerprint,
    input: ContractIdentity,
    output: ContractIdentity,
    bindings: Vec<(PluginIdentity, Arc<dyn InterfaceAdmissionContribution>)>,
    marker: PhantomData<fn(I, O)>,
}

macro_rules! typed_decision_plan {
    ($name:ident, $contribution:ident) => {
        impl<I, O> $name<I, O>
        where
            I: InterfaceContract,
            O: InterfaceContract,
        {
            pub fn new(graph: GraphFingerprint) -> Self {
                Self {
                    graph,
                    input: contract_identity::<I>(),
                    output: contract_identity::<O>(),
                    bindings: Vec::new(),
                    marker: PhantomData,
                }
            }
            pub fn bind(
                mut self,
                plugin: PluginIdentity,
                contribution: Arc<dyn $contribution>,
            ) -> Self {
                self.bindings.push((plugin, contribution));
                self
            }
            pub fn graph_fingerprint(&self) -> &GraphFingerprint {
                &self.graph
            }
            pub fn input_contract(&self) -> &ContractIdentity {
                &self.input
            }
            pub fn output_contract(&self) -> &ContractIdentity {
                &self.output
            }
            pub(crate) fn plugin_bindings(&self) -> Vec<PluginIdentity> {
                self.bindings
                    .iter()
                    .map(|(plugin, _)| plugin.clone())
                    .collect()
            }
        }
    };
}

typed_decision_plan!(
    TypedInterfaceAuthorizationPlan,
    InterfaceAuthorizationContribution
);
typed_decision_plan!(TypedInterfaceAdmissionPlan, InterfaceAdmissionContribution);

impl<I, O> TypedInterfaceAuthorizationPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    pub(crate) async fn run(
        &self,
        request: InterfaceAuthorizationContributionRequest,
    ) -> Result<(), InterfaceAuthorizationContributionError> {
        for (_, contribution) in &self.bindings {
            contribution.authorize(request.clone()).await?;
        }
        Ok(())
    }
}

impl<I, O> TypedInterfaceAdmissionPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    pub(crate) async fn run(
        &self,
        request: InterfaceAdmissionContributionRequest,
    ) -> Result<(), InterfaceAdmissionContributionError> {
        for (_, contribution) in &self.bindings {
            contribution.admit(request.clone()).await?;
        }
        Ok(())
    }
}

pub(crate) trait ErasedInterfaceDecisionPlan: Send + Sync {
    fn graph_fingerprint(&self) -> &GraphFingerprint;
    fn input_contract(&self) -> &ContractIdentity;
    fn output_contract(&self) -> &ContractIdentity;
    fn plugin_bindings(&self) -> Vec<PluginIdentity>;
    fn as_any(&self) -> &dyn Any;
}

macro_rules! erased_decision_plan {
    ($name:ident) => {
        impl<I, O> ErasedInterfaceDecisionPlan for $name<I, O>
        where
            I: InterfaceContract,
            O: InterfaceContract,
        {
            fn graph_fingerprint(&self) -> &GraphFingerprint {
                self.graph_fingerprint()
            }
            fn input_contract(&self) -> &ContractIdentity {
                self.input_contract()
            }
            fn output_contract(&self) -> &ContractIdentity {
                self.output_contract()
            }
            fn plugin_bindings(&self) -> Vec<PluginIdentity> {
                self.plugin_bindings()
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
        }
    };
}

erased_decision_plan!(TypedInterfaceAuthorizationPlan);
erased_decision_plan!(TypedInterfaceAdmissionPlan);

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed decision contract constants must be valid identities")
}
