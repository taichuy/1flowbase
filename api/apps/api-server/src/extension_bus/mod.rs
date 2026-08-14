mod input_assembly;

pub(crate) use input_assembly::infrastructure_provider_contribution_id;
pub use input_assembly::{
    assemble_extension_graph_input, ExtensionGraphInputAssembly, ModuleActivationFact,
    BOOT_CORE_MODULE_ID, CACHE_STORE_CONTRACT_ID, CACHE_STORE_CONTRACT_VERSION,
    CACHE_STORE_EXTENSION_POINT_ID, DEFAULT_PLUGIN_SET_PATH,
};
