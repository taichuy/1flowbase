use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use domain::{
    BackupComponentId, BackupManifest, BackupSetId, ContentDigest, ManifestAuthenticationTag,
    SealedBackupManifest, SYSTEM_BACKUP_CHUNK_SIZE_BYTES,
};
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::ports::BackupKeyMaterial;

const ENVELOPE_MAGIC: &[u8; 8] = b"1FBKENC1";
const NONCE_PREFIX_BYTES: usize = 16;
const TAG_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedStreamReceipt {
    pub plaintext_size_bytes: u64,
    pub encrypted_size_bytes: u64,
    pub plaintext_digest: ContentDigest,
    pub envelope_digest: ContentDigest,
}

#[derive(Debug, Error)]
pub enum BackupEnvelopeError {
    #[error("backup key must contain exactly 32 bytes")]
    InvalidKey,
    #[error("backup envelope header is invalid")]
    InvalidHeader,
    #[error("backup envelope is truncated")]
    Truncated,
    #[error("backup envelope authentication failed")]
    Authentication,
    #[error("backup envelope chunk length is invalid")]
    InvalidChunk,
    #[error("backup manifest serialization failed")]
    ManifestSerialization,
    #[error("backup envelope I/O failed")]
    Io(#[from] std::io::Error),
}

pub fn authenticate_backup_manifest(
    manifest: BackupManifest,
    key: &BackupKeyMaterial,
) -> Result<SealedBackupManifest, BackupEnvelopeError> {
    let bytes =
        serde_json::to_vec(&manifest).map_err(|_| BackupEnvelopeError::ManifestSerialization)?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.expose_bytes())
        .map_err(|_| BackupEnvelopeError::InvalidKey)?;
    mac.update(&bytes);
    let tag = mac.finalize().into_bytes();
    let tag = tag
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let tag = ManifestAuthenticationTag::try_from(tag)
        .map_err(|_| BackupEnvelopeError::ManifestSerialization)?;
    Ok(SealedBackupManifest::new(manifest, tag))
}

pub fn verify_backup_manifest(
    sealed: &SealedBackupManifest,
    key: &BackupKeyMaterial,
) -> Result<(), BackupEnvelopeError> {
    let bytes = serde_json::to_vec(sealed.manifest())
        .map_err(|_| BackupEnvelopeError::ManifestSerialization)?;
    let expected = decode_hex(sealed.authentication_tag().as_str())?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.expose_bytes())
        .map_err(|_| BackupEnvelopeError::InvalidKey)?;
    mac.update(&bytes);
    mac.verify_slice(&expected)
        .map_err(|_| BackupEnvelopeError::Authentication)
}

pub async fn encrypt_backup_stream<R, W>(
    mut reader: R,
    mut writer: W,
    key: &BackupKeyMaterial,
    backup_set_id: BackupSetId,
    component_id: &BackupComponentId,
) -> Result<EncryptedStreamReceipt, BackupEnvelopeError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let cipher = cipher(key)?;
    let mut nonce_prefix = [0_u8; NONCE_PREFIX_BYTES];
    OsRng.fill_bytes(&mut nonce_prefix);

    let mut envelope_hasher = Sha256::new();
    write_hashed(&mut writer, &mut envelope_hasher, ENVELOPE_MAGIC).await?;
    write_hashed(
        &mut writer,
        &mut envelope_hasher,
        &SYSTEM_BACKUP_CHUNK_SIZE_BYTES.to_le_bytes(),
    )
    .await?;
    write_hashed(&mut writer, &mut envelope_hasher, &nonce_prefix).await?;

    let mut plaintext_hasher = Sha256::new();
    let mut plaintext_size = 0_u64;
    let mut encrypted_size = (ENVELOPE_MAGIC.len() + size_of::<u32>() + NONCE_PREFIX_BYTES) as u64;
    let mut chunk_index = 0_u64;
    let mut buffer = vec![0_u8; SYSTEM_BACKUP_CHUNK_SIZE_BYTES as usize];

    loop {
        let read = read_chunk(&mut reader, &mut buffer).await?;
        if read == 0 {
            break;
        }
        let plaintext = &buffer[..read];
        plaintext_hasher.update(plaintext);
        plaintext_size = plaintext_size
            .checked_add(read as u64)
            .ok_or(BackupEnvelopeError::InvalidChunk)?;
        let ciphertext = cipher
            .encrypt(
                &nonce(&nonce_prefix, chunk_index),
                Payload {
                    msg: plaintext,
                    aad: &associated_data(backup_set_id, component_id, chunk_index, false),
                },
            )
            .map_err(|_| BackupEnvelopeError::Authentication)?;
        write_ciphertext(&mut writer, &mut envelope_hasher, &ciphertext).await?;
        encrypted_size = encrypted_size
            .checked_add(size_of::<u32>() as u64 + ciphertext.len() as u64)
            .ok_or(BackupEnvelopeError::InvalidChunk)?;
        chunk_index = chunk_index
            .checked_add(1)
            .ok_or(BackupEnvelopeError::InvalidChunk)?;
    }

    let terminal = cipher
        .encrypt(
            &nonce(&nonce_prefix, chunk_index),
            Payload {
                msg: &[],
                aad: &associated_data(backup_set_id, component_id, chunk_index, true),
            },
        )
        .map_err(|_| BackupEnvelopeError::Authentication)?;
    write_ciphertext(&mut writer, &mut envelope_hasher, &terminal).await?;
    encrypted_size += size_of::<u32>() as u64 + terminal.len() as u64;
    writer.flush().await?;

    Ok(EncryptedStreamReceipt {
        plaintext_size_bytes: plaintext_size,
        encrypted_size_bytes: encrypted_size,
        plaintext_digest: digest(plaintext_hasher.finalize()),
        envelope_digest: digest(envelope_hasher.finalize()),
    })
}

pub async fn decrypt_backup_stream<R, W>(
    mut reader: R,
    mut writer: W,
    key: &BackupKeyMaterial,
    backup_set_id: BackupSetId,
    component_id: &BackupComponentId,
) -> Result<EncryptedStreamReceipt, BackupEnvelopeError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let cipher = cipher(key)?;
    let mut header = [0_u8; 8];
    reader.read_exact(&mut header).await.map_err(header_error)?;
    if &header != ENVELOPE_MAGIC {
        return Err(BackupEnvelopeError::InvalidHeader);
    }
    let mut chunk_size = [0_u8; 4];
    reader
        .read_exact(&mut chunk_size)
        .await
        .map_err(header_error)?;
    if u32::from_le_bytes(chunk_size) != SYSTEM_BACKUP_CHUNK_SIZE_BYTES {
        return Err(BackupEnvelopeError::InvalidHeader);
    }
    let mut nonce_prefix = [0_u8; NONCE_PREFIX_BYTES];
    reader
        .read_exact(&mut nonce_prefix)
        .await
        .map_err(header_error)?;

    let mut envelope_hasher = Sha256::new();
    envelope_hasher.update(header);
    envelope_hasher.update(chunk_size);
    envelope_hasher.update(nonce_prefix);
    let mut plaintext_hasher = Sha256::new();
    let mut plaintext_size = 0_u64;
    let mut encrypted_size = (header.len() + chunk_size.len() + nonce_prefix.len()) as u64;
    let mut chunk_index = 0_u64;

    loop {
        let mut length_bytes = [0_u8; 4];
        reader
            .read_exact(&mut length_bytes)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::UnexpectedEof => BackupEnvelopeError::Truncated,
                _ => BackupEnvelopeError::Io(error),
            })?;
        let length = u32::from_le_bytes(length_bytes) as usize;
        if !(TAG_BYTES..=(SYSTEM_BACKUP_CHUNK_SIZE_BYTES as usize + TAG_BYTES)).contains(&length) {
            return Err(BackupEnvelopeError::InvalidChunk);
        }
        let mut ciphertext = vec![0_u8; length];
        reader
            .read_exact(&mut ciphertext)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::UnexpectedEof => BackupEnvelopeError::Truncated,
                _ => BackupEnvelopeError::Io(error),
            })?;
        envelope_hasher.update(length_bytes);
        envelope_hasher.update(&ciphertext);
        encrypted_size += length_bytes.len() as u64 + ciphertext.len() as u64;

        let terminal = length == TAG_BYTES;
        let plaintext = cipher
            .decrypt(
                &nonce(&nonce_prefix, chunk_index),
                Payload {
                    msg: &ciphertext,
                    aad: &associated_data(backup_set_id, component_id, chunk_index, terminal),
                },
            )
            .map_err(|_| BackupEnvelopeError::Authentication)?;
        if terminal {
            if !plaintext.is_empty() {
                return Err(BackupEnvelopeError::InvalidChunk);
            }
            let mut trailing = [0_u8; 1];
            if reader.read(&mut trailing).await? != 0 {
                return Err(BackupEnvelopeError::InvalidChunk);
            }
            writer.flush().await?;
            return Ok(EncryptedStreamReceipt {
                plaintext_size_bytes: plaintext_size,
                encrypted_size_bytes: encrypted_size,
                plaintext_digest: digest(plaintext_hasher.finalize()),
                envelope_digest: digest(envelope_hasher.finalize()),
            });
        }
        if plaintext.is_empty() {
            return Err(BackupEnvelopeError::InvalidChunk);
        }
        writer.write_all(&plaintext).await?;
        plaintext_hasher.update(&plaintext);
        plaintext_size += plaintext.len() as u64;
        chunk_index = chunk_index
            .checked_add(1)
            .ok_or(BackupEnvelopeError::InvalidChunk)?;
    }
}

async fn read_chunk<R: AsyncRead + Unpin>(
    reader: &mut R,
    buffer: &mut [u8],
) -> Result<usize, std::io::Error> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = reader.read(&mut buffer[filled..]).await?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

async fn write_hashed<W: AsyncWrite + Unpin>(
    writer: &mut W,
    hasher: &mut Sha256,
    bytes: &[u8],
) -> Result<(), std::io::Error> {
    writer.write_all(bytes).await?;
    hasher.update(bytes);
    Ok(())
}

async fn write_ciphertext<W: AsyncWrite + Unpin>(
    writer: &mut W,
    hasher: &mut Sha256,
    ciphertext: &[u8],
) -> Result<(), BackupEnvelopeError> {
    let length = u32::try_from(ciphertext.len()).map_err(|_| BackupEnvelopeError::InvalidChunk)?;
    write_hashed(writer, hasher, &length.to_le_bytes()).await?;
    write_hashed(writer, hasher, ciphertext).await?;
    Ok(())
}

fn cipher(key: &BackupKeyMaterial) -> Result<XChaCha20Poly1305, BackupEnvelopeError> {
    XChaCha20Poly1305::new_from_slice(key.expose_bytes())
        .map_err(|_| BackupEnvelopeError::InvalidKey)
}

fn nonce(prefix: &[u8; NONCE_PREFIX_BYTES], chunk_index: u64) -> XNonce {
    let mut value = [0_u8; 24];
    value[..NONCE_PREFIX_BYTES].copy_from_slice(prefix);
    value[NONCE_PREFIX_BYTES..].copy_from_slice(&chunk_index.to_le_bytes());
    value.into()
}

fn associated_data(
    backup_set_id: BackupSetId,
    component_id: &BackupComponentId,
    chunk_index: u64,
    terminal: bool,
) -> Vec<u8> {
    format!(
        "1flowbase/system-backup/v1/{}/{}/{chunk_index}/{}",
        backup_set_id.as_uuid(),
        component_id.as_str(),
        if terminal { "end" } else { "data" }
    )
    .into_bytes()
}

fn digest(bytes: impl AsRef<[u8]>) -> ContentDigest {
    let encoded = bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    ContentDigest::try_from(encoded).expect("SHA-256 always produces a valid content digest")
}

fn header_error(error: std::io::Error) -> BackupEnvelopeError {
    match error.kind() {
        std::io::ErrorKind::UnexpectedEof => BackupEnvelopeError::Truncated,
        _ => BackupEnvelopeError::Io(error),
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, BackupEnvelopeError> {
    if !value.len().is_multiple_of(2) {
        return Err(BackupEnvelopeError::ManifestSerialization);
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or(BackupEnvelopeError::ManifestSerialization)
        })
        .collect()
}
