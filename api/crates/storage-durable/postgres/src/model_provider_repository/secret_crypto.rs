use super::*;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305,
};
use sha2::{Digest, Sha256};

const AEAD_ALGORITHM: &str = "aead_xchacha20poly1305_v1";
const LEGACY_XOR_ALGORITHM: &str = "xor_v1";

pub(crate) fn encrypt_secret_json(secret_json: &Value, master_key: &str) -> Result<Value> {
    if master_key.is_empty() {
        bail!(ControlPlaneError::InvalidInput(
            "provider_secret_master_key"
        ));
    }

    let plaintext = serde_json::to_vec(secret_json)?;
    let cipher = XChaCha20Poly1305::new(&derive_aead_key(master_key));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|_| anyhow!("failed to encrypt provider secret"))?;

    Ok(json!({
        "algorithm": AEAD_ALGORITHM,
        "nonce": hex_encode(&nonce),
        "ciphertext": hex_encode(&ciphertext),
    }))
}

pub(crate) fn decrypt_secret_json(
    encrypted_secret_json: &Value,
    master_key: &str,
) -> Result<Value> {
    if master_key.is_empty() {
        bail!(ControlPlaneError::InvalidInput(
            "provider_secret_master_key"
        ));
    }

    let algorithm = encrypted_secret_json
        .get("algorithm")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing secret encryption algorithm"))?;
    if algorithm == LEGACY_XOR_ALGORITHM {
        let ciphertext = encrypted_secret_json
            .get("ciphertext")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing secret ciphertext"))?;
        let plaintext = xor_hex_decode(ciphertext, master_key.as_bytes())?;
        return Ok(serde_json::from_slice(&plaintext)?);
    }

    if algorithm != AEAD_ALGORITHM {
        bail!(anyhow!(
            "unsupported secret encryption algorithm: {algorithm}"
        ));
    }
    let nonce = encrypted_secret_json
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing secret nonce"))
        .and_then(hex_decode)?;
    if nonce.len() != 24 {
        bail!(anyhow!("invalid secret nonce length"));
    }
    let nonce = chacha20poly1305::XNonce::from_slice(&nonce);
    let ciphertext = encrypted_secret_json
        .get("ciphertext")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing secret ciphertext"))?;
    let ciphertext = hex_decode(ciphertext)?;
    let cipher = XChaCha20Poly1305::new(&derive_aead_key(master_key));
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| anyhow!("invalid secret ciphertext"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

fn derive_aead_key(master_key: &str) -> chacha20poly1305::Key {
    let digest = Sha256::digest(master_key.as_bytes());
    *chacha20poly1305::Key::from_slice(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn hex_decode(ciphertext: &str) -> Result<Vec<u8>> {
    if !ciphertext.len().is_multiple_of(2) {
        bail!(anyhow!("invalid secret ciphertext length"));
    }

    let mut decoded = Vec::with_capacity(ciphertext.len() / 2);
    for chunk in ciphertext.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(chunk)?;
        decoded.push(u8::from_str_radix(pair, 16)?);
    }

    Ok(decoded)
}

#[cfg(test)]
fn xor_hex(bytes: &[u8], key: &[u8]) -> String {
    bytes
        .iter()
        .enumerate()
        .map(|(index, byte)| format!("{:02x}", byte ^ key[index % key.len()]))
        .collect::<String>()
}

pub(super) fn xor_hex_decode(ciphertext: &str, key: &[u8]) -> Result<Vec<u8>> {
    let encrypted = hex_decode(ciphertext)?;

    Ok(encrypted
        .into_iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn encrypted_secret_uses_aead_with_random_nonce() {
        let secret = json!({"api_key": "sk-live", "base_url": "https://example.test"});
        let first = encrypt_secret_json(&secret, "strong-provider-secret-master-key").unwrap();
        let second = encrypt_secret_json(&secret, "strong-provider-secret-master-key").unwrap();

        assert_eq!(first["algorithm"], "aead_xchacha20poly1305_v1");
        assert_eq!(second["algorithm"], "aead_xchacha20poly1305_v1");
        assert_ne!(first["nonce"], second["nonce"]);
        assert_ne!(first["ciphertext"], second["ciphertext"]);
        assert_eq!(
            decrypt_secret_json(&first, "strong-provider-secret-master-key").unwrap(),
            secret
        );
    }

    #[test]
    fn encrypted_secret_rejects_tampered_ciphertext() {
        let secret = json!({"api_key": "sk-live"});
        let mut encrypted =
            encrypt_secret_json(&secret, "strong-provider-secret-master-key").unwrap();
        let ciphertext = encrypted["ciphertext"].as_str().unwrap();
        let replacement = if ciphertext.ends_with('0') { "1" } else { "0" };
        encrypted["ciphertext"] = json!(format!(
            "{}{replacement}",
            &ciphertext[..ciphertext.len() - 1]
        ));

        let error = decrypt_secret_json(&encrypted, "strong-provider-secret-master-key")
            .expect_err("tampered ciphertext must fail authentication");

        assert!(error.to_string().contains("secret ciphertext"));
    }

    #[test]
    fn decrypt_secret_keeps_legacy_xor_v1_readable() {
        let secret = json!({"api_key": "legacy"});
        let legacy = json!({
            "algorithm": "xor_v1",
            "ciphertext": xor_hex(&serde_json::to_vec(&secret).unwrap(), b"legacy-key")
        });

        assert_eq!(decrypt_secret_json(&legacy, "legacy-key").unwrap(), secret);
    }
}
