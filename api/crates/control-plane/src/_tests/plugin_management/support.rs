use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
    sync::Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
use ed25519_dalek::{pkcs8::EncodePublicKey, Signer, SigningKey};
use flate2::{write::GzEncoder, Compression};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tar::Builder;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    i18n::RequestedLocales,
    ports::{
        AuthRepository, CommitPluginInstallationProjectionInput, CreateModelProviderInstanceInput,
        CreatePluginAssignmentInput, CreatePluginTaskInput, DownloadedOfficialPluginPackage,
        FrontendBlockCatalogRepository, HostInfrastructureConfigRepository, JsDependencyRepository,
        ModelProviderRepository, NodeContributionRepository, OfficialPluginArtifact,
        OfficialPluginCatalogSnapshot, OfficialPluginCatalogSource, OfficialPluginI18nSummary,
        OfficialPluginSourceEntry, OfficialPluginSourcePort, PluginRepository,
        ProviderRuntimeInvocationOutput, ProviderRuntimePort, ReassignModelProviderInstancesInput,
        ReplaceInstallationFrontendBlocksInput, ReplaceInstallationJsDependenciesInput,
        ReplaceInstallationNodeContributionsInput, UpdateModelProviderInstanceInput,
        UpdatePluginArtifactSnapshotInput, UpdatePluginDesiredStateInput,
        UpdatePluginRuntimeSnapshotInput, UpdatePluginTaskStatusInput, UpdateProfileInput,
        UpsertHostInfrastructureProviderConfigInput, UpsertModelProviderCatalogCacheInput,
        UpsertModelProviderSecretInput, UpsertPluginArtifactInstanceInput,
        UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
    },
};
use domain::{
    ActorContext, AuditLogRecord, AuthenticatorRecord, HostInfrastructureProviderConfigRecord,
    ModelProviderCatalogCacheRecord, ModelProviderCatalogRefreshStatus, ModelProviderCatalogSource,
    ModelProviderDiscoveryMode, ModelProviderInstanceRecord, ModelProviderInstanceStatus,
    ModelProviderSecretRecord, NodeContributionDependencyStatus, PermissionDefinition,
    PluginArtifactInstanceRecord, PluginArtifactStatus, PluginAssignmentRecord,
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord,
    PluginPackageCatalogProjectionRecord, PluginPackageCatalogProjectionStatus,
    PluginRuntimeStatus, PluginTaskRecord, PluginTaskStatus, ScopeContext, UserRecord,
};
use plugin_framework::provider_contract::{
    ProviderInvocationInput, ProviderInvocationResult, ProviderModelDescriptor,
};
use time::OffsetDateTime;

#[path = "support/fixtures.rs"]
mod fixtures;
#[path = "support/repository.rs"]
mod repository;
#[path = "support/source.rs"]
mod source;

pub(crate) use fixtures::{
    actor_with_permissions, build_capability_plugin_package_bytes,
    create_capability_plugin_fixture, create_frontend_block_fixture,
    create_js_dependency_pack_fixture, create_provider_fixture,
};
pub(super) use fixtures::{
    build_openai_compatible_package_bytes, build_signed_openai_upload_package,
    create_provider_fixture_with_node_contribution, seed_test_installation,
};
pub(crate) use repository::MemoryPluginManagementRepository;
pub(super) use source::{requested_locales, sample_artifact, sample_i18n_summary};
pub(crate) use source::{MemoryOfficialPluginSource, MemoryProviderRuntime};
