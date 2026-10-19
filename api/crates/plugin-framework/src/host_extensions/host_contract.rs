#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostContractCode {
    Identity,
    Workspace,
    PluginManagement,
    RuntimeOrchestration,
    StorageDurable,
    StorageEphemeral,
    StorageObject,
    FileManagement,
    DataAccess,
    ModelRuntime,
    ScopeProvider,
    Observability,
}

impl HostContractCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Workspace => "workspace",
            Self::PluginManagement => "plugin_management",
            Self::RuntimeOrchestration => "runtime_orchestration",
            Self::StorageDurable => "storage-durable",
            Self::StorageEphemeral => "storage-ephemeral",
            Self::StorageObject => "storage-object",
            Self::FileManagement => "file_management",
            Self::DataAccess => "data_access",
            Self::ModelRuntime => "model_runtime",
            Self::ScopeProvider => "scope-provider",
            Self::Observability => "observability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeSlotCode {
    ModelProvider,
    EmbeddingProvider,
    RerankerProvider,
    DataSource,
    DataImportSnapshot,
    FileProcessor,
    RecordValidator,
    FieldComputedValue,
    NetworkEgressProvider,
}

impl RuntimeSlotCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelProvider => "model_provider",
            Self::EmbeddingProvider => "embedding_provider",
            Self::RerankerProvider => "reranker_provider",
            Self::DataSource => "data_source",
            Self::DataImportSnapshot => "data_import_snapshot",
            Self::FileProcessor => "file_processor",
            Self::RecordValidator => "record_validator",
            Self::FieldComputedValue => "field_computed_value",
            Self::NetworkEgressProvider => "network_egress_provider",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageImplementationKind {
    Durable,
    Ephemeral,
    Object,
}

impl StorageImplementationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Durable => "storage-durable",
            Self::Ephemeral => "storage-ephemeral",
            Self::Object => "storage-object",
        }
    }
}
