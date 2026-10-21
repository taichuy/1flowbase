#[path = "extension_ports/managed_service.rs"]
mod managed_service;
pub use managed_service::*;
#[path = "resource_ports/application.rs"]
pub mod application;
#[path = "resource_ports/application_public_api.rs"]
pub mod application_public_api;
pub mod auth;
pub mod billing_core;
#[path = "resource_ports/data_source.rs"]
pub mod data_source;
#[path = "extension_ports/extension_installation.rs"]
pub mod extension_installation;
#[path = "resource_ports/file_management.rs"]
pub mod file_management;
#[path = "resource_ports/flow.rs"]
pub mod flow;
pub mod frontstage;
pub mod frontstage_blocks;
pub mod i18n_catalog;
pub mod infrastructure;
#[path = "extension_ports/lifecycle_outbox.rs"]
pub mod lifecycle_outbox;
#[path = "extension_ports/managed_schema.rs"]
pub mod managed_schema;
#[path = "integration_ports/mcp_management.rs"]
pub mod mcp_management;
#[path = "integration_ports/mcp_result_receipt.rs"]
pub mod mcp_result_receipt;
#[path = "resource_ports/model_definition.rs"]
pub mod model_definition;
pub mod model_provider;
#[path = "integration_ports/network_egress.rs"]
pub mod network_egress;
#[path = "extension_ports/plugin.rs"]
pub mod plugin;
#[cfg(feature = "responses-history-migration")]
pub mod responses_history_migration;
pub mod runtime;
#[cfg(feature = "responses-history-migration")]
pub use responses_history_migration::*;
#[path = "resource_ports/system_branding.rs"]
pub mod system_branding;
pub use system_branding::*;

pub mod system_backup;
pub mod ui_management;

use async_trait::async_trait;
use domain::{
    ActorContext, AuditLogRecord, DataModelScopeKind, ModelDefinitionRecord, ModelFieldKind,
    ModelFieldRecord,
};
use time::OffsetDateTime;
use uuid::Uuid;

pub use application::*;
pub use application_public_api::*;
pub use auth::*;
pub use billing_core::*;
pub use data_source::*;
pub use extension_installation::*;
pub use file_management::*;
pub use flow::*;
pub use frontstage::*;
pub use frontstage_blocks::*;
pub use i18n_catalog::*;
pub use infrastructure::*;
pub use lifecycle_outbox::*;
pub use managed_schema::*;
pub use mcp_management::*;
pub use mcp_result_receipt::*;
pub use model_definition::*;
pub use model_provider::*;
pub use network_egress::*;
pub use plugin::*;
pub use runtime::*;
pub use system_backup::*;
pub use ui_management::*;

#[path = "extension_ports/plugin_contribution_authority.rs"]
pub mod plugin_contribution_authority;
pub use plugin_contribution_authority::*;

#[path = "resource_ports/portable_template.rs"]
pub mod portable_template;
pub use portable_template::*;

#[path = "integration_ports/mcp_oauth.rs"]
pub mod mcp_oauth;

pub mod organization;
pub use organization::*;

#[path = "extension_ports/plugin_credential.rs"]
pub mod plugin_credential;
pub use plugin_credential::*;

#[path = "integration_ports/notifications.rs"]
pub mod notifications;
pub use notifications::*;

#[path = "integration_ports/notification_publication.rs"]
pub mod notification_publication;
pub use notification_publication::*;
