//! Runtime-safe package descriptors, manifest parsing, and installed artifact loading.
//!
//! Registry, installation orchestration, package intake, and graph compilation remain in
//! `plugin-framework`. Runtime hosts depend on this bounded loading layer instead of the full
//! package-management crate.

pub mod artifact_reconcile;
pub mod capability_kind;
pub mod data_source_package;
pub mod error {
    pub use extension_contracts::error::{
        ContractResult as FrameworkResult, ExtensionContractError as PluginFrameworkError,
        ExtensionContractErrorKind as PluginFrameworkErrorKind,
    };
}
pub mod manifest_v1;
pub mod network_egress_provider_package;
pub mod provider_count_tokens_estimator;
pub mod provider_package;

pub use artifact_reconcile::*;
pub use capability_kind::*;
pub use data_source_package::*;
pub use extension_contracts::error::{
    ContractResult as FrameworkResult, ExtensionContractError as PluginFrameworkError,
    ExtensionContractErrorKind as PluginFrameworkErrorKind,
};
pub use extension_contracts::*;
pub use manifest_v1::*;
pub use network_egress_provider_package::*;
pub use provider_count_tokens_estimator::*;
pub use provider_package::*;

#[cfg(test)]
mod _tests;
