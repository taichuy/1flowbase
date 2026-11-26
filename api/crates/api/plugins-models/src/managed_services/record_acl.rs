//! Host-owned record identity and authorization shared by all managed protocol adapters.
use super::ManagedServiceRegistration;
use crate::{app_state::ApiState, error_response::ApiError};
use control_plane::errors::ControlPlaneError as Error;
use control_plane_contracts::ports::{
    ContributionAuthorityLease, PluginContributionAuthorityRepository,
};
use domain::{ActorContext, RecordPermissionKey, RecordPermissionResourceKey};
use extension_contracts::*;
use plugin_framework::{
    ManagedServiceOperation, ManagedServiceRecordResource, ManagedServiceRecordSelector,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HandleKey {
    installation: uuid::Uuid,
    generation: String,
    actor: uuid::Uuid,
    workspace: uuid::Uuid,
    resource: String,
    handle: String,
}
fn handles() -> &'static Mutex<BTreeMap<HandleKey, String>> {
    static HANDLES: OnceLock<Mutex<BTreeMap<HandleKey, String>>> = OnceLock::new();
    HANDLES.get_or_init(Mutex::default)
}
fn handle_key(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    resource: &str,
    handle: &str,
) -> Result<HandleKey, ApiError> {
    Ok(HandleKey {
        installation: registration.installation_id,
        generation: super::stream_ticket::generation(state)?,
        actor: actor.user_id,
        workspace: actor.current_workspace_id,
        resource: resource.into(),
        handle: handle.into(),
    })
}
pub(super) fn clear_handles() {
    handles().lock().unwrap_or_else(|e| e.into_inner()).clear();
}
pub(crate) fn resource<'a>(
    registration: &'a ManagedServiceRegistration,
    code: &str,
) -> Result<&'a ManagedServiceRecordResource, ApiError> {
    registration
        .declaration
        .record_resources
        .iter()
        .find(|r| r.resource_code == code)
        .ok_or_else(|| Error::NotFound("record_permission_resource").into())
}
pub(crate) fn key(
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    resource: &ManagedServiceRecordResource,
    record: &str,
) -> Result<RecordPermissionKey, ApiError> {
    let (publisher, plugin, _) = registration.resource_owner(resource);
    Ok(RecordPermissionKey {
        resource: RecordPermissionResourceKey {
            publisher_namespace: publisher.into(),
            plugin_code: plugin.into(),
            resource_code: resource.resource_code.clone(),
        },
        record_id: canonical_record_id(registration, resource, record)?,
        workspace_id: actor.current_workspace_id,
    })
}
fn id_value(
    registration: &ManagedServiceRegistration,
    resource: &ManagedServiceRecordResource,
    record: &str,
) -> Result<PluginDataValue, ApiError> {
    if record.is_empty() || record.len() > 1024 || record.chars().any(char::is_control) {
        return Err(Error::InvalidInput("record_id").into());
    }
    let id_type = if resource.id_field == "id" {
        PluginDataFieldType::Uuid
    } else {
        registration
            .resource_data_models(resource)
            .iter()
            .flat_map(|m| &m.owned_collections)
            .find(|c| c.collection_code == resource.collection_code)
            .and_then(|c| c.fields.iter().find(|f| f.field_code == resource.id_field))
            .map(|f| f.field_type)
            .ok_or(Error::Conflict("record_permission_id_field"))?
    };
    Ok(match id_type {
        PluginDataFieldType::Uuid => PluginDataValue::Uuid(
            uuid::Uuid::parse_str(record)
                .map_err(|_| Error::InvalidInput("record_id"))?
                .to_string(),
        ),
        PluginDataFieldType::String | PluginDataFieldType::Text => {
            PluginDataValue::String(record.into())
        }
        _ => return Err(Error::Conflict("record_permission_id_field").into()),
    })
}
fn canonical_record_id(
    registration: &ManagedServiceRegistration,
    resource: &ManagedServiceRecordResource,
    record: &str,
) -> Result<String, ApiError> {
    match id_value(registration, resource, record)? {
        PluginDataValue::String(value) | PluginDataValue::Uuid(value) => Ok(value),
        _ => Err(Error::Conflict("record_permission_id_field").into()),
    }
}
pub(crate) async fn operation_allowed(
    state: &ApiState,
    actor: &ActorContext,
    op: &ManagedServiceOperation,
) -> Result<bool, ApiError> {
    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(actor)
        .await?;
    operation_allowed_with_policies(state, actor, op, &policies)
}

pub(super) fn operation_allowed_with_policies(
    state: &ApiState,
    actor: &ActorContext,
    op: &ManagedServiceOperation,
    policies: &[domain::RoleConsolePolicy],
) -> Result<bool, ApiError> {
    use crate::middleware::require_settings_feature_permission::*;
    let access =
        compiled_console_route_access(&state.console_operation_registry, &op.method, &op.path)
            .map_err(Error::PermissionDenied)?;
    Ok(authorize_compiled_console_access(&access, actor, policies))
}
/// Reject inactive owners before touching data. The Host picks identity from the frozen registry.
fn ensure_live_registration(
    registration: &ManagedServiceRegistration,
    lease: &dyn ContributionAuthorityLease,
) -> Result<(), ApiError> {
    let current = lease
        .installation(registration.installation_id)
        .ok_or(Error::Conflict("managed_service_inactive"))?;
    if current.desired_state != domain::PluginDesiredState::ActiveRequested
        || current.organization != registration.publisher_namespace
        || current.provider_code != registration.plugin_code
        || current.plugin_version != registration.plugin_version
        || current.contract_version != "1flowbase.extension-bus/v1"
        || domain::managed_installation_scope(current, domain::DEFAULT_SCOPE_ID)
            != domain::SYSTEM_SCOPE_ID
    {
        return Err(Error::Conflict("managed_service_inactive").into());
    }
    let resolved =
        control_plane::plugin_management::resolve_resource_imports(current, lease.installations())?;
    if resolved != registration.resource_imports {
        return Err(Error::Conflict("plugin_resource_generation_mismatch").into());
    }
    Ok(())
}
pub(crate) async fn ensure_active(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
) -> Result<(), ApiError> {
    let lease = state
        .store
        .lock_installation_resource_authority(registration.installation_id, domain::SYSTEM_SCOPE_ID)
        .await?;
    ensure_live_registration(registration, lease.as_ref())?;
    lease.release().await?;
    let composition = state.provider_runtime.managed_composition()?;
    let snapshot = match state
        .extension_boot_snapshot
        .as_ref()
        .filter(|b| b.has_pinned_managed_snapshots())
    {
        Some(boot) => boot.managed_snapshot(domain::SYSTEM_SCOPE_ID),
        None => composition.snapshot(domain::SYSTEM_SCOPE_ID).await,
    }
    .ok_or(Error::Conflict("managed_service_inactive"))?;
    for op in &registration.declaration.operations {
        let id = plugin_framework::extension_bus::ContributionId::new(&op.contribution_id)?;
        let binding = snapshot
            .bindings
            .get(&id)
            .ok_or(Error::Conflict("managed_service_inactive"))?;
        if binding.handle.identity().installation_id().as_str()
            != registration.installation_id.to_string()
        {
            return Err(Error::Conflict("managed_service_generation_mismatch").into());
        }
    }
    Ok(())
}
pub(crate) async fn ensure_record(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    resource: &ManagedServiceRecordResource,
    record: &str,
) -> Result<(), ApiError> {
    ensure_records(state, registration, actor, resource, &[record])
        .await
        .map(|_| ())
}

pub(crate) async fn ensure_records(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    resource: &ManagedServiceRecordResource,
    records: &[impl AsRef<str>],
) -> Result<Vec<String>, ApiError> {
    let mut ids = Vec::with_capacity(records.len());
    let mut invalid = None;
    for record in records {
        match canonical_record_id(registration, resource, record.as_ref()) {
            Ok(id) => ids.push(id),
            Err(error) => {
                // Earlier IDs still take precedence over a later malformed ID.
                invalid = Some(error);
                break;
            }
        }
    }
    if ids.is_empty() {
        return invalid.map_or(Ok(ids), Err);
    }
    let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
    let expected = unique.len() as u64;
    let (publisher, plugin, version) = registration.resource_owner(resource);
    let binding = PluginDataBinding {
        managed_subject: None,
        publisher_namespace: publisher.into(),
        plugin_code: plugin.into(),
        plugin_version: version.into(),
        storage_binding: "main".into(),
        workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
        actor_id: Some(actor.user_id.to_string()),
        provider_instance_id: registration.installation_id.to_string(),
        permissions: [PluginDataPermission::Read].into_iter().collect(),
        deadline_unix_ms: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
            as i64
            + 10000,
        // This Host-only existence lookup is separate from the user's row authorization below.
        record_permissions: None,
    };
    let request = PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Count {
            target: PluginDataTarget::OwnedCollection {
                collection_code: resource.collection_code.clone(),
            },
            filters: vec![PluginDataFilter {
                field: resource.id_field.clone(),
                operator: PluginDataFilterOperator::In,
                value: Some(PluginDataValue::Json(serde_json::json!(unique))),
            }],
        }],
    };
    let lease = state
        .store
        .lock_installation_resource_authority(registration.installation_id, domain::SYSTEM_SCOPE_ID)
        .await?;
    ensure_live_registration(registration, lease.as_ref())?;
    let response = lease.commit_plugin_data(&binding, &request).await.map_err(|error| {
        tracing::warn!(kind = ?error.kind, code = %error.code, "record permission lookup failed");
        Error::UpstreamUnavailable("record_permission_record_lookup")
    })?;
    if !matches!(response.results.first(), Some(PluginDataOperationResult::Count {count}) if *count == expected)
    {
        return Err(Error::NotFound("record_permission_record").into());
    }
    invalid.map_or(Ok(ids), Err)
}
fn selector_value<'a>(
    payload: &'a Value,
    pointer: &str,
    allow_missing: bool,
) -> Result<Option<&'a str>, ApiError> {
    match payload.pointer(pointer) {
        None | Some(Value::Null) if allow_missing => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= 1024 => Ok(Some(value)),
        _ => Err(Error::InvalidInput("record_permission_selector").into()),
    }
}
pub(super) async fn authorize(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    op: &ManagedServiceOperation,
    payload: &Value,
) -> Result<Option<PluginDataRecordPermissionContext>, ApiError> {
    if registration.declaration.record_resources.is_empty() {
        return Ok(None);
    }
    ensure_active(state, registration).await?;
    let mut selected = None;
    if let Some(permission) = &op.record_permission {
        let definition = resource(registration, &permission.resource_code)?;
        selected = match &permission.selector {
            ManagedServiceRecordSelector::List | ManagedServiceRecordSelector::Create => None,
            ManagedServiceRecordSelector::Record {
                pointer,
                allow_missing,
            } => selector_value(payload, pointer, *allow_missing)?.map(str::to_owned),
            ManagedServiceRecordSelector::Handle { pointer } => {
                let handle = selector_value(payload, pointer, false)?
                    .ok_or(Error::InvalidInput("record_permission_handle"))?;
                let handle_key = handle_key(
                    state,
                    registration,
                    actor,
                    &permission.resource_code,
                    handle,
                )?;
                Some(resolve_handle(&handle_key)?)
            }
        };
        selected = selected
            .map(|id| canonical_record_id(registration, definition, &id))
            .transpose()?;
        if let Some(record) = selected.as_deref() {
            ensure_record(state, registration, actor, definition, record).await?;
            control_plane::record_permissions::RecordPermissionService::new(state.store.clone())
                .authorize(
                    actor,
                    &key(registration, actor, definition, record)?,
                    &permission.action_code,
                    operation_allowed(state, actor, op).await?,
                )
                .await?;
        }
    }
    Ok(Some(data_context(registration, actor, op, selected)))
}
fn data_context(
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    op: &ManagedServiceOperation,
    selected: Option<String>,
) -> PluginDataRecordPermissionContext {
    PluginDataRecordPermissionContext {
        caller_workspace_id: actor.current_workspace_id.to_string(),
        actor_user_id: actor.user_id.to_string(),
        active_role_code: actor.effective_display_role.clone(),
        is_root: actor.is_root,
        resources: registration
            .declaration
            .record_resources
            .iter()
            .map(|r| {
                let permission = op
                    .record_permission
                    .as_ref()
                    .filter(|p| p.resource_code == r.resource_code);
                PluginDataRecordResource {
                    resource_code: r.resource_code.clone(),
                    collection_code: r.collection_code.clone(),
                    id_field: r.id_field.clone(),
                    action_code: permission.map_or("", |p| p.action_code.as_str()).into(),
                    record_id: permission.and(selected.clone()),
                }
            })
            .collect(),
    }
}
pub(super) fn bind_response(
    state: &ApiState,
    registration: &ManagedServiceRegistration,
    actor: &ActorContext,
    op: &ManagedServiceOperation,
    context: &Option<PluginDataRecordPermissionContext>,
    output: &Value,
) -> Result<(), ApiError> {
    let Some(permission) = &op.record_permission else {
        return Ok(());
    };
    let Some(pointer) = &permission.response_handle_pointer else {
        return Ok(());
    };
    let record = context
        .as_ref()
        .and_then(|c| {
            c.resources
                .iter()
                .find(|r| r.resource_code == permission.resource_code)
        })
        .and_then(|r| r.record_id.as_ref())
        .ok_or(Error::PermissionDenied("record_permission_handle"))?;
    let handle = selector_value(output, pointer, false)?
        .ok_or(Error::InvalidInput("record_permission_handle"))?;
    let key = handle_key(
        state,
        registration,
        actor,
        &permission.resource_code,
        handle,
    )?;
    bind_handle(key, record)
}
fn resolve_handle(key: &HandleKey) -> Result<String, ApiError> {
    handles()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(key)
        .cloned()
        .ok_or_else(|| Error::PermissionDenied("record_permission_handle").into())
}
fn bind_handle(key: HandleKey, record: &str) -> Result<(), ApiError> {
    let mut all = handles().lock().unwrap_or_else(|e| e.into_inner());
    if all.get(&key).is_some_and(|old| old != record) {
        return Err(Error::Conflict("record_permission_handle_rebound").into());
    }
    all.insert(key, record.into());
    Ok(())
}

#[cfg(test)]
#[path = "_tests/record_acl.rs"]
mod tests;
