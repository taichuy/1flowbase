use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

impl ApiEventInput for RoleAccessInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListDataModelOptions
            | Self::GetConsolePolicyCatalog { locale: _ }
            | Self::ListRoles
            | Self::ListPermissionOptions => json!({}),
            Self::ReplaceConsoleSettingsOrder { locale: _, body } => {
                json!({"body":event::to_value(body)?})
            }
            Self::GetRoleConsolePolicy { role_code }
            | Self::DeleteRole { role_code }
            | Self::GetRolePermissions { role_code }
            | Self::GetRoleFrontstageRoutes { role_code }
            | Self::GetRoleDataPolicy { role_code } => json!({"path":{"id":role_code}}),
            Self::ReplaceRoleConsolePolicy { role_code, body } => {
                json!({"path":{"id":role_code},"body":event::to_value(body)?})
            }
            Self::CreateRole(body) => json!({"body":event::to_value(body)?}),
            Self::UpdateRole { role_code, body } => {
                json!({"path":{"id":role_code},"body":event::to_value(body)?})
            }
            Self::ReplaceRolePermissions { role_code, body } => {
                json!({"path":{"id":role_code},"body":event::to_value(body)?})
            }
            Self::ReplaceRoleFrontstageRoutes { role_code, body } => {
                json!({"path":{"id":role_code},"body":event::to_value(body)?})
            }
            Self::ReplaceRoleDataPolicy { role_code, body } => {
                json!({"path":{"id":role_code},"body":event::to_value(body)?})
            }
        })
    }
}
impl ApiEventOutput for RoleAccessOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::DataModelOptions(value) => event::to_value(value),
            Self::ConsolePolicyCatalog(value) => event::to_value(value),
            Self::RoleConsolePolicy(value) => event::to_value(value),
            Self::Roles(value) => event::to_value(value),
            Self::Role(value) => event::to_value(value),
            Self::RolePermissions(value) => event::to_value(value),
            Self::RoleFrontstageRoutes(value) => event::to_value(value),
            Self::RoleDataPolicy(value) => event::to_value(value),
            Self::PermissionOptions(value) => event::to_value(value),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
