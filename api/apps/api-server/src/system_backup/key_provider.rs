use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use control_plane::ports::{BackupKeyMaterial, BackupKeyProvider, BackupKeyProviderError};
use domain::KeyFingerprint;
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct LegacyBackupKey {
    fingerprint: KeyFingerprint,
    key: [u8; 32],
}

#[derive(Clone)]
pub struct EnvironmentBackupKeyProvider {
    fingerprint: KeyFingerprint,
    key: [u8; 32],
    legacy_keys: Vec<LegacyBackupKey>,
}

impl EnvironmentBackupKeyProvider {
    pub fn from_master_key(value: &str) -> Result<Self, BackupKeyProviderError> {
        Self::from_master_key_with_legacy(value, None)
    }

    pub fn from_master_key_with_legacy(
        value: &str,
        legacy_key_base64: Option<&str>,
    ) -> Result<Self, BackupKeyProviderError> {
        let mut derivation = Sha256::new();
        derivation.update(b"1flowbase/system-backup/key/v1\0");
        derivation.update(value.as_bytes());
        let key = derivation.finalize();
        let key = <[u8; 32]>::try_from(key.as_slice())
            .map_err(|_| BackupKeyProviderError::Unavailable)?;
        let fingerprint = Sha256::digest(key)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(Self {
            fingerprint: KeyFingerprint::try_from(fingerprint)
                .map_err(|_| BackupKeyProviderError::Unavailable)?,
            key,
            legacy_keys: legacy_key_base64
                .map(parse_legacy_key)
                .transpose()?
                .into_iter()
                .collect(),
        })
    }

    fn key_material(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        BackupKeyMaterial::new(self.fingerprint.clone(), self.key.to_vec())
            .ok_or(BackupKeyProviderError::Unavailable)
    }

    fn legacy_key_material(
        legacy: &LegacyBackupKey,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        BackupKeyMaterial::new(legacy.fingerprint.clone(), legacy.key.to_vec())
            .ok_or(BackupKeyProviderError::Unavailable)
    }
}

fn parse_legacy_key(value: &str) -> Result<LegacyBackupKey, BackupKeyProviderError> {
    let decoded = STANDARD
        .decode(value.trim())
        .map_err(|_| BackupKeyProviderError::Unavailable)?;
    let key = <[u8; 32]>::try_from(decoded.as_slice())
        .map_err(|_| BackupKeyProviderError::Unavailable)?;
    let fingerprint = Sha256::digest(key)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(LegacyBackupKey {
        fingerprint: KeyFingerprint::try_from(fingerprint)
            .map_err(|_| BackupKeyProviderError::Unavailable)?,
        key,
    })
}

#[async_trait]
impl BackupKeyProvider for EnvironmentBackupKeyProvider {
    async fn active_key(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        self.key_material()
    }

    async fn key_for(
        &self,
        fingerprint: &KeyFingerprint,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        if fingerprint != &self.fingerprint {
            return self
                .legacy_keys
                .iter()
                .find(|legacy| &legacy.fingerprint == fingerprint)
                .map(Self::legacy_key_material)
                .unwrap_or(Err(BackupKeyProviderError::NotFound));
        }
        self.key_material()
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentBackupKeyProvider;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use control_plane::{
        ports::BackupKeyProvider,
        system_backup::{authenticate_backup_manifest, verify_backup_manifest},
    };
    use domain::{
        ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
        BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupManifest,
        BackupSetId, BackupSourceIdentity, ContentDigest, KeyFingerprint, MigrationHead,
    };
    use time::OffsetDateTime;

    #[test]
    fn derives_a_stable_backup_key_from_the_provider_master_key() {
        let first = EnvironmentBackupKeyProvider::from_master_key("master-key").unwrap();
        let second = EnvironmentBackupKeyProvider::from_master_key("master-key").unwrap();
        let other = EnvironmentBackupKeyProvider::from_master_key("other-key").unwrap();

        assert_eq!(first.key, second.key);
        assert_ne!(first.key, other.key);
    }

    #[tokio::test]
    async fn legacy_key_is_readable_by_fingerprint_but_never_active() {
        let legacy_key = [9_u8; 32];
        let provider = EnvironmentBackupKeyProvider::from_master_key_with_legacy(
            "master-key",
            Some(&STANDARD.encode(legacy_key)),
        )
        .unwrap();
        let active = provider.active_key().await.unwrap();
        let legacy_fingerprint = super::parse_legacy_key(&STANDARD.encode(legacy_key))
            .unwrap()
            .fingerprint;
        let legacy = provider.key_for(&legacy_fingerprint).await.unwrap();

        assert_ne!(active.fingerprint(), legacy.fingerprint());
        assert_ne!(active.expose_bytes(), legacy.expose_bytes());
        assert_eq!(legacy.expose_bytes(), legacy_key);
    }

    #[tokio::test]
    async fn unknown_fingerprint_does_not_fallback_to_active_or_legacy_key() {
        let provider = EnvironmentBackupKeyProvider::from_master_key_with_legacy(
            "master-key",
            Some(&STANDARD.encode([9_u8; 32])),
        )
        .unwrap();
        let unknown = domain::KeyFingerprint::try_from("f".repeat(64)).unwrap();

        assert!(matches!(
            provider.key_for(&unknown).await,
            Err(control_plane::ports::BackupKeyProviderError::NotFound)
        ));
    }

    #[tokio::test]
    async fn pre_upgrade_sealed_manifest_verifies_through_legacy_fingerprint_route() {
        let legacy_key = [9_u8; 32];
        let provider = EnvironmentBackupKeyProvider::from_master_key_with_legacy(
            "master-key",
            Some(&STANDARD.encode(legacy_key)),
        )
        .unwrap();
        let legacy_fingerprint = super::parse_legacy_key(&STANDARD.encode(legacy_key))
            .unwrap()
            .fingerprint;
        let legacy_material = provider.key_for(&legacy_fingerprint).await.unwrap();
        let manifest = BackupManifest::try_new(
            BackupSetId::new(),
            OffsetDateTime::UNIX_EPOCH,
            ApplicationBuild::try_from("pre-upgrade.fixture").unwrap(),
            MigrationHead::try_from("migration.fixture").unwrap(),
            KeyFingerprint::try_from("b".repeat(64)).unwrap(),
            legacy_fingerprint.clone(),
            vec![BackupComponent {
                component_id: BackupComponentId::try_from("postgres/main").unwrap(),
                kind: BackupComponentKind::PostgreSql,
                source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
                content_type: "application/octet-stream".to_owned(),
                size_bytes: 1,
                content_digest: ContentDigest::try_from("d".repeat(64)).unwrap(),
                disposition: BackupComponentDisposition::Embedded,
                rebuildability: ArtifactRebuildability::NotApplicable,
                restore_target: BackupComponentRestoreTarget::PostgreSql,
            }],
            1,
            ContentDigest::try_from("e".repeat(64)).unwrap(),
        )
        .unwrap();
        let sealed = authenticate_backup_manifest(manifest, &legacy_material).unwrap();

        verify_backup_manifest(&sealed, &legacy_material).unwrap();
        assert_eq!(
            sealed.manifest().backup_key_fingerprint(),
            &legacy_fingerprint
        );
        assert_ne!(
            provider.active_key().await.unwrap().expose_bytes(),
            &legacy_key
        );
    }
}
