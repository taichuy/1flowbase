use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupManifest,
    BackupSetId, BackupSourceIdentity, ContentDigest, KeyFingerprint, MigrationHead,
    SealedBackupManifest,
};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    ports::BackupKeyMaterial,
    system_backup::{
        authenticate_backup_manifest, decrypt_backup_stream, encrypt_backup_stream,
        verify_backup_manifest,
    },
};

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
