use anyhow::Result;
use domain::DataModelScopeKind;
use uuid::Uuid;

use crate::errors::ControlPlaneError;

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

    domain::builtin_data_model_contract(code).map(|contract| contract.physical_table_name)
}

pub(super) fn normalize_api_exposure_for_status(
    status: domain::DataModelStatus,
    exposure: domain::ApiExposureStatus,
) -> Result<domain::ApiExposureStatus> {
    match status {
        domain::DataModelStatus::Draft => Ok(domain::ApiExposureStatus::Draft),
        domain::DataModelStatus::Published => {
            if exposure == domain::ApiExposureStatus::Draft {
                Err(ControlPlaneError::InvalidInput("api_exposure_status").into())
            } else {
                Ok(exposure)
            }
        }
        domain::DataModelStatus::Disabled | domain::DataModelStatus::Broken => {
            Ok(domain::ApiExposureStatus::PublishedNotExposed)
        }
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
