use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, RwLock},
};

use extension_contracts::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelOperationMethod,
    DataModelTemplateContractError, DataModelTemplateDescriptor, DataModelTemplateIdentity,
    DataModelTemplateOperation, DataModelTemplateSource,
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

    pub fn operation(&self, code: &str) -> Option<&DataModelTemplateOperation> {
        self.descriptor
            .operations
            .iter()
            .find(|operation| operation.code == code)
    }

    pub fn match_operation<'a>(
        &'a self,
        method: DataModelOperationMethod,
        request_path: &str,
    ) -> Option<MatchedDataModelTemplateOperation<'a>> {
        self.descriptor.operations.iter().find_map(|operation| {
            (operation.method == method)
                .then(|| match_path_template(&operation.path, request_path))
                .flatten()
                .map(|path_parameters| MatchedDataModelTemplateOperation {
                    operation,
                    path_parameters,
                })
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedDataModelTemplateOperation<'a> {
    pub operation: &'a DataModelTemplateOperation,
    pub path_parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DataModelTemplateRegistry {
    templates: BTreeMap<DataModelTemplateIdentity, CompiledDataModelTemplate>,
}

fn match_path_template(template: &str, request_path: &str) -> Option<BTreeMap<String, String>> {
    let template_segments = template.split('/').collect::<Vec<_>>();
    let request_segments = request_path.split('/').collect::<Vec<_>>();
    if template_segments.len() != request_segments.len() {
        return None;
    }

    let mut parameters = BTreeMap::new();
    for (template_segment, request_segment) in template_segments.into_iter().zip(request_segments) {
        if let Some(parameter) = template_segment
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        {
            if request_segment.is_empty() {
                return None;
            }
            parameters.insert(parameter.to_owned(), request_segment.to_owned());
        } else if template_segment != request_segment {
            return None;
        }
    }
    Some(parameters)
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
                        handler_ref: Box::new(operation.handler_ref.clone()),
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

#[derive(Debug, Clone)]
pub struct DataModelTemplateCatalog {
    core_descriptors: Arc<Vec<DataModelTemplateDescriptor>>,
    provider_descriptors: Arc<RwLock<BTreeMap<String, Vec<DataModelTemplateDescriptor>>>>,
    compiled: Arc<RwLock<DataModelTemplateRegistry>>,
}

impl DataModelTemplateCatalog {
    pub fn core() -> Self {
        let core_registry = crate::general_data_model_template::core_data_model_template_registry()
            .expect("Core data model template registry must compile");
        let core_descriptors = core_registry
            .templates()
            .map(|template| template.descriptor().clone())
            .collect::<Vec<_>>();
        Self {
            core_descriptors: Arc::new(core_descriptors),
            provider_descriptors: Arc::new(RwLock::new(BTreeMap::new())),
            compiled: Arc::new(RwLock::new(core_registry.clone())),
        }
    }

    pub fn replace_provider(
        &self,
        provider_key: impl Into<String>,
        provider_namespace: &str,
        descriptors: Vec<DataModelTemplateDescriptor>,
    ) -> Result<(), DataModelTemplateRegistryError> {
        for descriptor in &descriptors {
            if descriptor.identity.provider != provider_namespace
                || descriptor
                    .operations
                    .iter()
                    .any(|operation| operation.handler_ref.provider != provider_namespace)
            {
                return Err(DataModelTemplateRegistryError::ProviderNamespaceMismatch {
                    provider_namespace: provider_namespace.to_owned(),
                    identity: descriptor.identity.clone(),
                });
            }
        }

        let provider_key = provider_key.into();
        let mut providers = self
            .provider_descriptors
            .write()
            .expect("data model template provider catalog poisoned");
        let previous = providers.insert(provider_key.clone(), descriptors);
        let compiled = compile_catalog(&self.core_descriptors, &providers);
        match compiled {
            Ok(compiled) => {
                *self
                    .compiled
                    .write()
                    .expect("data model template compiled catalog poisoned") = compiled;
                Ok(())
            }
            Err(error) => {
                match previous {
                    Some(previous) => {
                        providers.insert(provider_key, previous);
                    }
                    None => {
                        providers.remove(&provider_key);
                    }
                }
                Err(error)
            }
        }
    }

    pub fn resolve(
        &self,
        identity: &DataModelTemplateIdentity,
    ) -> Result<CompiledDataModelTemplate, DataModelTemplateRegistryError> {
        self.compiled
            .read()
            .expect("data model template compiled catalog poisoned")
            .resolve(identity)
            .cloned()
    }

    pub fn compatible_templates<'a>(
        &self,
        source: &DataModelTemplateSource,
        provided_capabilities: impl IntoIterator<Item = &'a str>,
    ) -> Vec<CompiledDataModelTemplate> {
        let provided_capabilities = provided_capabilities
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        self.compiled
            .read()
            .expect("data model template compiled catalog poisoned")
            .templates
            .values()
            .filter(|template| template.descriptor.source_selector.matches(source))
            .filter(|template| {
                template
                    .descriptor
                    .required_capabilities
                    .iter()
                    .all(|required| provided_capabilities.contains(&required.code))
            })
            .cloned()
            .collect()
    }

    pub fn templates(&self) -> Vec<CompiledDataModelTemplate> {
        self.compiled
            .read()
            .expect("data model template compiled catalog poisoned")
            .templates()
            .cloned()
            .collect()
    }
}

fn compile_catalog(
    core_descriptors: &[DataModelTemplateDescriptor],
    providers: &BTreeMap<String, Vec<DataModelTemplateDescriptor>>,
) -> Result<DataModelTemplateRegistry, DataModelTemplateRegistryError> {
    let descriptors = core_descriptors
        .iter()
        .chain(providers.values().flatten())
        .cloned()
        .collect::<Vec<_>>();
    let resolution = CatalogResolution::new(&descriptors);
    DataModelTemplateRegistry::compile(descriptors, &resolution)
}

struct CatalogResolution {
    capabilities: BTreeSet<DataModelCapabilityRequirement>,
    operation_handlers: BTreeSet<DataModelOperationHandlerRef>,
}

impl CatalogResolution {
    fn new(descriptors: &[DataModelTemplateDescriptor]) -> Self {
        Self {
            capabilities: descriptors
                .iter()
                .flat_map(|descriptor| descriptor.required_capabilities.iter().cloned())
                .collect(),
            operation_handlers: descriptors
                .iter()
                .flat_map(|descriptor| {
                    descriptor
                        .operations
                        .iter()
                        .map(|operation| operation.handler_ref.clone())
                })
                .collect(),
        }
    }
}

impl DataModelTemplateResolutionContract for CatalogResolution {
    fn contains_capability(&self, capability: &DataModelCapabilityRequirement) -> bool {
        self.capabilities.contains(capability)
    }

    fn contains_operation_handler(&self, handler_ref: &DataModelOperationHandlerRef) -> bool {
        self.operation_handlers.contains(handler_ref)
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
        "data model template {} does not belong to provider namespace {provider_namespace}",
        identity.canonical_name()
    )]
    ProviderNamespaceMismatch {
        provider_namespace: String,
        identity: DataModelTemplateIdentity,
    },
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
        handler_ref: Box<DataModelOperationHandlerRef>,
    },
}
