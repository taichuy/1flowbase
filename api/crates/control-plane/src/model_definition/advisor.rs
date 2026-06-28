use anyhow::Result;
use domain::ScopeDataModelPermissionProfile;
use uuid::Uuid;

use crate::errors::ControlPlaneError;

pub(super) fn advisor_finding(
    data_model_id: Uuid,
    severity: domain::DataModelAdvisorSeverity,
    code: &'static str,
    message: &'static str,
    recommended_action: &'static str,
    can_acknowledge: bool,
) -> domain::DataModelAdvisorFinding {
    domain::DataModelAdvisorFinding {
        id: format!("{data_model_id}:{code}"),
        data_model_id,
        severity,
        code: code.to_string(),
        message: message.to_string(),
        recommended_action: recommended_action.to_string(),
        can_acknowledge,
    }
}

pub(super) fn has_duplicate_or_risky_field_configuration(
    fields: &[domain::ModelFieldRecord],
) -> bool {
    let mut codes = std::collections::HashSet::new();
    let mut external_keys = std::collections::HashSet::new();

    for field in fields {
        if !codes.insert(field.code.as_str()) {
            return true;
        }
        if let Some(external_key) = field.external_field_key.as_deref() {
            if !external_keys.insert(external_key) {
                return true;
            }
        }
        if field.is_unique && field.field_kind == domain::ModelFieldKind::Json {
            return true;
        }
    }

    false
}

pub(super) fn external_source_is_unsafe(model: &domain::ModelDefinitionRecord) -> bool {
    if model.source_kind != domain::DataModelSourceKind::ExternalSource {
        return false;
    }

    let Some(snapshot) = &model.external_capability_snapshot else {
        return true;
    };

    !snapshot
        .get("supports_scope_filter")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn ensure_unsafe_external_system_all_confirmed(
    model: &domain::ModelDefinitionRecord,
    permission_profile: ScopeDataModelPermissionProfile,
    confirmed: bool,
) -> Result<(), ControlPlaneError> {
    if permission_profile == ScopeDataModelPermissionProfile::SystemAll
        && external_source_is_unsafe(model)
        && !confirmed
    {
        return Err(ControlPlaneError::InvalidInput("confirmation"));
    }

    Ok(())
}
