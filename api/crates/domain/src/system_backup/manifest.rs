use std::{collections::BTreeSet, error::Error, fmt};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::{
    ApplicationBuild, BackupComponentId, BackupSetId, BackupSourceIdentity, ContentDigest,
    KeyFingerprint, MigrationHead,
};

pub const SYSTEM_BACKUP_FORMAT_VERSION: u32 = 1;
pub const SYSTEM_BACKUP_CHUNK_SIZE_BYTES: u32 = 4 * 1024 * 1024;
pub const SYSTEM_BACKUP_MAX_PARALLEL_STREAMS: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupComponentKind {
    PostgreSql,
    BusinessObject,
    ExtensionArtifact,
    McpArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupComponentDisposition {
    Embedded,
    IdentityOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRebuildability {
    Rebuildable,
    NonRebuildable,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BackupComponent {
    pub component_id: BackupComponentId,
    pub kind: BackupComponentKind,
    pub source_identity: BackupSourceIdentity,
    pub content_type: String,
    pub size_bytes: u64,
    pub content_digest: ContentDigest,
    pub disposition: BackupComponentDisposition,
    pub rebuildability: ArtifactRebuildability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackupExcludedDomain {
    ExternalDataSourceContent,
    EphemeralState,
    DeploymentEnvironment,
    TlsMaterial,
    ContainerImages,
    BackupRepository,
}

impl BackupExcludedDomain {
    pub const ALL: [Self; 6] = [
        Self::ExternalDataSourceContent,
        Self::EphemeralState,
        Self::DeploymentEnvironment,
        Self::TlsMaterial,
        Self::ContainerImages,
        Self::BackupRepository,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BackupManifestWire {
    format_version: u32,
    backup_set_id: BackupSetId,
    created_at: OffsetDateTime,
    application_build: ApplicationBuild,
    migration_head: MigrationHead,
    master_key_fingerprint: KeyFingerprint,
    backup_key_fingerprint: KeyFingerprint,
    components: Vec<BackupComponent>,
    excluded_domains: BTreeSet<BackupExcludedDomain>,
    total_size_bytes: u64,
    envelope_digest: ContentDigest,
    chunk_size_bytes: u32,
    max_parallel_streams: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BackupManifestWire", into = "BackupManifestWire")]
pub struct BackupManifest {
    format_version: u32,
    backup_set_id: BackupSetId,
    created_at: OffsetDateTime,
    application_build: ApplicationBuild,
    migration_head: MigrationHead,
    master_key_fingerprint: KeyFingerprint,
    backup_key_fingerprint: KeyFingerprint,
    components: Vec<BackupComponent>,
    excluded_domains: BTreeSet<BackupExcludedDomain>,
    total_size_bytes: u64,
    envelope_digest: ContentDigest,
    chunk_size_bytes: u32,
    max_parallel_streams: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupManifestError {
    UnsupportedFormatVersion,
    MissingPostgreSqlComponent,
    DuplicateComponentId,
    InvalidComponentDisposition,
    InvalidComponentMetadata,
    IncompleteExclusionContract,
    SizeMismatch,
    InvalidStreamingLimits,
}

impl fmt::Display for BackupManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid backup manifest: {self:?}")
    }
}

impl Error for BackupManifestError {}

impl From<BackupManifest> for BackupManifestWire {
    fn from(manifest: BackupManifest) -> Self {
        Self {
            format_version: manifest.format_version,
            backup_set_id: manifest.backup_set_id,
            created_at: manifest.created_at,
            application_build: manifest.application_build,
            migration_head: manifest.migration_head,
            master_key_fingerprint: manifest.master_key_fingerprint,
            backup_key_fingerprint: manifest.backup_key_fingerprint,
            components: manifest.components,
            excluded_domains: manifest.excluded_domains,
            total_size_bytes: manifest.total_size_bytes,
            envelope_digest: manifest.envelope_digest,
            chunk_size_bytes: manifest.chunk_size_bytes,
            max_parallel_streams: manifest.max_parallel_streams,
        }
    }
}

impl TryFrom<BackupManifestWire> for BackupManifest {
    type Error = BackupManifestError;

    fn try_from(wire: BackupManifestWire) -> Result<Self, Self::Error> {
        Self::try_from_parts(Self {
            format_version: wire.format_version,
            backup_set_id: wire.backup_set_id,
            created_at: wire.created_at,
            application_build: wire.application_build,
            migration_head: wire.migration_head,
            master_key_fingerprint: wire.master_key_fingerprint,
            backup_key_fingerprint: wire.backup_key_fingerprint,
            components: wire.components,
            excluded_domains: wire.excluded_domains,
            total_size_bytes: wire.total_size_bytes,
            envelope_digest: wire.envelope_digest,
            chunk_size_bytes: wire.chunk_size_bytes,
            max_parallel_streams: wire.max_parallel_streams,
        })
    }
}

impl BackupManifest {
    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    pub const fn backup_set_id(&self) -> BackupSetId {
        self.backup_set_id
    }

    pub const fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn application_build(&self) -> &ApplicationBuild {
        &self.application_build
    }

    pub fn migration_head(&self) -> &MigrationHead {
        &self.migration_head
    }

    pub fn master_key_fingerprint(&self) -> &KeyFingerprint {
        &self.master_key_fingerprint
    }

    pub fn backup_key_fingerprint(&self) -> &KeyFingerprint {
        &self.backup_key_fingerprint
    }

    pub fn components(&self) -> &[BackupComponent] {
        &self.components
    }

    pub fn excluded_domains(&self) -> &BTreeSet<BackupExcludedDomain> {
        &self.excluded_domains
    }

    pub const fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }

    pub fn envelope_digest(&self) -> &ContentDigest {
        &self.envelope_digest
    }

    pub const fn chunk_size_bytes(&self) -> u32 {
        self.chunk_size_bytes
    }

    pub const fn max_parallel_streams(&self) -> u16 {
        self.max_parallel_streams
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        backup_set_id: BackupSetId,
        created_at: OffsetDateTime,
        application_build: ApplicationBuild,
        migration_head: MigrationHead,
        master_key_fingerprint: KeyFingerprint,
        backup_key_fingerprint: KeyFingerprint,
        components: Vec<BackupComponent>,
        total_size_bytes: u64,
        envelope_digest: ContentDigest,
    ) -> Result<Self, BackupManifestError> {
        Self::try_from_parts(Self {
            format_version: SYSTEM_BACKUP_FORMAT_VERSION,
            backup_set_id,
            created_at,
            application_build,
            migration_head,
            master_key_fingerprint,
            backup_key_fingerprint,
            components,
            excluded_domains: BackupExcludedDomain::ALL.into_iter().collect(),
            total_size_bytes,
            envelope_digest,
            chunk_size_bytes: SYSTEM_BACKUP_CHUNK_SIZE_BYTES,
            max_parallel_streams: SYSTEM_BACKUP_MAX_PARALLEL_STREAMS,
        })
    }

    pub fn try_from_parts(manifest: Self) -> Result<Self, BackupManifestError> {
        if manifest.format_version != SYSTEM_BACKUP_FORMAT_VERSION {
            return Err(BackupManifestError::UnsupportedFormatVersion);
        }
        if manifest.chunk_size_bytes == 0 || manifest.max_parallel_streams == 0 {
            return Err(BackupManifestError::InvalidStreamingLimits);
        }
        if manifest.excluded_domains
            != BackupExcludedDomain::ALL
                .into_iter()
                .collect::<BTreeSet<_>>()
        {
            return Err(BackupManifestError::IncompleteExclusionContract);
        }
        let mut component_ids = BTreeSet::new();
        let mut has_postgres = false;
        let mut component_size = 0_u64;
        for component in &manifest.components {
            if !component_ids.insert(component.component_id.clone()) {
                return Err(BackupManifestError::DuplicateComponentId);
            }
            if component.content_type.trim().is_empty()
                || (component.disposition == BackupComponentDisposition::Embedded
                    && component.size_bytes == 0)
            {
                return Err(BackupManifestError::InvalidComponentMetadata);
            }
            match component.kind {
                BackupComponentKind::PostgreSql | BackupComponentKind::BusinessObject => {
                    if component.kind == BackupComponentKind::PostgreSql {
                        has_postgres = true;
                    }
                    if component.disposition != BackupComponentDisposition::Embedded
                        || component.rebuildability != ArtifactRebuildability::NotApplicable
                    {
                        return Err(BackupManifestError::InvalidComponentDisposition);
                    }
                }
                BackupComponentKind::ExtensionArtifact | BackupComponentKind::McpArtifact => {
                    if component.rebuildability == ArtifactRebuildability::NotApplicable {
                        return Err(BackupManifestError::InvalidComponentDisposition);
                    }
                }
            }
            if component.rebuildability == ArtifactRebuildability::NonRebuildable
                && component.disposition != BackupComponentDisposition::Embedded
            {
                return Err(BackupManifestError::InvalidComponentDisposition);
            }
            if component.rebuildability == ArtifactRebuildability::Rebuildable
                && component.disposition != BackupComponentDisposition::IdentityOnly
            {
                return Err(BackupManifestError::InvalidComponentDisposition);
            }
            component_size = component_size
                .checked_add(component.size_bytes)
                .ok_or(BackupManifestError::SizeMismatch)?;
        }
        if !has_postgres {
            return Err(BackupManifestError::MissingPostgreSqlComponent);
        }
        if component_size != manifest.total_size_bytes {
            return Err(BackupManifestError::SizeMismatch);
        }
        Ok(manifest)
    }
}
