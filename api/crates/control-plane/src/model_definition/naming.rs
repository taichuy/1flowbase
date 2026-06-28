use anyhow::Result;
use domain::DataModelScopeKind;
use uuid::Uuid;

use crate::errors::ControlPlaneError;

const REGISTERED_SYSTEM_TABLE_CODES: [&str; 3] = ["attachments", "users", "roles"];

pub(super) fn registered_system_table_name(
    scope_kind: domain::DataModelScopeKind,
    source_kind: domain::DataModelSourceKind,
    protection: &domain::DataModelProtection,
    code: &str,
) -> Option<&'static str> {
    if scope_kind != domain::DataModelScopeKind::System
        || source_kind != domain::DataModelSourceKind::MainSource
        || protection.owner_kind != domain::DataModelOwnerKind::Core
        || !protection.is_protected
    {
        return None;
    }

    REGISTERED_SYSTEM_TABLE_CODES
        .into_iter()
        .find(|registered_code| *registered_code == code)
}

pub(super) fn normalize_api_exposure_for_status(
    status: domain::DataModelStatus,
    exposure: domain::ApiExposureStatus,
) -> Result<domain::ApiExposureStatus> {
    let effective_exposure = if status == domain::DataModelStatus::Draft {
        domain::ApiExposureStatus::Draft
    } else {
        exposure
    };
    if domain::ApiExposureStatus::validate_for_status(
        status,
        effective_exposure,
        domain::ApiExposureReadiness::default(),
    )
    .is_rejected()
    {
        Err(ControlPlaneError::InvalidInput("api_exposure_status").into())
    } else {
        Ok(effective_exposure)
    }
}

pub(super) fn build_physical_table_name(scope_kind: DataModelScopeKind, code: &str) -> String {
    let prefix = match scope_kind {
        DataModelScopeKind::Workspace => "workspace",
        DataModelScopeKind::System => "system",
    };
    let suffix = Uuid::now_v7().simple().to_string();
    let sanitized_code = code.replace('-', "_");

    format!(
        "rtm_{prefix}_{}_{}",
        &suffix[suffix.len() - 8..],
        sanitized_code
    )
}

pub(super) fn build_physical_column_name(code: &str) -> String {
    code.replace('-', "_")
}
