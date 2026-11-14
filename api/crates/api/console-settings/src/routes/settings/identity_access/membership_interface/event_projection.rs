use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext};
use serde_json::{json, Value};

// Passwords belong to the HTTP/authentication contract, never an event payload.
#[derive(serde::Serialize, utoipa::ToSchema)]
struct CreateMemberEvent<'a> {
    account: &'a str,
    email: &'a str,
    phone: &'a Option<String>,
    name: &'a str,
    nickname: &'a str,
    introduction: &'a str,
    email_login_enabled: bool,
    phone_login_enabled: bool,
}
impl ApiInput for MembershipInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "members.create" => Some(
                json!({"type":"object","properties":{"body":event::schema::<CreateMemberEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            "members.password.reset" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]},"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["path","body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListMemberRoleOptions | Self::GetWorkspace | Self::ListWorkspaces => json!({}),
            Self::ListMembers { department_id } => match department_id {
                Some(id) => json!({"query":{"department_id":id}}),
                None => json!({"query":{}}),
            },
            Self::CreateMember(body) => json!({"body": event::to_value(&CreateMemberEvent {
                account:&body.account, email:&body.email, phone:&body.phone, name:&body.name,
                nickname:&body.nickname, introduction:&body.introduction,
                email_login_enabled:body.email_login_enabled, phone_login_enabled:body.phone_login_enabled,
            })?}),
            Self::UpdateMember { member_id, body } => {
                json!({"path":{"id":member_id},"body":event::to_value(body)?})
            }
            Self::DisableMember { member_id }
            | Self::EnableMember { member_id }
            | Self::DeleteMember { member_id } => json!({"path":{"id":member_id}}),
            Self::ResetMember { member_id, body: _ } => json!({"path":{"id":member_id},"body":{}}),
            Self::ReplaceMemberRoles { member_id, body } => {
                json!({"path":{"id":member_id},"body":event::to_value(body)?})
            }
            Self::PatchWorkspace(body) => {
                json!({"body":{"name":body.name,"logo_url":body.logo_url,"introduction":body.introduction}})
            }
        })
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
