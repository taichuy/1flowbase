use super::*;

pub(super) fn validate_application_management_filter(
    filter: &domain::ResourceFilterExpr,
) -> Result<(), ControlPlaneError> {
    match filter {
        domain::ResourceFilterExpr::All(items) | domain::ResourceFilterExpr::Any(items) => {
            for item in items {
                validate_application_management_filter(item)?;
            }
            Ok(())
        }
        domain::ResourceFilterExpr::Field {
            field, operator, ..
        } => {
            let operator_allowed = match field.as_str() {
                "id" | "name" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq
                        | domain::ResourceFilterOperator::Ne
                        | domain::ResourceFilterOperator::Includes
                        | domain::ResourceFilterOperator::NotIncludes
                        | domain::ResourceFilterOperator::In
                ),
                "application_type"
                | "workflow_trigger_type"
                | "publication_status"
                | "created_by" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq
                        | domain::ResourceFilterOperator::Ne
                        | domain::ResourceFilterOperator::In
                ),
                "tags.id" => matches!(
                    operator,
                    domain::ResourceFilterOperator::Eq | domain::ResourceFilterOperator::In
                ),
                _ => false,
            };

            if operator_allowed {
                Ok(())
            } else {
                Err(ControlPlaneError::InvalidInput("filter"))
            }
        }
    }
}

pub(super) fn applications_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
    )
    .expect("compiled applications settings feature id must be valid")
}

pub(crate) fn effective_application_row_scope(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> domain::ConsoleOperationRowScope {
    let operation_id = domain::ConsoleOperationId::try_from(operation_id)
        .expect("compiled applications row operation id must be valid");
    domain::effective_console_row_scope(policies, &applications_console_group(), &operation_id)
}

pub(crate) fn resolve_application_console_visibility(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> Result<ApplicationVisibility, ControlPlaneError> {
    match effective_application_row_scope(policies, operation_id) {
        domain::ConsoleOperationRowScope::ScopeAll => Ok(ApplicationVisibility::All),
        domain::ConsoleOperationRowScope::Own => Ok(ApplicationVisibility::Own),
        domain::ConsoleOperationRowScope::Disabled => {
            Err(ControlPlaneError::PermissionDenied("permission_denied"))
        }
    }
}

pub(crate) fn ensure_application_console_row_scope(
    actor: &domain::ActorContext,
    application: &domain::ApplicationRecord,
    scope: domain::ConsoleOperationRowScope,
) -> Result<(), ControlPlaneError> {
    match scope {
        domain::ConsoleOperationRowScope::ScopeAll => Ok(()),
        domain::ConsoleOperationRowScope::Own if application.created_by == actor.user_id => Ok(()),
        domain::ConsoleOperationRowScope::Own | domain::ConsoleOperationRowScope::Disabled => {
            Err(ControlPlaneError::PermissionDenied("permission_denied"))
        }
    }
}

pub(crate) fn ensure_application_console_simple_operation(
    policies: &[domain::RoleConsolePolicy],
    operation_id: &str,
) -> Result<(), ControlPlaneError> {
    let operation_id = domain::ConsoleOperationId::try_from(operation_id)
        .expect("compiled applications simple operation id must be valid");
    if domain::effective_console_simple_operation(
        policies,
        &applications_console_group(),
        &operation_id,
    ) {
        Ok(())
    } else {
        Err(ControlPlaneError::PermissionDenied("permission_denied"))
    }
}

pub(super) fn normalize_required_text(
    value: &str,
    field: &'static str,
) -> Result<String, ControlPlaneError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field));
    }

    Ok(normalized.to_string())
}

pub(super) fn normalize_optional_text(value: String) -> Option<String> {
    let normalized = value.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

pub(super) fn dedupe_tag_ids(tag_ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for tag_id in tag_ids {
        if seen.insert(tag_id) {
            deduped.push(tag_id);
        }
    }

    deduped
}

pub(super) fn normalize_environment_variables(
    variables: Vec<ApplicationEnvironmentVariableInput>,
) -> Result<Vec<ApplicationEnvironmentVariableInput>, ControlPlaneError> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(variables.len());

    for variable in variables {
        let name = normalize_environment_variable_name(&variable.name)?;
        if !seen.insert(name.clone()) {
            return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
        }

        let value_type = normalize_environment_variable_value_type(&variable.value_type)?;
        ensure_environment_variable_value_matches_type(&value_type, &variable.value)?;
        normalized.push(ApplicationEnvironmentVariableInput {
            name,
            value_type,
            value: variable.value,
            description: variable.description.trim().to_string(),
        });
    }

    Ok(normalized)
}

pub(super) fn normalize_environment_variable_name(
    value: &str,
) -> Result<String, ControlPlaneError> {
    let name = value.trim();
    let mut chars = name.chars();

    if !chars.next().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
    }

    if !chars.all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(ControlPlaneError::InvalidInput("environment_variable.name"));
    }

    Ok(name.to_string())
}

pub(super) fn normalize_environment_variable_value_type(
    value: &str,
) -> Result<String, ControlPlaneError> {
    let value_type = value.trim();
    let allowed = [
        "string",
        "number",
        "boolean",
        "object",
        "array[string]",
        "array[number]",
        "array[boolean]",
        "array[object]",
    ];

    if allowed.contains(&value_type) {
        Ok(value_type.to_string())
    } else {
        Err(ControlPlaneError::InvalidInput(
            "environment_variable.value_type",
        ))
    }
}

pub(super) fn ensure_environment_variable_value_matches_type(
    value_type: &str,
    value: &serde_json::Value,
) -> Result<(), ControlPlaneError> {
    let valid = match value_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array[string]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
        "array[number]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_number)),
        "array[boolean]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_boolean)),
        "array[object]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_object)),
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput(
            "environment_variable.value",
        ))
    }
}
