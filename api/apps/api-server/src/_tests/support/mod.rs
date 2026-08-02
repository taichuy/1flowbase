mod applications;
mod auth;
mod packages;
mod plugins;

use std::{fs, io::Write, path::Path, sync::Arc};

use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::bootstrap::{BootstrapConfig, BootstrapService};
use control_plane::ports::{
    DownloadedOfficialPluginPackage, OfficialPluginArtifact, OfficialPluginCatalogSnapshot,
    OfficialPluginCatalogSource, OfficialPluginI18nSummary, OfficialPluginSourceEntry,
    OfficialPluginSourcePort,
};
use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::{pkcs8::EncodePublicKey, SigningKey};
use flate2::{write::GzEncoder, Compression};
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
        OfficialExtensionCatalogPageMetadata, OfficialExtensionCatalogSourcePort,
    },
    official_mcp_bundles::{
        DownloadedOfficialMcpBundle, OfficialMcpBundleCatalogEntry,
        OfficialMcpBundleCatalogSnapshot, OfficialMcpBundleCatalogSource,
        OfficialMcpBundleSourcePort,
    },
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
    runtime_profile_client::{
        ApiRuntimeProfilePort, HostApiRuntimeProfileCollector, PluginRunnerSystemPort,
    },
};

pub(crate) use applications::*;
pub(crate) use auth::*;
pub(crate) use packages::*;
