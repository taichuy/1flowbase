use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use control_plane::ports::{BackupKeyMaterial, BackupKeyProvider, BackupKeyProviderError};
use domain::KeyFingerprint;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct EnvironmentBackupKeyProvider {
    fingerprint: KeyFingerprint,
    key: [u8; 32],
}

impl EnvironmentBackupKeyProvider {
    pub fn from_base64(value: &str) -> Result<Self, BackupKeyProviderError> {
        let decoded = STANDARD
            .decode(value.trim())
            .map_err(|_| BackupKeyProviderError::Unavailable)?;
        let key = <[u8; 32]>::try_from(decoded).map_err(|_| BackupKeyProviderError::Unavailable)?;
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
