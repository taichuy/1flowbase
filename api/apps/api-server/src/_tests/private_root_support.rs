// Pure fixture bridge; no test leaf or integration_bridge is mounted here.
pub(crate) mod support {
    pub(crate) use crate::test_support::support::*;
}

#[cfg_attr(test, api_test_selection::group(core))]
pub(crate) mod application {
    pub(crate) use crate::test_support::application::*;
}
#[cfg_attr(test, api_test_selection::group(core))]
pub(crate) mod extension_bus {
    pub(crate) mod managed_snapshot_tests {
        use crate::extension_bus::{
            ManagedExtensionComposition, ManagedWorkspacePublicationSource,
        };
        use crate::host_extensions::lifecycle::ApiLifecycleFactDelivery;
        use crate::provider_runtime::ApiRuntimeServices;
        use std::sync::Arc;
        use storage_durable_postgres::MainDurableStore;

        include!("extension_bus/managed/runtime_fixture.rs");
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
pub(crate) use crate::native_settings_fixture::NativeSettingsFixture;
