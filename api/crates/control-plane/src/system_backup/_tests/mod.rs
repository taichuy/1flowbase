use base64::{engine::general_purpose::STANDARD, Engine as _};
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupManifest,
    BackupSetId, BackupSourceIdentity, ContentDigest, KeyFingerprint, MigrationHead,
    SealedBackupManifest,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    ports::{BackupKeyMaterial, BackupKeyProvider, BackupKeyProviderError},
    system_backup::{
        authenticate_backup_manifest, decrypt_backup_stream, encrypt_backup_stream,
        verify_backup_manifest,
    },
};

struct RejectingKeyProvider;

#[async_trait::async_trait]
impl BackupKeyProvider for RejectingKeyProvider {
    async fn active_key(&self) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        Err(BackupKeyProviderError::NotFound)
    }

    async fn key_for(
        &self,
        _: &KeyFingerprint,
    ) -> Result<BackupKeyMaterial, BackupKeyProviderError> {
        Err(BackupKeyProviderError::NotFound)
    }
}

fn key() -> BackupKeyMaterial {
    BackupKeyMaterial::new(
        KeyFingerprint::try_from(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .unwrap(),
        vec![7_u8; 32],
    )
    .unwrap()
}

fn fingerprint(value: char) -> String {
    std::iter::repeat_n(value, 64).collect()
}

fn manifest() -> BackupManifest {
    BackupManifest::try_new(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.test").unwrap(),
        MigrationHead::try_from("migration.test").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        KeyFingerprint::try_from(fingerprint('c')).unwrap(),
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/octet-stream".to_owned(),
            size_bytes: 1,
            content_digest: ContentDigest::try_from(fingerprint('d')).unwrap(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }],
        1,
        ContentDigest::try_from(fingerprint('e')).unwrap(),
    )
    .unwrap()
}

#[test]
fn manifest_authentication_rejects_tampered_manifest() {
    let key = key();
    let sealed = authenticate_backup_manifest(manifest(), &key).unwrap();
    verify_backup_manifest(&sealed, &key).unwrap();

    let mut value = serde_json::to_value(&sealed).unwrap();
    value["manifest"]["application_build"] = serde_json::json!("git.tampered");
    let tampered = serde_json::from_value::<SealedBackupManifest>(value).unwrap();

    assert!(verify_backup_manifest(&tampered, &key).is_err());
}

#[tokio::test]
async fn portable_manifest_resolves_without_the_target_environment_key() {
    let source_master_key = b"source-deployment-master-key";
    let mut derivation = Sha256::new();
    derivation.update(b"1flowbase/system-backup/key/v1\0");
    derivation.update(source_master_key);
    let backup_key = derivation.finalize().to_vec();
    let backup_fingerprint =
        KeyFingerprint::try_from(format!("{:x}", Sha256::digest(&backup_key))).unwrap();
    let manifest = BackupManifest::try_new_portable(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("v99.0.0").unwrap(),
        MigrationHead::try_from("migration.test").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        backup_fingerprint,
        STANDARD.encode(source_master_key),
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/octet-stream".to_owned(),
            size_bytes: 1,
            content_digest: ContentDigest::try_from(fingerprint('d')).unwrap(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }],
        1,
        ContentDigest::try_from(fingerprint('e')).unwrap(),
    )
    .unwrap();

    let resolved = super::resolve_backup_key(&RejectingKeyProvider, &manifest, None)
        .await
        .unwrap();
    assert_eq!(resolved.expose_bytes(), backup_key);
}

#[tokio::test]
async fn password_protection_requires_the_correct_password_before_releasing_recovery_material() {
    let protection =
        super::password_protection("correct-password", Some(&STANDARD.encode("source-master")))
            .unwrap();
    let manifest = BackupManifest::try_new_password_encrypted(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("v99.0.0").unwrap(),
        MigrationHead::try_from("migration.test").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        protection.key.fingerprint().clone(),
        protection.salt_base64,
        protection.encrypted_source_master_key_base64,
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/octet-stream".to_owned(),
            size_bytes: 1,
            content_digest: ContentDigest::try_from(fingerprint('d')).unwrap(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }],
        1,
        ContentDigest::try_from(fingerprint('e')).unwrap(),
    )
    .unwrap();
    assert!(
        super::recover_source_master_key(&RejectingKeyProvider, &manifest, None)
            .await
            .is_err()
    );
    assert!(super::recover_source_master_key(
        &RejectingKeyProvider,
        &manifest,
        Some("wrong-password")
    )
    .await
    .is_err());
    assert_eq!(
        super::recover_source_master_key(
            &RejectingKeyProvider,
            &manifest,
            Some("correct-password")
        )
        .await
        .unwrap(),
        "source-master"
    );
}

#[tokio::test]
async fn truncated_password_recovery_material_is_a_controlled_error() {
    let protection =
        super::password_protection("correct-password", Some(&STANDARD.encode("source-master")))
            .unwrap();
    let manifest = BackupManifest::try_new_password_encrypted(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("v99.0.0").unwrap(),
        MigrationHead::try_from("migration.test").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        protection.key.fingerprint().clone(),
        protection.salt_base64,
        STANDARD.encode([0_u8; 23]),
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/octet-stream".to_owned(),
            size_bytes: 1,
            content_digest: ContentDigest::try_from(fingerprint('d')).unwrap(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }],
        1,
        ContentDigest::try_from(fingerprint('e')).unwrap(),
    )
    .unwrap();

    assert!(super::recover_source_master_key(
        &RejectingKeyProvider,
        &manifest,
        Some("correct-password")
    )
    .await
    .is_err());
}

async fn encrypt(
    plaintext: Vec<u8>,
    backup_set_id: BackupSetId,
    component_id: BackupComponentId,
) -> Vec<u8> {
    let (mut source_writer, source_reader) = tokio::io::duplex(plaintext.len().max(1) + 1);
    source_writer.write_all(&plaintext).await.unwrap();
    source_writer.shutdown().await.unwrap();
    let (encrypted_writer, mut encrypted_reader) = tokio::io::duplex(1024 * 1024);
    let task = tokio::spawn(async move {
        encrypt_backup_stream(
            source_reader,
            encrypted_writer,
            &key(),
            backup_set_id,
            &component_id,
        )
        .await
    });
    let mut encrypted = Vec::new();
    encrypted_reader.read_to_end(&mut encrypted).await.unwrap();
    task.await.unwrap().unwrap();
    encrypted
}

#[tokio::test]
async fn envelope_round_trip_binds_backup_and_component_identity() {
    let backup_set_id = BackupSetId::new();
    let component_id = BackupComponentId::try_from("postgres/main").unwrap();
    let plaintext = vec![9_u8; 1024 * 1024 + 17];
    let encrypted = encrypt(plaintext.clone(), backup_set_id, component_id.clone()).await;
    assert!(!encrypted
        .windows(32)
        .any(|window| window == &plaintext[..32]));

    let (mut encrypted_writer, encrypted_reader) = tokio::io::duplex(encrypted.len() + 1);
    encrypted_writer.write_all(&encrypted).await.unwrap();
    encrypted_writer.shutdown().await.unwrap();
    let (plaintext_writer, mut plaintext_reader) = tokio::io::duplex(plaintext.len() + 1);
    let task = tokio::spawn(async move {
        decrypt_backup_stream(
            encrypted_reader,
            plaintext_writer,
            &key(),
            backup_set_id,
            &component_id,
        )
        .await
    });
    let mut restored = Vec::new();
    plaintext_reader.read_to_end(&mut restored).await.unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(restored, plaintext);
}

#[tokio::test]
async fn envelope_rejects_tamper_and_wrong_component() {
    let backup_set_id = BackupSetId::new();
    let component_id = BackupComponentId::try_from("postgres/main").unwrap();
    let mut encrypted = encrypt(vec![3_u8; 4096], backup_set_id, component_id).await;
    let last = encrypted.len() - 1;
    encrypted[last] ^= 1;

    let (mut encrypted_writer, encrypted_reader) = tokio::io::duplex(encrypted.len() + 1);
    encrypted_writer.write_all(&encrypted).await.unwrap();
    encrypted_writer.shutdown().await.unwrap();
    let (plaintext_writer, _plaintext_reader) = tokio::io::duplex(8192);
    let error = decrypt_backup_stream(
        encrypted_reader,
        plaintext_writer,
        &key(),
        backup_set_id,
        &BackupComponentId::try_from("postgres/other").unwrap(),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("authentication"));
}
