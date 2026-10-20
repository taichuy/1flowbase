use super::*;

impl SystemBackupService {
    pub async fn import<R>(
        &self,
        source: R,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        self.import_with_password(source, None).await
    }

    pub async fn import_with_password<R>(
        &self,
        mut source: R,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let mut magic = [0_u8; 8];
        source
            .read_exact(&mut magic)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        if &magic != BACKUP_BUNDLE_MAGIC {
            return Err(SystemBackupServiceError::Bundle);
        }
        let manifest_len = read_u64(&mut source).await?;
        if manifest_len == 0 || manifest_len > 16 * 1024 * 1024 {
            return Err(SystemBackupServiceError::Bundle);
        }
        let mut manifest_bytes = vec![0_u8; manifest_len as usize];
        source
            .read_exact(&mut manifest_bytes)
            .await
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        let sealed: SealedBackupManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|_| SystemBackupServiceError::Bundle)?;
        let backup_set_id = sealed.manifest().backup_set_id();
        let key = resolve_backup_key(self.key_provider.as_ref(), sealed.manifest(), password)
            .await
            .map_err(|_| SystemBackupServiceError::Key)?;
        verify_backup_manifest(&sealed, &key).map_err(map_envelope_error)?;
        if let Ok(existing) = self.repository.load_manifest(backup_set_id).await {
            if existing.authentication_tag() == sealed.authentication_tag()
                && existing.manifest().envelope_digest() == sealed.manifest().envelope_digest()
            {
                self.verify_with_password(backup_set_id, password).await?;
                return Ok(existing);
            }
            return Err(SystemBackupServiceError::Repository);
        }
        self.repository
            .begin_staging(backup_set_id)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        let result = self.import_in_staging(&mut source, &sealed, password).await;
        if result.is_err() {
            let _ = self.repository.abort_staging(backup_set_id).await;
        }
        result
    }

    pub(super) async fn import_in_staging<R>(
        &self,
        source: &mut R,
        sealed: &SealedBackupManifest,
        password: Option<&str>,
    ) -> Result<SealedBackupManifest, SystemBackupServiceError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let backup_set_id = sealed.manifest().backup_set_id();
        let expected = sealed
            .manifest()
            .components()
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
            .map(|component| component.component_id.clone())
            .collect::<BTreeSet<_>>();
        let count = read_u32(source).await? as usize;
        if count != expected.len() {
            return Err(SystemBackupServiceError::Bundle);
        }
        let mut imported = BTreeSet::new();
        for _ in 0..count {
            let mut id_len = [0_u8; 2];
            source
                .read_exact(&mut id_len)
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            let id_len = u16::from_le_bytes(id_len) as usize;
            if id_len == 0 || id_len > 255 {
                return Err(SystemBackupServiceError::Bundle);
            }
            let mut id = vec![0_u8; id_len];
            source
                .read_exact(&mut id)
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
            let id = std::str::from_utf8(&id).map_err(|_| SystemBackupServiceError::Bundle)?;
            let component_id =
                BackupComponentId::try_from(id).map_err(|_| SystemBackupServiceError::Bundle)?;
            if !expected.contains(&component_id) || !imported.insert(component_id.clone()) {
                return Err(SystemBackupServiceError::Bundle);
            }
            let mut writer = self
                .repository
                .open_staging_component(backup_set_id, &component_id)
                .await
                .map_err(|_| SystemBackupServiceError::Repository)?;
            let mut component_bytes = 0_u64;
            loop {
                let length = read_u32(source).await? as usize;
                if length == 0 {
                    break;
                }
                if length > BACKUP_BUNDLE_IO_BUFFER_BYTES {
                    return Err(SystemBackupServiceError::Bundle);
                }
                let mut chunk = vec![0_u8; length];
                source
                    .read_exact(&mut chunk)
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
                writer
                    .write_all(&chunk)
                    .await
                    .map_err(|_| SystemBackupServiceError::Bundle)?;
                component_bytes = component_bytes
                    .checked_add(length as u64)
                    .ok_or(SystemBackupServiceError::Bundle)?;
            }
            if component_bytes == 0 {
                return Err(SystemBackupServiceError::Bundle);
            }
            writer
                .shutdown()
                .await
                .map_err(|_| SystemBackupServiceError::Bundle)?;
        }
        if imported != expected {
            return Err(SystemBackupServiceError::Bundle);
        }
        self.repository
            .seal(sealed)
            .await
            .map_err(|_| SystemBackupServiceError::Repository)?;
        self.verify_with_password(backup_set_id, password).await?;
        Ok(sealed.clone())
    }
}
