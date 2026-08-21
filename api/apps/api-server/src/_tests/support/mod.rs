mod applications;
mod auth;
mod packages;
mod plugins;

use std::{fs, io::Write, path::Path, sync::Arc};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString},
};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use control_plane::bootstrap::{BootstrapConfig, BootstrapService};
use control_plane::ports::{
    DownloadedOfficialPluginPackage, OfficialPluginArtifact, OfficialPluginCatalogSnapshot,
    OfficialPluginCatalogSource, OfficialPluginI18nSummary, OfficialPluginSourceEntry,
    OfficialPluginSourcePort,
};
use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::{SigningKey, pkcs8::EncodePublicKey};
use flate2::{Compression, write::GzEncoder};
use runtime_profile::{
    RuntimeCpu, RuntimeCpuMetrics, RuntimeDiskIoMetrics, RuntimeMemory, RuntimeMemoryMetrics,
    RuntimeMetricAvailability, RuntimeMetricScopeKind, RuntimeMetricsSnapshot,
    RuntimeNetworkMetrics, RuntimePlatform, RuntimeProfile, RuntimeStorageMetrics,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use tar::Builder;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    config::ApiConfig,
    host_infrastructure::build_local_host_infrastructure,
    official_extension_catalog::{
        DownloadedOfficialExtensionArtifact, LocatedOfficialExtensionCatalogEntry,
        OfficialExtensionArtifactDescriptor, OfficialExtensionArtifactPlatform,
        OfficialExtensionCatalogEntry, OfficialExtensionCatalogEntrySource,
        OfficialExtensionCatalogFreshness, OfficialExtensionCatalogPage,
        OfficialExtensionCatalogPageMetadata, OfficialExtensionCatalogSearchQuery,
        OfficialExtensionCatalogSearchResult, OfficialExtensionCatalogSourcePort,
    },
    official_mcp_bundles::{
        DownloadedOfficialMcpBundle, McpBundleLibraryCatalog, McpBundleLibraryEntry,
        OfficialMcpBundleCatalogEntry, OfficialMcpBundleCatalogSnapshot,
        OfficialMcpBundleCatalogSource, OfficialMcpBundleSourcePort,
    },
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
    runtime_profile_client::{
        ApiRuntimeProfilePort, HostApiRuntimeProfileCollector, PluginRunnerSystemPort,
    },
};

pub(crate) use applications::*;
pub(crate) use auth::*;
pub(crate) use packages::*;
