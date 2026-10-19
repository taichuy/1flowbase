//! Connection-bound AEAD for authentication-provider configuration.
use control_plane_contracts::ports::AuthenticationSecretCodec;
use serde_json::Value;
use uuid::Uuid;

pub struct AuthenticationSecretCipher {
    master_key: String,
}

impl AuthenticationSecretCipher {
    pub fn new(master_key: impl Into<String>) -> Self {
        Self {
            master_key: master_key.into(),
        }
    }
    fn aad(connection_id: Uuid) -> Vec<u8> {
        format!("authentication-connection/secrets/v1:{connection_id}").into_bytes()
    }
}

impl AuthenticationSecretCodec for AuthenticationSecretCipher {
    fn seal(&self, connection_id: Uuid, secrets: &Value) -> anyhow::Result<Value> {
        super::encrypt_secret_json_with_aad(secrets, &self.master_key, &Self::aad(connection_id))
    }
    fn open(&self, connection_id: Uuid, envelope: &Value) -> anyhow::Result<Value> {
        super::decrypt_secret_json_with_aad(envelope, &self.master_key, &Self::aad(connection_id))
    }
}

#[cfg(test)]
#[path = "_tests/authentication.rs"]
mod tests;
