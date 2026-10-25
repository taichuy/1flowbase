use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use control_plane::{
    file_management::{FileUploadService, FileUploadTarget, UploadFileCommand},
    ports::{
        ClaimedCommunicationInput, CommunicationAttachmentUpdate, CommunicationClaim,
        FileManagementRepository,
    },
};
use extension_contracts::{
    NotificationAttachmentDownloadOutcome, NotificationOperation, NotificationRuntimeResult,
    NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES,
};

pub(super) async fn validate_target(
    state: &ApiState,
    actor: &ActorContext,
    binding: &CommunicationBindingRecord,
) -> Result<()> {
    let table = state
        .store
        .list_visible_file_tables(actor.current_workspace_id)
        .await?
        .into_iter()
        .find(|table| {
            binding
                .config
                .file_table_id
                .map_or(table.is_default, |id| id == table.id)
        })
        .context("file_table_unavailable")?;
    ensure!(table.status == "active", "file_table_unavailable");
    let grant = control_plane::model_definition::ModelDefinitionService::new(state.store.clone())
        .load_runtime_scope_grant(
            actor,
            table.model_definition_id,
            runtime_core::runtime_acl::RuntimeDataAction::Create,
        )
        .await?
        .context("file_access_denied")?;
    runtime_core::runtime_acl::resolve_access_scope(
        actor,
        runtime_core::runtime_acl::RuntimeDataAction::Create,
        table.model_definition_id,
        Some(&grant),
    )?;
    Ok(())
}

pub(super) async fn attach(
    state: &ApiState,
    input: &ClaimedCommunicationInput,
    claim: &CommunicationClaim,
) -> Result<Vec<Value>> {
    let context = ProviderContext::load(state, &input.binding).await?;
    validate_target(state, &context.actor, &input.binding).await?;
    let mut files = Vec::new();
    for attachment in &input.attachments {
        let record = &attachment.record;
        if let (Some(table_id), Some(record_id)) = (record.file_table_id, record.file_record_id) {
            files.push(file_reference(record, table_id, record_id));
            continue;
        }
        let result = async {
            let reference: extension_contracts::NotificationAttachmentReference =
                serde_json::from_value(attachment.reference.clone())?;
            let mut bytes = Vec::new();
            loop {
                let result = context
                    .invoke(context.input(NotificationOperation::DownloadAttachment {
                        attachment: reference.clone(),
                        offset: bytes.len() as u64,
                        max_bytes: NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES,
                    }))
                    .await?;
                let NotificationRuntimeResult::DownloadAttachment {
                    outcome:
                        NotificationAttachmentDownloadOutcome::Succeeded {
                            data_base64, eof, ..
                        },
                } = result
                else {
                    anyhow::bail!("attachment_download_failed")
                };
                bytes.extend(STANDARD.decode(data_base64)?);
                if eof {
                    break;
                }
            }
            if let Some(expected) = record.size_bytes {
                ensure!(
                    bytes.len() as u64 == u64::try_from(expected)?,
                    "attachment_size_mismatch"
                );
            }
            // The existing file service/driver accepts a complete Vec, not a stream. The
            // provider transport is chunked; this final upload buffers one whole attachment.
            let uploaded = FileUploadService::new(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
            )
            .upload(UploadFileCommand {
                actor: context.actor.clone(),
                target: input
                    .binding
                    .config
                    .file_table_id
                    .map_or(FileUploadTarget::Default, FileUploadTarget::Table),
                original_filename: record.file_name.clone(),
                content_type: Some(record.media_type.clone()),
                bytes,
            })
            .await?;
            let id = uploaded
                .record
                .get("id")
                .and_then(Value::as_str)
                .context("file_record_missing")?
                .parse::<Uuid>()?;
            Ok::<_, anyhow::Error>((uploaded.file_table_id, id))
        }
        .await;
        let update = match &result {
            Ok((table, id)) => CommunicationAttachmentUpdate {
                attachment_id: record.id,
                file_table_id: Some(*table),
                file_record_id: Some(*id),
                failure_code: None,
            },
            Err(_) => CommunicationAttachmentUpdate {
                attachment_id: record.id,
                file_table_id: None,
                file_record_id: None,
                failure_code: Some("attachment_upload_failed".into()),
            },
        };
        ensure!(
            state
                .notification_service
                .record_attached_file(claim, update)
                .await?,
            "claim_lost"
        );
        let (table, id) = result?;
        files.push(file_reference(record, table, id));
    }
    Ok(files)
}
fn file_reference(record: &domain::CommunicationAttachment, table: Uuid, file: Uuid) -> Value {
    serde_json::json!({"source":"upload_file_id","value":file.to_string(),"name":record.file_name,"mime_type":record.media_type,"metadata":{"file_table_id":table}})
}
