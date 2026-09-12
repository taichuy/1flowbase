use super::*;

impl InterfaceContract for RoleAccessInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ListDataModelOptions"))]),
            mp::object_schema(&[("variant", mp::tag_schema("GetConsolePolicyCatalog"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceConsoleSettingsOrder")),
                (
                    "body",
                    mp::object_schema(&[
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                        (
                            "group_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRoleConsolePolicy")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceRoleConsolePolicy")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "groups",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("SettingsFeature"))]), mp::object_schema(&[("variant",mp::tag_schema("Other"))])])), ("group_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("strategy",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Full"))]), mp::object_schema(&[("variant",mp::tag_schema("Custom"))])])), ("operations",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListRoles"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateRole")),
                (
                    "0",
                    mp::object_schema(&[
                        ("code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateRole")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteRole")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRolePermissions")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceRolePermissions")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "permission_codes",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRoleFrontstageRoutes")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceRoleFrontstageRoutes")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRoleDataPolicy")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceRoleDataPolicy")),
                ("role_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "default_policy",
                            mp::object_schema(&[
                                ("can_view", serde_json::json!({"type":"boolean"})),
                                ("can_create", serde_json::json!({"type":"boolean"})),
                                ("can_update", serde_json::json!({"type":"boolean"})),
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                                (
                                    "default_view_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "model_policies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("data_model_id",mp::text_schema()), ("can_create_override",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("view_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("update_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("delete_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListPermissionOptions"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListDataModelOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListDataModelOptions".to_owned()),
            )]),
            Self::GetConsolePolicyCatalog { .. } => mp::object_value(&[(
                "variant",
                serde_json::Value::String("GetConsolePolicyCatalog".to_owned()),
            )]),
            Self::ReplaceConsoleSettingsOrder {
                body: _field_body, ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceConsoleSettingsOrder".to_owned()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_body).expected_revision)),
                        ),
                        ("group_ids", {
                            if (&(_field_body).group_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).group_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::GetRoleConsolePolicy {
                role_code: _field_role_code,
                ..
            } => mp::object_value(&[
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleConsolePolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value(&[("groups", {
                        if (&(_field_body).groups).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array((&(_field_body).groups).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::SettingsFeature => mp::object_value(&[("variant",serde_json::Value::String("SettingsFeature".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::Other => mp::object_value(&[("variant",serde_json::Value::String("Other".to_owned()))])}), ("group_id",mp::text(&(item).group_id)?), ("enabled",serde_json::Value::Bool(*(&(item).enabled))), ("strategy",match &(item).strategy {crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Full => mp::object_value(&[("variant",serde_json::Value::String("Full".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Custom => mp::object_value(&[("variant",serde_json::Value::String("Custom".to_owned()))])}), ("operations",mp::object_value(&[("item_count",serde_json::json!((&(item).operations).len()))]))]))).collect::<Option<Vec<_>>>()?)
                    })]),
                ),
            ]),
            Self::ListRoles => {
                mp::object_value(&[("variant", serde_json::Value::String("ListRoles".to_owned()))])
            }
            Self::CreateRole(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateRole".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("code", mp::text(&(_field_0).code)?),
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
                        (
                            "auto_grant_new_permissions",
                            match (&(_field_0).auto_grant_new_permissions).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_default_member_role",
                            match (&(_field_0).is_default_member_role).as_ref() {
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateRole".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
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
                            "introduction",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).introduction).len()),
                            )]),
                        ),
                        (
                            "auto_grant_new_permissions",
                            match (&(_field_body).auto_grant_new_permissions).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_default_member_role",
                            match (&(_field_body).is_default_member_role).as_ref() {
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteRole".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::GetRolePermissions {
                role_code: _field_role_code,
                ..
            } => mp::object_value(&[
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceRolePermissions".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value(&[(
                        "permission_codes",
                        mp::object_value(&[(
                            "item_count",
                            serde_json::json!((&(_field_body).permission_codes).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::GetRoleFrontstageRoutes {
                role_code: _field_role_code,
                ..
            } => mp::object_value(&[
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleFrontstageRoutes".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value(&[
                        ("page_ids", {
                            if (&(_field_body).page_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).page_ids)
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("tab_ids", {
                            if (&(_field_body).tab_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).tab_ids)
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
            } => mp::object_value(&[
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
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReplaceRoleDataPolicy".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "default_policy",
                            mp::object_value(&[
                                (
                                    "can_view",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_body).default_policy).can_view),
                                    ),
                                ),
                                (
                                    "can_create",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_body).default_policy).can_create),
                                    ),
                                ),
                                (
                                    "can_update",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_body).default_policy).can_update),
                                    ),
                                ),
                                (
                                    "can_delete",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_body).default_policy).can_delete),
                                    ),
                                ),
                                (
                                    "default_view_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_body).default_policy)
                                            .default_view_scope)
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_body).default_policy)
                                            .default_update_scope)
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_body).default_policy)
                                            .default_delete_scope)
                                            .len()),
                                    )]),
                                ),
                            ]),
                        ),
                        ("model_policies", {
                            if (&(_field_body).model_policies).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).model_policies)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "data_model_id",
                                                serde_json::Value::String(
                                                    (&(item).data_model_id).to_string(),
                                                ),
                                            ),
                                            (
                                                "can_create_override",
                                                match (&(item).can_create_override).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "view_scope_override",
                                                match (&(item).view_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "update_scope_override",
                                                match (&(item).update_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "delete_scope_override",
                                                match (&(item).delete_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
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
            Self::ListPermissionOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListPermissionOptions".to_owned()),
            )]),
        })
    }

    const CONTRACT_ID: &'static str = "console-role-access-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for RoleAccessOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("DataModelOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ConsolePolicyCatalog")),
                (
                    "0",
                    mp::object_schema(&[
                        ("schema_version", mp::text_schema()),
                        ("locale", mp::text_schema()),
                        (
                            "settings_order_revision",
                            serde_json::json!({"type":"integer"}),
                        ),
                        (
                            "group_strategy_options",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("value",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "groups",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("SettingsFeature"))]), mp::object_schema(&[("variant",mp::tag_schema("Other"))])])), ("group_id",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("operations",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                        (
                            "resources",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("resource_code",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("actions",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RoleConsolePolicy")),
                (
                    "0",
                    mp::object_schema(&[
                        ("role_code", mp::text_schema()),
                        (
                            "groups",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("SettingsFeature"))]), mp::object_schema(&[("variant",mp::tag_schema("Other"))])])), ("group_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("strategy",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Full"))]), mp::object_schema(&[("variant",mp::tag_schema("Custom"))])])), ("operations",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Roles")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("introduction",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("is_builtin",serde_json::json!({"type":"boolean"})), ("is_editable",serde_json::json!({"type":"boolean"})), ("auto_grant_new_permissions",serde_json::json!({"type":"boolean"})), ("is_default_member_role",serde_json::json!({"type":"boolean"})), ("permission_codes",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Role")),
                (
                    "0",
                    mp::object_schema(&[
                        ("code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("is_builtin", serde_json::json!({"type":"boolean"})),
                        ("is_editable", serde_json::json!({"type":"boolean"})),
                        (
                            "auto_grant_new_permissions",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        (
                            "is_default_member_role",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        (
                            "permission_codes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RolePermissions")),
                (
                    "0",
                    mp::object_schema(&[
                        ("role_code", mp::text_schema()),
                        (
                            "permission_codes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RoleFrontstageRoutes")),
                (
                    "0",
                    mp::object_schema(&[
                        ("role_code", mp::text_schema()),
                        (
                            "checked_page_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "checked_tab_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "tree",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("kind",mp::text_schema()), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("slug",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("children",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RoleDataPolicy")),
                (
                    "0",
                    mp::object_schema(&[
                        ("role_code", mp::text_schema()),
                        (
                            "default_policy",
                            mp::object_schema(&[
                                ("can_view", serde_json::json!({"type":"boolean"})),
                                ("can_create", serde_json::json!({"type":"boolean"})),
                                ("can_update", serde_json::json!({"type":"boolean"})),
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                                (
                                    "default_view_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "model_policies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("data_model_id",mp::text_schema()), ("can_create_override",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("view_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("update_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("delete_scope_override",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PermissionOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("resource",mp::object_schema(&[("byte_count",mp::count_schema())])), ("action",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope",mp::object_schema(&[("byte_count",mp::count_schema())])), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("settings_feature",serde_json::json!({"anyOf": [mp::object_schema(&[("feature_id",mp::text_schema()), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("order",serde_json::json!({"type":"integer"}))]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::DataModelOptions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DataModelOptions".to_owned()),
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
                                    ("id", serde_json::Value::String((&(item).id).to_string())),
                                    ("code", mp::text(&(item).code)?),
                                    (
                                        "title",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).title).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::ConsolePolicyCatalog(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ConsolePolicyCatalog".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("schema_version", mp::text(&(_field_0).schema_version)?),
                        ("locale", mp::text(&(_field_0).locale)?),
                        (
                            "settings_order_revision",
                            serde_json::json!(*(&(_field_0).settings_order_revision)),
                        ),
                        ("group_strategy_options", {
                            if (&(_field_0).group_strategy_options).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).group_strategy_options)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "value",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).value).len()),
                                                )]),
                                            ),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).label).len()),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).description).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("groups", {
                            if (&(_field_0).groups).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).groups).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::SettingsFeature => mp::object_value(&[("variant",serde_json::Value::String("SettingsFeature".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::Other => mp::object_value(&[("variant",serde_json::Value::String("Other".to_owned()))])}), ("group_id",mp::text(&(item).group_id)?), ("label",mp::object_value(&[("byte_count",serde_json::json!((&(item).label).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("operations",mp::object_value(&[("item_count",serde_json::json!((&(item).operations).len()))]))]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("resources", {
                            if (&(_field_0).resources).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).resources)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("resource_code", mp::text(&(item).resource_code)?),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).label).len()),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).description).len()),
                                                )]),
                                            ),
                                            (
                                                "actions",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).actions).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::RoleConsolePolicy(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RoleConsolePolicy".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("role_code", mp::text(&(_field_0).role_code)?),
                        ("groups", {
                            if (&(_field_0).groups).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).groups).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::SettingsFeature => mp::object_value(&[("variant",serde_json::Value::String("SettingsFeature".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyGroupKindBody::Other => mp::object_value(&[("variant",serde_json::Value::String("Other".to_owned()))])}), ("group_id",mp::text(&(item).group_id)?), ("enabled",serde_json::Value::Bool(*(&(item).enabled))), ("strategy",match &(item).strategy {crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Full => mp::object_value(&[("variant",serde_json::Value::String("Full".to_owned()))]), crate::routes::settings_group::roles::ConsolePolicyStrategyBody::Custom => mp::object_value(&[("variant",serde_json::Value::String("Custom".to_owned()))])}), ("operations",mp::object_value(&[("item_count",serde_json::json!((&(item).operations).len()))]))]))).collect::<Option<Vec<_>>>()?)
                        }),
                    ]),
                ),
            ]),
            Self::Roles(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Roles".to_owned())),
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
                                    (
                                        "introduction",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).introduction).len()),
                                        )]),
                                    ),
                                    (
                                        "scope_kind",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).scope_kind).len()),
                                        )]),
                                    ),
                                    ("is_builtin", serde_json::Value::Bool(*(&(item).is_builtin))),
                                    (
                                        "is_editable",
                                        serde_json::Value::Bool(*(&(item).is_editable)),
                                    ),
                                    (
                                        "auto_grant_new_permissions",
                                        serde_json::Value::Bool(
                                            *(&(item).auto_grant_new_permissions),
                                        ),
                                    ),
                                    (
                                        "is_default_member_role",
                                        serde_json::Value::Bool(*(&(item).is_default_member_role)),
                                    ),
                                    (
                                        "permission_codes",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).permission_codes).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Role(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Role".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("code", mp::text(&(_field_0).code)?),
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
                        (
                            "scope_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).scope_kind).len()),
                            )]),
                        ),
                        (
                            "is_builtin",
                            serde_json::Value::Bool(*(&(_field_0).is_builtin)),
                        ),
                        (
                            "is_editable",
                            serde_json::Value::Bool(*(&(_field_0).is_editable)),
                        ),
                        (
                            "auto_grant_new_permissions",
                            serde_json::Value::Bool(*(&(_field_0).auto_grant_new_permissions)),
                        ),
                        (
                            "is_default_member_role",
                            serde_json::Value::Bool(*(&(_field_0).is_default_member_role)),
                        ),
                        (
                            "permission_codes",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).permission_codes).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::RolePermissions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RolePermissions".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("role_code", mp::text(&(_field_0).role_code)?),
                        (
                            "permission_codes",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).permission_codes).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::RoleFrontstageRoutes(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RoleFrontstageRoutes".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("role_code", mp::text(&(_field_0).role_code)?),
                        ("checked_page_ids", {
                            if (&(_field_0).checked_page_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).checked_page_ids)
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("checked_tab_ids", {
                            if (&(_field_0).checked_tab_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).checked_tab_ids)
                                    .iter()
                                    .map(|item| Some(serde_json::Value::String((item).to_string())))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("tree", {
                            if (&(_field_0).tree).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).tree)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                serde_json::Value::String((&(item).id).to_string()),
                                            ),
                                            ("kind", mp::text(&(item).kind)?),
                                            (
                                                "title",
                                                match (&(item).title).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "slug",
                                                match (&(item).slug).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "children",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).children).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::RoleDataPolicy(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RoleDataPolicy".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("role_code", mp::text(&(_field_0).role_code)?),
                        (
                            "default_policy",
                            mp::object_value(&[
                                (
                                    "can_view",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).default_policy).can_view),
                                    ),
                                ),
                                (
                                    "can_create",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).default_policy).can_create),
                                    ),
                                ),
                                (
                                    "can_update",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).default_policy).can_update),
                                    ),
                                ),
                                (
                                    "can_delete",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).default_policy).can_delete),
                                    ),
                                ),
                                (
                                    "default_view_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).default_policy)
                                            .default_view_scope)
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_update_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).default_policy)
                                            .default_update_scope)
                                            .len()),
                                    )]),
                                ),
                                (
                                    "default_delete_scope",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).default_policy)
                                            .default_delete_scope)
                                            .len()),
                                    )]),
                                ),
                            ]),
                        ),
                        ("model_policies", {
                            if (&(_field_0).model_policies).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).model_policies)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "data_model_id",
                                                serde_json::Value::String(
                                                    (&(item).data_model_id).to_string(),
                                                ),
                                            ),
                                            (
                                                "can_create_override",
                                                match (&(item).can_create_override).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "view_scope_override",
                                                match (&(item).view_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "update_scope_override",
                                                match (&(item).update_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "delete_scope_override",
                                                match (&(item).delete_scope_override).as_ref() {
                                                    Some(item) => mp::object_value(&[(
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
            Self::PermissionOptions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PermissionOptions".to_owned()),
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
                                        "resource",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).resource).len()),
                                        )]),
                                    ),
                                    (
                                        "action",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).action).len()),
                                        )]),
                                    ),
                                    (
                                        "scope",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).scope).len()),
                                        )]),
                                    ),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "settings_feature",
                                        match (&(item).settings_feature).as_ref() {
                                            Some(item) => mp::object_value(&[
                                                ("feature_id", mp::text(&(item).feature_id)?),
                                                (
                                                    "label_key",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!(
                                                            (&(item).label_key).len()
                                                        ),
                                                    )]),
                                                ),
                                                ("order", serde_json::json!(*(&(item).order))),
                                            ]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
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

    const CONTRACT_ID: &'static str = "console-role-access-output";
    const CONTRACT_VERSION: &'static str = "1";
}
