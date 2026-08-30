use std::marker::PhantomData;

use thiserror::Error;

use crate::{
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceExecutionMode,
    InvocationAdapterPlan, InvocationPrincipal, PrincipalProfile, ProtocolBinding,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContributedProtocolBinding {
    binding: ProtocolBinding,
    adapter_plan: InvocationAdapterPlan,
}

impl ContributedProtocolBinding {
    pub fn new(binding: ProtocolBinding, adapter_plan: InvocationAdapterPlan) -> Self {
        Self {
            binding,
            adapter_plan,
        }
    }

    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }

    pub fn adapter_plan(&self) -> &InvocationAdapterPlan {
        &self.adapter_plan
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DefinitionContributionBindingError {
    #[error("definition contribution execution mode does not match its typed binding")]
    ExecutionModeMismatch,
    #[error("definition contribution contracts do not match its typed binding")]
    ContractMismatch,
    #[error("definition contribution principal profile does not match its typed binding")]
    PrincipalProfileMismatch,
    #[error("definition contribution must include at least one protocol binding")]
    MissingProtocolBinding,
    #[error("definition contribution protocol binding does not match its definition")]
    ProtocolBindingMismatch,
}

pub struct TypedInterfaceDefinitionContribution<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    definition: InterfaceDefinition,
    bindings: Vec<ContributedProtocolBinding>,
    marker: PhantomData<fn(I, O, E, P)>,
}

impl<I, O, E, P> TypedInterfaceDefinitionContribution<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    pub fn new(
        definition: InterfaceDefinition,
        bindings: impl IntoIterator<Item = ContributedProtocolBinding>,
    ) -> Result<Self, DefinitionContributionBindingError> {
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        let contracts = InterfaceContracts::unary(
            contract_identity::<I>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        validate_definition_contribution(&definition, &contracts, P::PROFILE, &bindings).map(
            |bindings| Self {
                definition,
                bindings,
                marker: PhantomData,
            },
        )
    }
}

pub struct TypedInterfaceStreamDefinitionContribution<I, S, O, E, P>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    definition: InterfaceDefinition,
    bindings: Vec<ContributedProtocolBinding>,
    marker: PhantomData<fn(I, S, O, E, P)>,
}

impl<I, S, O, E, P> TypedInterfaceStreamDefinitionContribution<I, S, O, E, P>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    pub fn new(
        definition: InterfaceDefinition,
        bindings: impl IntoIterator<Item = ContributedProtocolBinding>,
    ) -> Result<Self, DefinitionContributionBindingError> {
        let bindings = bindings.into_iter().collect::<Vec<_>>();
        let contracts = InterfaceContracts::server_stream(
            contract_identity::<I>(),
            contract_identity::<S>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        validate_definition_contribution(&definition, &contracts, P::PROFILE, &bindings).map(
            |bindings| Self {
                definition,
                bindings,
                marker: PhantomData,
            },
        )
    }
}

pub(crate) trait ErasedDefinitionContribution: Send + Sync {
    fn definition(&self) -> &InterfaceDefinition;
    fn bindings(&self) -> &[ContributedProtocolBinding];
}

impl<I, O, E, P> ErasedDefinitionContribution for TypedInterfaceDefinitionContribution<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }

    fn bindings(&self) -> &[ContributedProtocolBinding] {
        &self.bindings
    }
}

impl<I, S, O, E, P> ErasedDefinitionContribution
    for TypedInterfaceStreamDefinitionContribution<I, S, O, E, P>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }

    fn bindings(&self) -> &[ContributedProtocolBinding] {
        &self.bindings
    }
}

fn validate_definition_contribution(
    definition: &InterfaceDefinition,
    contracts: &InterfaceContracts,
    principal_profile: PrincipalProfile,
    bindings: &[ContributedProtocolBinding],
) -> Result<Vec<ContributedProtocolBinding>, DefinitionContributionBindingError> {
    if definition.execution_mode() != contracts.mode() {
        return Err(DefinitionContributionBindingError::ExecutionModeMismatch);
    }
    if definition.contracts() != contracts {
        return Err(DefinitionContributionBindingError::ContractMismatch);
    }
    if definition.principal_profile() != principal_profile {
        return Err(DefinitionContributionBindingError::PrincipalProfileMismatch);
    }
    if bindings.is_empty() {
        return Err(DefinitionContributionBindingError::MissingProtocolBinding);
    }
    if bindings.iter().any(|entry| {
        entry.binding().interface_identity() != definition.identity()
            || entry.binding().contracts() != definition.contracts()
    }) {
        return Err(DefinitionContributionBindingError::ProtocolBindingMismatch);
    }
    Ok(bindings.to_vec())
}

fn contract_identity<T: InterfaceContract>() -> crate::ContractIdentity {
    crate::ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed definition contribution contract constants must be valid identities")
}
