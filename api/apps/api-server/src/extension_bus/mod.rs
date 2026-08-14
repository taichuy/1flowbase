mod input_assembly;
mod plugin_set;

pub use input_assembly::{
    assemble_extension_graph_input, ExtensionGraphInputAssembly, ModuleActivationFact,
    BOOT_CORE_MODULE_ID, DEFAULT_PLUGIN_SET_PATH,
};
pub use plugin_set::{
    parse_deployment_plugin_set, DeploymentPluginSet, DeploymentPluginSetCatalog,
};
