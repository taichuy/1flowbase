use super::*;

impl InterfaceContract for RoleAccessInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([("variant", mp::tag_schema("ListDataModelOptions"))]),
            mp::object_schema([("variant", mp::tag_schema("GetConsolePolicyCatalog"))]),
            mp::object_schema([
                ("variant", mp::tag_schema("ReplaceConsoleSettingsOrder")),
                (
                    "body",
                    mp::object_schema([
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                        (
                            "group_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("GetRoleConsolePolicy")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ReplaceRoleConsolePolicy")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([(
                        "groups",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("kind",mp::union_schema(vec![mp::object_schema([("variant",mp::tag_schema("SettingsFeature"))]), mp::object_schema([("variant",mp::tag_schema("Other"))])])), ("group_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("strategy",mp::union_schema(vec![mp::object_schema([("variant",mp::tag_schema("Full"))]), mp::object_schema([("variant",mp::tag_schema("Custom"))])])), ("operations",mp::object_schema([("item_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema([("variant", mp::tag_schema("ListRoles"))]),
            mp::object_schema([
                ("variant", mp::tag_schema("CreateRole")),
                (
                    "0",
                    mp::object_schema([
                        ("code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "auto_grant_new_permissions",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "is_default_member_role",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UpdateRole")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "auto_grant_new_permissions",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "is_default_member_role",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("DeleteRole")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("GetRolePermissions")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ReplaceRolePermissions")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([(
                        "permission_codes",
                        mp::object_schema([("item_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("GetRoleFrontstageRoutes")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ReplaceRoleFrontstageRoutes")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "page_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "tab_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("GetRoleDataPolicy")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ReplaceRoleDataPolicy")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "default_policy",
                            mp::object_schema([
                                ("can_view", serde_json::json!({"type":"boolean"})),
                                ("can_create", serde_json::json!({"type":"boolean"})),
                                ("can_update", serde_json::json!({"type":"boolean"})),
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                                (
                                    "default_view_scope",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "model_policies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("data_model_id",mp::text_schema()), ("can_create_override",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("view_scope_override",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("update_scope_override",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("delete_scope_override",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([("variant", mp::tag_schema("ListPermissionOptions"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListDataModelOptions => mp::object_value([(
                "variant",
                serde_json::Value::String("ListDataModelOptions".to_owned()),
            )]),
            Self::GetConsolePolicyCatalog { .. } => mp::object_value([(
                "variant",
                serde_json::Value::String("GetConsolePolicyCatalog".to_owned()),
            )]),
            Self::ReplaceConsoleSettingsOrder {
                body: _field_body, ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReplaceConsoleSettingsOrder".to_owned()),
                ),
                (
                    "body",
                    mp::object_value([
                        (
                            "expected_revision",
                            serde_json::json!((_field_body).expected_revision),
                        ),
                        ("group_ids", {
                            if (_field_body).group_ids.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_body)
                                    .group_ids
                                    .iter()
                                    .map(|item| mp::text(item))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::GetRoleConsolePolicy {
                role_code: _field_role_code,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("GetRoleConsolePolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::ReplaceRoleConsolePolicy {
                role_code: _field_role_code,
                body: _field_body,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleConsolePolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value([("groups", {
                        if (_field_body).groups.len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array((_field_body).groups.iter().map(|item| Some(mp::object_value([("kind",match &(item).kind {crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::SettingsFeature => mp::object_value([("variant",serde_json::Value::String("SettingsFeature".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::Other => mp::object_value([("variant",serde_json::Value::String("Other".to_owned()))])}), ("group_id",mp::text(&(item).group_id)?), ("enabled",serde_json::Value::Bool((item).enabled)), ("strategy",match &(item).strategy {crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Full => mp::object_value([("variant",serde_json::Value::String("Full".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Custom => mp::object_value([("variant",serde_json::Value::String("Custom".to_owned()))])}), ("operations",mp::object_value([("item_count",serde_json::json!((item).operations.len()))]))]))).collect::<Option<Vec<_>>>()?)
                    })]),
                ),
            ]),
            Self::ListRoles => {
                mp::object_value([("variant", serde_json::Value::String("ListRoles".to_owned()))])
            }
            Self::CreateRole(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("CreateRole".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([
                        ("code", mp::text(&(_field_0).code)?),
                        (
                            "name",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).name.len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).introduction.len()),
                            )]),
                        ),
                        (
                            "auto_grant_new_permissions",
                            match (_field_0).auto_grant_new_permissions.as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_default_member_role",
                            match (_field_0).is_default_member_role.as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::UpdateRole {
                role_code: _field_role_code,
                body: _field_body,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("UpdateRole".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value([
                        (
                            "name",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_body).name.len()),
                            )]),
                        ),
                        (
                            "introduction",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_body).introduction.len()),
                            )]),
                        ),
                        (
                            "auto_grant_new_permissions",
                            match (_field_body).auto_grant_new_permissions.as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_default_member_role",
                            match (_field_body).is_default_member_role.as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::DeleteRole {
                role_code: _field_role_code,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("DeleteRole".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::GetRolePermissions {
                role_code: _field_role_code,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("GetRolePermissions".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::ReplaceRolePermissions {
                role_code: _field_role_code,
                body: _field_body,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReplaceRolePermissions".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value([(
                        "permission_codes",
                        mp::object_value([(
                            "item_count",
                            serde_json::json!((_field_body).permission_codes.len()),
                        )]),
                    )]),
                ),
            ]),
            Self::GetRoleFrontstageRoutes {
                role_code: _field_role_code,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("GetRoleFrontstageRoutes".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::ReplaceRoleFrontstageRoutes {
                role_code: _field_role_code,
                body: _field_body,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleFrontstageRoutes".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value([
                        ("page_ids", {
                            if (_field_body).page_ids.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_body)
                                    .page_ids
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("tab_ids", {
                            if (_field_body).tab_ids.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_body)
                                    .tab_ids
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::GetRoleDataPolicy {
                role_code: _field_role_code,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("GetRoleDataPolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::ReplaceRoleDataPolicy {
                role_code: _field_role_code,
                body: _field_body,
                ..
            } => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleDataPolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value([
                        (
                            "default_policy",
                            mp::object_value([
                                (
                                    "can_view",
                                    serde_json::Value::Bool((_field_body).default_policy.can_view),
                                ),
                                (
                                    "can_create",
                                    serde_json::Value::Bool(
                                        (_field_body).default_policy.can_create,
                                    ),
                                ),
                                (
                                    "can_update",
                                    serde_json::Value::Bool(
                                        (_field_body).default_policy.can_update,
                                    ),
                                ),
                                (
                                    "can_delete",
                                    serde_json::Value::Bool(
                                        (_field_body).default_policy.can_delete,
                                    ),
                                ),
                                (
                                    "default_view_scope",
                                    mp::object_value([(
                                        "byte_count",
                                        serde_json::json!((_field_body)
                                            .default_policy
                                            .default_view_scope
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_value([(
                                        "byte_count",
                                        serde_json::json!((_field_body)
                                            .default_policy
                                            .default_update_scope
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_value([(
                                        "byte_count",
                                        serde_json::json!((_field_body)
                                            .default_policy
                                            .default_delete_scope
                                            .len()),
                                    )]),
                                ),
                            ]),
                        ),
                        ("model_policies", {
                            if (_field_body).model_policies.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_body)
                                    .model_policies
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value([
                                            (
                                                "data_model_id",
                                                serde_json::Value::String(
                                                    (item).data_model_id.to_string(),
                                                ),
                                            ),
                                            (
                                                "can_create_override",
                                                match (item).can_create_override.as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "view_scope_override",
                                                match (item).view_scope_override.as_ref() {
                                                    Some(item) => mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "update_scope_override",
                                                match (item).update_scope_override.as_ref() {
                                                    Some(item) => mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "delete_scope_override",
                                                match (item).delete_scope_override.as_ref() {
                                                    Some(item) => mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::ListPermissionOptions => mp::object_value([(
                "variant",
                serde_json::Value::String("ListPermissionOptions".to_owned()),
            )]),
        })
    }

    const CONTRACT_ID: &'static str = "console-role-access-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[path = "managed_projection/output_projection.rs"]
mod output_projection;
