use std::collections::{BTreeMap, BTreeSet};

use plugin_framework::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelTemplateContractError,
    DataModelTemplateDescriptor, DataModelTemplateIdentity, DataModelTemplateSource,
};
use thiserror::Error;

/// Resolves stable descriptor references against the host's compiled inventories.
/// Implementations confirm registration only; executable functions remain owned by
/// the runtime dispatch boundary.
pub trait DataModelTemplateResolutionContract {
    fn contains_capability(&self, capability: &DataModelCapabilityRequirement) -> bool;

    fn contains_operation_handler(&self, handler_ref: &DataModelOperationHandlerRef) -> bool;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledDataModelTemplate {
    descriptor: DataModelTemplateDescriptor,
}

impl CompiledDataModelTemplate {
    pub fn descriptor(&self) -> &DataModelTemplateDescriptor {
        &self.descriptor
    }

    pub fn identity(&self) -> &DataModelTemplateIdentity {
        &self.descriptor.identity
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DataModelTemplateRegistry {
    templates: BTreeMap<DataModelTemplateIdentity, CompiledDataModelTemplate>,
}

impl DataModelTemplateRegistry {
    pub fn compile(
        descriptors: impl IntoIterator<Item = DataModelTemplateDescriptor>,
        resolution: &impl DataModelTemplateResolutionContract,
    ) -> Result<Self, DataModelTemplateRegistryError> {
        let mut templates = BTreeMap::new();

        for descriptor in descriptors {
            descriptor.validate()?;

            for capability in &descriptor.required_capabilities {
                if !resolution.contains_capability(capability) {
                    return Err(DataModelTemplateRegistryError::UnresolvedCapability {
                        identity: descriptor.identity.clone(),
                        capability: capability.code.clone(),
                    });
                }
            }
            for operation in &descriptor.operations {
                if !resolution.contains_operation_handler(&operation.handler_ref) {
                    return Err(DataModelTemplateRegistryError::UnresolvedOperationHandler {
                        identity: descriptor.identity.clone(),
                        operation: operation.code.clone(),
                        handler_ref: operation.handler_ref.clone(),
                    });
                }
            }

            let identity = descriptor.identity.clone();
            let compiled = CompiledDataModelTemplate { descriptor };
            if templates.insert(identity.clone(), compiled).is_some() {
                return Err(DataModelTemplateRegistryError::DuplicateIdentity(identity));
            }
        }

        Ok(Self { templates })
    }

    pub fn resolve(
        &self,
        identity: &DataModelTemplateIdentity,
    ) -> Result<&CompiledDataModelTemplate, DataModelTemplateRegistryError> {
        self.templates
            .get(identity)
            .ok_or_else(|| DataModelTemplateRegistryError::UnknownIdentity(identity.clone()))
    }

    pub fn templates(&self) -> impl Iterator<Item = &CompiledDataModelTemplate> {
        self.templates.values()
    }

    pub fn compatible_templates<'a>(
        &'a self,
        source: &DataModelTemplateSource,
        provided_capabilities: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&'a CompiledDataModelTemplate> {
        let provided_capabilities = provided_capabilities.into_iter().collect::<BTreeSet<_>>();

        self.templates
            .values()
            .filter(|template| template.descriptor.source_selector.matches(source))
            .filter(|template| {
                template
                    .descriptor
                    .required_capabilities
                    .iter()
                    .all(|required| provided_capabilities.contains(required.code.as_str()))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DataModelTemplateRegistryError {
    #[error(transparent)]
    InvalidDescriptor(#[from] DataModelTemplateContractError),
    #[error("duplicate data model template identity: {}", .0.canonical_name())]
    DuplicateIdentity(DataModelTemplateIdentity),
    #[error("unknown data model template identity: {}", .0.canonical_name())]
    UnknownIdentity(DataModelTemplateIdentity),
    #[error(
        "unresolved capability {capability} for data model template {}",
        identity.canonical_name()
    )]
    UnresolvedCapability {
        identity: DataModelTemplateIdentity,
        capability: String,
    },
    #[error(
        "unresolved operation handler {} for operation {operation} in data model template {}",
        handler_ref.canonical_name(),
        identity.canonical_name()
    )]
    UnresolvedOperationHandler {
        identity: DataModelTemplateIdentity,
        operation: String,
        handler_ref: DataModelOperationHandlerRef,
    },
}
