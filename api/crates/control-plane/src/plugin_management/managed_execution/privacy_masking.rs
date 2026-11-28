//! Host-owned managed callback service for the `privacy-masking/v1` Host Service contract.
use super::HostContributionGrantPolicy;
use crate::errors::ControlPlaneError;
use crate::privacy_masking::{
    list_builtin_rule_descriptors, PrivacyMaskingKernel, PrivacyPolicySyncError,
};
use control_plane_contracts::ports::{
    ApplicationRepository, ApplicationVisibility, ListPrivacyMaskingLogsQuery,
    ModelProviderRepository, PluginContributionAuthorityRepository, PrivacyMaskingRepository,
};
use extension_contracts::{
    HostServiceError, PrivacyMaskingAgentFlowSubjectDto, PrivacyMaskingBinding,
    PrivacyMaskingFuture, PrivacyMaskingModelInstanceSubjectDto, PrivacyMaskingPort,
    PrivacyMaskingRequest, PrivacyMaskingResponse, PrivacyMaskingSubjectKind,
    PRIVACY_LOGS_READ_PERMISSION, PRIVACY_POLICY_SYNC_PERMISSION, PRIVACY_SUBJECTS_READ_PERMISSION,
};
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

pub struct ManagedPrivacyMaskingService {
    authority: Arc<dyn PluginContributionAuthorityRepository>,
    model_providers: Arc<dyn ModelProviderRepository>,
    applications: Arc<dyn ApplicationRepository>,
    repository: Arc<dyn PrivacyMaskingRepository>,
    kernel: Arc<PrivacyMaskingKernel>,
}

impl ManagedPrivacyMaskingService {
    pub fn new(
        authority: Arc<dyn PluginContributionAuthorityRepository>,
        model_providers: Arc<dyn ModelProviderRepository>,
        applications: Arc<dyn ApplicationRepository>,
        repository: Arc<dyn PrivacyMaskingRepository>,
        kernel: Arc<PrivacyMaskingKernel>,
    ) -> Self {
        Self {
            authority,
            model_providers,
            applications,
            repository,
            kernel,
        }
    }

    async fn dispatch(
        &self,
        installation_id: Uuid,
        binding: &PrivacyMaskingBinding,
        request: &PrivacyMaskingRequest,
    ) -> Result<PrivacyMaskingResponse, HostServiceError> {
        use PrivacyMaskingRequest as Request;
        use PrivacyMaskingResponse as Response;

        match request {
            Request::InspectCallerContext => Ok(Response::CallerContext {
                caller: binding.caller.clone(),
            }),
            Request::ListBuiltinRules => Ok(Response::BuiltinRules {
                items: list_builtin_rule_descriptors(),
            }),
            Request::ListSubjects => {
                let (workspace_id, actor_user_id) = require_caller_workspace_and_actor(binding)?;
                let instances = self
                    .model_providers
                    .list_instances(workspace_id)
                    .await
                    .map_err(map_storage_error)?;
                let model_provider_instances = instances
                    .into_iter()
                    .map(|record| PrivacyMaskingModelInstanceSubjectDto {
                        instance_id: record.id.to_string(),
                        display_name: record.display_name,
                        provider_code: record.provider_code,
                        protocol: record.protocol,
                        status: record.status.as_str().to_string(),
                        enabled_model_ids: record.enabled_model_ids,
                    })
                    .collect();

                let apps = self
                    .applications
                    .list_applications(workspace_id, actor_user_id, ApplicationVisibility::All)
                    .await
                    .map_err(map_storage_error)?;
                let agent_flows = apps
                    .into_iter()
                    .filter(|app| app.application_type == domain::ApplicationType::AgentFlow)
                    .map(|app| PrivacyMaskingAgentFlowSubjectDto {
                        application_id: app.id.to_string(),
                        name: app.name,
                        status: if app.release_version > 0 {
                            "published".to_string()
                        } else {
                            "draft".to_string()
                        },
                    })
                    .collect();

                Ok(Response::Subjects {
                    model_provider_instances,
                    agent_flows,
                })
            }
            Request::SyncPolicy { policy } => {
                let (workspace_id, actor_user_id) = require_caller_workspace_and_actor(binding)?;
                let mut policy = policy.clone();
                for subject in &mut policy.subject_bindings {
                    let id = parse_uuid(&subject.subject_id)?;
                    let owned = match subject.subject_kind {
                        PrivacyMaskingSubjectKind::ModelProviderInstance => self
                            .model_providers
                            .get_instance(workspace_id, id)
                            .await
                            .map_err(map_storage_error)?
                            .is_some(),
                        PrivacyMaskingSubjectKind::AgentFlow => self
                            .applications
                            .get_application(workspace_id, id)
                            .await
                            .map_err(map_storage_error)?
                            .is_some_and(|app| {
                                app.application_type == domain::ApplicationType::AgentFlow
                            }),
                    };
                    if !owned {
                        let mut error = HostServiceError::new("privacy_subject_not_in_workspace");
                        error.status = Some(400);
                        return Err(error);
                    }
                    // The runtime matches subjects against UUIDs in canonical text form.
                    subject.subject_id = id.to_string();
                }
                let summary = self
                    .kernel
                    .sync_workspace_policy(
                        installation_id,
                        workspace_id,
                        Some(actor_user_id),
                        &policy,
                    )
                    .await
                    .map_err(map_sync_error)?;
                Ok(Response::PolicySynced {
                    policy_version: summary.policy_version,
                    enabled_rule_count: summary.enabled_rule_count,
                    active_subject_count: summary.active_subject_count,
                })
            }
            Request::PreviewMasking {
                text,
                policy,
                restore_output,
            } => {
                let (workspace_id, _) = require_caller_workspace_and_actor(binding)?;
                let preview = self
                    .kernel
                    .preview_masking(workspace_id, text, policy.as_ref(), *restore_output)
                    .await
                    .map_err(map_sync_error)?;
                Ok(Response::PreviewMasked {
                    masked_text: preview.masked_text,
                    restored_text: preview.restored_text,
                    hits: preview.hits,
                    total_hits: preview.total_hits,
                })
            }
            Request::ListLogs {
                subject_kind,
                subject_id,
                placeholder_kind,
                rule_id,
                flow_run_id,
                limit,
                cursor,
            } => {
                let (workspace_id, _) = require_caller_workspace_and_actor(binding)?;
                let parsed_flow_run_id = flow_run_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(parse_uuid)
                    .transpose()?;
                let parsed_cursor = cursor
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(decode_log_cursor)
                    .transpose()?;

                let page = self
                    .repository
                    .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
                        workspace_id,
                        subject_kind: *subject_kind,
                        subject_id: subject_id.clone(),
                        placeholder_kind: placeholder_kind.clone(),
                        rule_id: rule_id.clone(),
                        flow_run_id: parsed_flow_run_id,
                        limit: limit.unwrap_or(50).clamp(1, 200),
                        cursor: parsed_cursor,
                    })
                    .await
                    .map_err(map_storage_error)?;
                Ok(Response::Logs {
                    items: page.items,
                    next_cursor: page.next_cursor.map(encode_log_cursor),
                })
            }
        }
    }
}

impl PrivacyMaskingPort for ManagedPrivacyMaskingService {
    fn execute<'a>(
        &'a self,
        binding: &'a PrivacyMaskingBinding,
        request: &'a PrivacyMaskingRequest,
    ) -> PrivacyMaskingFuture<'a> {
        Box::pin(async move {
            let denied = || HostServiceError::new("privacy_masking_authority_denied");
            let remaining = i128::from(binding.deadline_unix_ms)
                - OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining = u64::try_from(remaining)
                .ok()
                .filter(|ms| *ms > 0)
                .ok_or_else(|| HostServiceError::new("privacy_masking_deadline_exceeded"))?;

            let required_permission = request.required_permission();
            let binding_allows = match required_permission {
                PRIVACY_SUBJECTS_READ_PERMISSION => binding.can_read_subjects,
                PRIVACY_POLICY_SYNC_PERMISSION => binding.can_sync_policy,
                PRIVACY_LOGS_READ_PERMISSION => binding.can_read_logs,
                _ => false,
            };
            if !binding_allows {
                return Err(denied());
            }

            tokio::time::timeout(std::time::Duration::from_millis(remaining), async {
                let installation_id = Uuid::parse_str(binding.subject.installation_id().as_str())
                    .map_err(|_| denied())?;
                if binding.subject.workspace_id().as_str() != domain::SYSTEM_SCOPE_ID.to_string() {
                    return Err(denied());
                }
                let lease = self
                    .authority
                    .lock_managed_callback_authority(&binding.subject)
                    .await
                    .map_err(|_| denied())?;
                let installation = lease.installation(installation_id).ok_or_else(denied)?;
                if installation.desired_state != domain::PluginDesiredState::ActiveRequested
                    || installation.organization != binding.publisher_namespace
                    || installation.provider_code != binding.plugin_code
                    || installation.plugin_version != binding.plugin_version
                    || domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                        != domain::SYSTEM_SCOPE_ID
                {
                    return Err(denied());
                }
                let managed: plugin_framework::ManagedManifest =
                    serde_json::from_value(installation.metadata_json["managed"].clone())
                        .map_err(|_| denied())?;
                let contribution = managed
                    .module
                    .contributions
                    .iter()
                    .find(|item| &item.contribution_id == binding.subject.contribution_id())
                    .ok_or_else(denied)?;
                let permissions = HostContributionGrantPolicy::root_composition()
                    .effective_permissions(
                        installation,
                        domain::SYSTEM_SCOPE_ID,
                        contribution,
                        lease.snapshot(),
                    )
                    .map_err(|_| denied())?;
                let granted = permissions
                    .iter()
                    .any(|p| p.as_str() == required_permission)
                    && contribution
                        .required_permissions
                        .iter()
                        .any(|p| p.as_str() == required_permission);
                if !granted {
                    return Err(denied());
                }
                let result = self.dispatch(installation_id, binding, request).await;
                lease.release().await.map_err(|_| denied())?;
                result
            })
            .await
            .map_err(|_| HostServiceError::new("privacy_masking_deadline_exceeded"))?
        })
    }
}

fn require_caller_workspace_and_actor(
    binding: &PrivacyMaskingBinding,
) -> Result<(Uuid, Uuid), HostServiceError> {
    let caller = binding.caller.as_ref().ok_or_else(|| {
        let mut err = HostServiceError::new("privacy_masking_caller_required");
        err.status = Some(401);
        err
    })?;
    let workspace_id = parse_uuid(&caller.current_workspace_id)?;
    let user_id = parse_uuid(&caller.user_id)?;
    Ok((workspace_id, user_id))
}

fn parse_uuid(value: &str) -> Result<Uuid, HostServiceError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        let mut err = HostServiceError::new("privacy_masking_invalid_uuid");
        err.status = Some(400);
        err
    })
}

fn encode_log_cursor((created_at, id): (OffsetDateTime, Uuid)) -> String {
    format!("{}|{}", created_at.unix_timestamp_nanos(), id)
}

fn decode_log_cursor(raw: &str) -> Result<(OffsetDateTime, Uuid), HostServiceError> {
    let (nanos_s, id_s) = raw.split_once('|').ok_or_else(|| {
        let mut err = HostServiceError::new("privacy_masking_invalid_cursor");
        err.status = Some(400);
        err
    })?;
    let nanos: i128 = nanos_s.parse().map_err(|_| {
        let mut err = HostServiceError::new("privacy_masking_invalid_cursor");
        err.status = Some(400);
        err
    })?;
    let ts = OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| {
        let mut err = HostServiceError::new("privacy_masking_invalid_cursor");
        err.status = Some(400);
        err
    })?;
    let id = parse_uuid(id_s)?;
    Ok((ts, id))
}

fn map_sync_error(error: PrivacyPolicySyncError) -> HostServiceError {
    match error {
        PrivacyPolicySyncError::Compile(compile_err) => {
            let mut err = HostServiceError::new(compile_err.code());
            err.status = Some(400);
            err.details = Some(serde_json::json!({ "message": compile_err.to_string() }));
            err
        }
        PrivacyPolicySyncError::Storage(storage_err) => map_storage_error(storage_err),
    }
}

fn map_storage_error(error: anyhow::Error) -> HostServiceError {
    let (code, status) = match error.downcast_ref::<ControlPlaneError>() {
        Some(ControlPlaneError::PermissionDenied(code)) => (*code, 403),
        Some(ControlPlaneError::Conflict(code)) => (*code, 409),
        Some(ControlPlaneError::InvalidInput(code)) => (*code, 400),
        Some(ControlPlaneError::NotFound(code)) => (*code, 404),
        _ => ("privacy_masking_failed", 500),
    };
    let mut err = HostServiceError::new(code);
    err.status = Some(status);
    err
}
