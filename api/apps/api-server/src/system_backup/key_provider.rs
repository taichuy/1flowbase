use async_trait::async_trait;
use control_plane::ports::{BackupKeyMaterial, BackupKeyProvider, BackupKeyProviderError};
use domain::KeyFingerprint;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct EnvironmentBackupKeyProvider {
    fingerprint: KeyFingerprint,
    key: [u8; 32],
}

impl EnvironmentBackupKeyProvider {
    pub fn from_master_key(value: &str) -> Result<Self, BackupKeyProviderError> {
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
        })
    }

    fn key_material(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        BackupKeyMaterial::new(self.fingerprint.clone(), self.key.to_vec())
            .ok_or(BackupKeyProviderError::Unavailable)
    }
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
            return Err(BackupKeyProviderError::NotFound);
        }
        self.key_material()
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentBackupKeyProvider;

    #[test]
    fn derives_a_stable_backup_key_from_the_provider_master_key() {
        let first = EnvironmentBackupKeyProvider::from_master_key("master-key").unwrap();
        let second = EnvironmentBackupKeyProvider::from_master_key("master-key").unwrap();
        let other = EnvironmentBackupKeyProvider::from_master_key("other-key").unwrap();

        assert_eq!(first.key, second.key);
        assert_ne!(first.key, other.key);
    }
}
