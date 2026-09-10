use std::sync::Arc;

use control_plane::{
    member::{
        CreateMemberCommand, DeleteMemberCommand, DisableMemberCommand, EnableMemberCommand,
        MemberService, ReplaceMemberRolesCommand, ResetMemberPasswordCommand, UpdateMemberCommand,
    },
    workspace::{UpdateWorkspaceCommand, WorkspaceService},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{members, workspace, workspaces};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum MembershipInput {
    ListMemberRoleOptions,
    ListMembers,
    CreateMember(members::CreateMemberBody),
    UpdateMember {
        member_id: String,
        body: members::UpdateMemberBody,
    },
    DisableMember {
        member_id: String,
    },
    EnableMember {
        member_id: String,
    },
    DeleteMember {
        member_id: String,
    },
    ResetMember {
        member_id: String,
        body: members::ResetMemberPasswordBody,
    },
    ReplaceMemberRoles {
        member_id: String,
        body: members::ReplaceMemberRolesBody,
    },
    GetWorkspace,
    PatchWorkspace(workspace::PatchWorkspaceBody),
    ListWorkspaces,
}

impl InterfaceContract for MembershipInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ListMemberRoleOptions"))]),
            mp::object_schema(&[("variant", mp::tag_schema("ListMembers"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateMember")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "nickname",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateMember")),
                ("member_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "nickname",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DisableMember")),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("EnableMember")),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteMember")),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResetMember")),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceMemberRoles")),
                ("member_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "role_codes",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("GetWorkspace"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PatchWorkspace")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListWorkspaces"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListMemberRoleOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListMemberRoleOptions".to_owned()),
            )]),
            Self::ListMembers => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListMembers".to_owned()),
            )]),
            Self::CreateMember(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateMember".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "nickname",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).nickname).len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).introduction).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateMember {
                member_id: _field_member_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateMember".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).name).len()),
                            )]),
                        ),
                        (
                            "nickname",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).nickname).len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).introduction).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::DisableMember {
                member_id: _field_member_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DisableMember".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
            ]),
            Self::EnableMember {
                member_id: _field_member_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("EnableMember".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
            ]),
            Self::DeleteMember {
                member_id: _field_member_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteMember".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
            ]),
            Self::ResetMember {
                member_id: _field_member_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ResetMember".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
            ]),
            Self::ReplaceMemberRoles {
                member_id: _field_member_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceMemberRoles".to_owned()),
                ),
                ("member_id", mp::text(_field_member_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "role_codes",
                        mp::object_value(&[(
                            "item_count",
                            serde_json::json!((&(_field_body).role_codes).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::GetWorkspace => mp::object_value(&[(
                "variant",
                serde_json::Value::String("GetWorkspace".to_owned()),
            )]),
            Self::PatchWorkspace(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PatchWorkspace".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).introduction).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::ListWorkspaces => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListWorkspaces".to_owned()),
            )]),
        })
    }

    const CONTRACT_ID: &'static str = "console-membership-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum MembershipOutput {
    MemberRoleOptions(Vec<members::MemberRoleOptionResponse>),
    Members(Vec<members::MemberResponse>),
    Member(members::MemberResponse),
    Workspace(workspace::WorkspaceResponse),
    Workspaces(Vec<workspaces::WorkspaceSummaryResponse>),
    NoContent,
}

impl InterfaceContract for MembershipOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("MemberRoleOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Members")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("nickname",mp::object_schema(&[("byte_count",mp::count_schema())])), ("introduction",mp::object_schema(&[("byte_count",mp::count_schema())])), ("default_display_role",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("role_codes",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Member")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "nickname",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "default_display_role",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "role_codes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Workspace")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Workspaces")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("introduction",mp::object_schema(&[("byte_count",mp::count_schema())])), ("is_current",serde_json::json!({"type":"boolean"}))])}),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::MemberRoleOptions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("MemberRoleOptions".to_owned()),
                ),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("code", mp::text(&(item).code)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Members(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Members".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", mp::text(&(item).id)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "nickname",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).nickname).len()),
                                        )]),
                                    ),
                                    (
                                        "introduction",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).introduction).len()),
                                        )]),
                                    ),
                                    (
                                        "default_display_role",
                                        match (&(item).default_display_role).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    (
                                        "role_codes",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).role_codes).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Member(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Member".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "nickname",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).nickname).len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).introduction).len()),
                            )]),
                        ),
                        (
                            "default_display_role",
                            match (&(_field_0).default_display_role).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "role_codes",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).role_codes).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Workspace(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Workspace".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).introduction).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Workspaces(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Workspaces".to_owned()),
                ),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", mp::text(&(item).id)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "introduction",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).introduction).len()),
                                        )]),
                                    ),
                                    ("is_current", serde_json::Value::Bool(*(&(item).is_current))),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-membership-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct MembershipAdapter {
    store: MainDurableStore,
}

pub(crate) fn membership_port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<MembershipInput, MembershipOutput>> {
    Arc::new(MembershipAdapter { store })
}

impl MembershipAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: MembershipInput,
    ) -> Result<MembershipOutput, ApiError> {
        let actor = principal.actor();
        match input {
            MembershipInput::ListMemberRoleOptions => {
                let roles = MemberService::new(self.store.for_actor(actor.clone()))
                    .list_assignable_role_options(actor.user_id)
                    .await?;
                Ok(MembershipOutput::MemberRoleOptions(
                    roles.into_iter().map(Into::into).collect(),
                ))
            }
            MembershipInput::ListMembers => {
                let users = MemberService::new(self.store.for_actor(actor.clone()))
                    .list_members(actor.user_id)
                    .await?;
                Ok(MembershipOutput::Members(
                    users.into_iter().map(members::to_member_response).collect(),
                ))
            }
            MembershipInput::CreateMember(body) => {
                let user = MemberService::new(self.store.for_actor(actor.clone()))
                    .create_member(CreateMemberCommand {
                        actor_user_id: actor.user_id,
                        account: body.account,
                        email: body.email,
                        phone: body.phone,
                        password_hash: members::hash_password(&body.password)?,
                        name: body.name,
                        nickname: body.nickname,
                        introduction: body.introduction,
                        email_login_enabled: body.email_login_enabled,
                        phone_login_enabled: body.phone_login_enabled,
                    })
                    .await?;
                Ok(MembershipOutput::Member(members::to_member_response(user)))
            }
            MembershipInput::UpdateMember { member_id, body } => {
                let user = MemberService::new(self.store.for_actor(actor.clone()))
                    .update_member(UpdateMemberCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                        email: body.email,
                        phone: body.phone,
                        name: body.name,
                        nickname: body.nickname,
                        introduction: body.introduction,
                    })
                    .await?;
                Ok(MembershipOutput::Member(members::to_member_response(user)))
            }
            MembershipInput::DisableMember { member_id } => {
                MemberService::new(self.store.for_actor(actor.clone()))
                    .disable_member(DisableMemberCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                    })
                    .await?;
                Ok(MembershipOutput::NoContent)
            }
            MembershipInput::EnableMember { member_id } => {
                MemberService::new(self.store.for_actor(actor.clone()))
                    .enable_member(EnableMemberCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                    })
                    .await?;
                Ok(MembershipOutput::NoContent)
            }
            MembershipInput::DeleteMember { member_id } => {
                MemberService::new(self.store.for_actor(actor.clone()))
                    .delete_member(DeleteMemberCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                    })
                    .await?;
                Ok(MembershipOutput::NoContent)
            }
            MembershipInput::ResetMember { member_id, body } => {
                MemberService::new(self.store.for_actor(actor.clone()))
                    .reset_member_password(ResetMemberPasswordCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                        password_hash: members::hash_password(&body.new_password)?,
                    })
                    .await?;
                Ok(MembershipOutput::NoContent)
            }
            MembershipInput::ReplaceMemberRoles { member_id, body } => {
                MemberService::new(self.store.for_actor(actor.clone()))
                    .replace_member_roles(ReplaceMemberRolesCommand {
                        actor_user_id: actor.user_id,
                        target_user_id: parse_member_id(&member_id)?,
                        role_codes: body.role_codes,
                    })
                    .await?;
                Ok(MembershipOutput::NoContent)
            }
            MembershipInput::GetWorkspace => {
                let record = WorkspaceService::new(self.store.for_actor(actor.clone()))
                    .get_workspace(actor.current_workspace_id)
                    .await?;
                Ok(MembershipOutput::Workspace(
                    workspace::to_workspace_response(record),
                ))
            }
            MembershipInput::PatchWorkspace(body) => {
                let record = WorkspaceService::new(self.store.for_actor(actor.clone()))
                    .update_workspace(UpdateWorkspaceCommand {
                        actor: actor.clone(),
                        workspace_id: actor.current_workspace_id,
                        name: body.name,
                        logo_url: body.logo_url,
                        introduction: body.introduction,
                    })
                    .await?;
                Ok(MembershipOutput::Workspace(
                    workspace::to_workspace_response(record),
                ))
            }
            MembershipInput::ListWorkspaces => {
                let records = WorkspaceService::new(self.store.for_actor(actor.clone()))
                    .list_accessible_workspaces(actor.user_id)
                    .await?;
                Ok(MembershipOutput::Workspaces(
                    workspaces::to_workspace_summaries(records, actor.current_workspace_id),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<MembershipInput, MembershipOutput> for MembershipAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: MembershipInput,
    ) -> ConsoleInterfaceFuture<'a, MembershipOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn parse_member_id(member_id: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(member_id)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("member_id").into())
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "members.role_options.list",
        binding_id: "http.console.members.role-options.list.v1",
        method: "GET",
        path: "/api/console/settings/members/role-options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.list",
        binding_id: "http.console.members.list.v1",
        method: "GET",
        path: "/api/console/settings/members",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.create",
        binding_id: "http.console.members.create.v1",
        method: "POST",
        path: "/api/console/settings/members",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.update",
        binding_id: "http.console.members.update.v1",
        method: "PATCH",
        path: "/api/console/settings/members/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.disable",
        binding_id: "http.console.members.disable.v1",
        method: "POST",
        path: "/api/console/settings/members/:id/disable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.enable",
        binding_id: "http.console.members.enable.v1",
        method: "POST",
        path: "/api/console/settings/members/:id/enable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.delete",
        binding_id: "http.console.members.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/members/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.password.reset",
        binding_id: "http.console.members.password.reset.v1",
        method: "POST",
        path: "/api/console/settings/members/:id/reset-password",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.roles.replace",
        binding_id: "http.console.members.roles.replace.v1",
        method: "PUT",
        path: "/api/console/settings/members/:id/roles",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "console.workspace.get",
        binding_id: "http.console.workspace.get.v1",
        method: "GET",
        path: "/api/console/workspace",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "workspace.update",
        binding_id: "http.console.workspace.update.v1",
        method: "PATCH",
        path: "/api/console/workspace",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "console.workspaces.list",
        binding_id: "http.console.workspaces.list.v1",
        method: "GET",
        path: "/api/console/workspaces",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<MembershipInput, MembershipOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-membership",
        "graph:console-membership-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableMembershipPort;

#[cfg(test)]
impl ConsoleInterfacePort<MembershipInput, MembershipOutput> for UnavailableMembershipPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: MembershipInput,
    ) -> ConsoleInterfaceFuture<'a, MembershipOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("membership fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f08b_registry_freezes_members_and_workspace_bindings() {
        let registry = compile_registry(Arc::new(UnavailableMembershipPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .expect("declared membership binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
