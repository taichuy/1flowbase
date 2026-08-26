/// A trusted signing key descriptor consumed by package intake implementations.
#[derive(Debug, Clone)]
pub struct TrustedPublicKey {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_pem: String,
}
