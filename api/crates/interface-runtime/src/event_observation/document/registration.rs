use super::*;
use crate::{ContractIdentity, RegistryCompilationError, RegistryCompiler};

pub(crate) trait ErasedDocumentOperationCodec: Send + Sync {
    fn input(&self, input: &dyn Any) -> Result<Value, Arc<str>>;
    fn output(&self, output: &dyn Any) -> Result<Value, Arc<str>>;
}
struct Codec<I: InterfaceContract, O: InterfaceContract>(Arc<dyn DocumentOperationCodec<I, O>>);
impl<I: InterfaceContract, O: InterfaceContract> ErasedDocumentOperationCodec for Codec<I, O> {
    fn input(&self, input: &dyn Any) -> Result<Value, Arc<str>> {
        self.0.project_input(
            input
                .downcast_ref()
                .ok_or("document-input-contract-mismatch")?,
        )
    }
    fn output(&self, output: &dyn Any) -> Result<Value, Arc<str>> {
        self.0.project_output(
            output
                .downcast_ref()
                .ok_or("document-output-contract-mismatch")?,
        )
    }
}
#[derive(Clone)]
pub(crate) struct Projection {
    pub descriptor: DocumentOperationDescriptor,
    pub codec: Arc<dyn ErasedDocumentOperationCodec>,
}
impl<I: InterfaceContract, O: InterfaceContract> From<DocumentOperationProjection<I, O>>
    for Projection
{
    fn from(value: DocumentOperationProjection<I, O>) -> Self {
        Self {
            descriptor: value.descriptor,
            codec: Arc::new(Codec(value.codec)),
        }
    }
}

/// Frozen per-binding registration, installed only by the composition root.
pub struct DocumentOperationRegistration<I: InterfaceContract, O: InterfaceContract> {
    generation: Arc<str>,
    static_operation: Option<DocumentOperationProjection<I, O>>,
    selector: Option<Arc<dyn DocumentOperationSelector<I, O>>>,
}
impl<I: InterfaceContract, O: InterfaceContract> DocumentOperationRegistration<I, O> {
    pub fn static_operation(
        generation: impl Into<Arc<str>>,
        projection: DocumentOperationProjection<I, O>,
    ) -> Self {
        Self {
            generation: generation.into(),
            static_operation: Some(projection),
            selector: None,
        }
    }
    pub fn dynamic(
        generation: impl Into<Arc<str>>,
        selector: Arc<dyn DocumentOperationSelector<I, O>>,
    ) -> Self {
        Self {
            generation: generation.into(),
            static_operation: None,
            selector: Some(selector),
        }
    }
}
pub(crate) trait ErasedDocumentOperationRegistration: Send + Sync {
    fn generation(&self) -> &str;
    fn contracts(&self) -> (ContractIdentity, ContractIdentity);
    fn contract_types(&self) -> (std::any::TypeId, std::any::TypeId);
    fn static_projection(&self) -> Option<Projection>;
    fn static_descriptor(&self) -> Option<&DocumentOperationDescriptor>;
    fn select(&self, snapshot: &(dyn Any + Send + Sync)) -> Result<Projection, Arc<str>>;
    fn fingerprint(&self) -> Vec<u8>;
}
impl<I: InterfaceContract, O: InterfaceContract> ErasedDocumentOperationRegistration
    for DocumentOperationRegistration<I, O>
{
    fn generation(&self) -> &str {
        &self.generation
    }
    fn contract_types(&self) -> (std::any::TypeId, std::any::TypeId) {
        (std::any::TypeId::of::<I>(), std::any::TypeId::of::<O>())
    }
    fn contracts(&self) -> (ContractIdentity, ContractIdentity) {
        (
            ContractIdentity::new(I::CONTRACT_ID, I::CONTRACT_VERSION).expect("typed contract"),
            ContractIdentity::new(O::CONTRACT_ID, O::CONTRACT_VERSION).expect("typed contract"),
        )
    }
    fn static_descriptor(&self) -> Option<&DocumentOperationDescriptor> {
        self.static_operation
            .as_ref()
            .map(|projection| &projection.descriptor)
    }
    fn static_projection(&self) -> Option<Projection> {
        self.static_operation.clone().map(Into::into)
    }
    fn select(&self, snapshot: &(dyn Any + Send + Sync)) -> Result<Projection, Arc<str>> {
        self.selector
            .as_ref()
            .ok_or("document-operation-static")?
            .select(snapshot)
            .map(Into::into)
    }
    fn fingerprint(&self) -> Vec<u8> {
        let static_descriptor = self
            .static_operation
            .as_ref()
            .map(|p| format!("{:?}", p.descriptor));
        format!(
            "{:?}|{:?}|{:?}",
            self.generation,
            self.contracts(),
            static_descriptor
        )
        .into_bytes()
    }
}
impl RegistryCompiler {
    pub fn with_document_operation_observer(
        mut self,
        observer: Arc<dyn DocumentOperationObserver>,
    ) -> Self {
        self.document_observer = Some(observer);
        self
    }
    pub fn bind_document_operation<I: InterfaceContract, O: InterfaceContract>(
        &mut self,
        binding_id: &BindingId,
        registration: DocumentOperationRegistration<I, O>,
    ) -> Result<(), RegistryCompilationError> {
        if self.document_operations.contains_key(binding_id) {
            return Err(RegistryCompilationError::DocumentOperationMismatch(
                binding_id.clone(),
            ));
        }
        self.document_operations
            .insert(binding_id.clone(), Arc::new(registration));
        Ok(())
    }
}

pub(crate) fn validate_descriptor(
    descriptor: &DocumentOperationDescriptor,
    binding: &crate::ProtocolBinding,
    generation: &str,
) -> bool {
    descriptor.binding_id == *binding.binding_id()
        && descriptor.interface_identity == *binding.interface_identity()
        && descriptor.registration_generation.as_ref() == generation
        && !descriptor.operation_id.trim().is_empty()
        && !descriptor.target_identity.trim().is_empty()
        && !descriptor.target_generation.trim().is_empty()
        && (descriptor.schemas.event.is_none() || binding.contracts().stream_event().is_some())
}
