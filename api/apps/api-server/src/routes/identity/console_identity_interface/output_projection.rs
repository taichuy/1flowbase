use super::*;

impl InterfaceContract for ConsoleIdentityOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("Session")),
                (
                    "0",
                    mp::object_schema([
                        ("actor", mp::json_summary_schema()),
                        (
                            "available_roles",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("code",mp::text_schema()), ("name",mp::object_schema([("byte_count",mp::count_schema())])), ("scope_kind",mp::object_schema([("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "active_role_permissions",
                            mp::object_schema([("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Me")),
                (
                    "0",
                    mp::object_schema([
                        ("id", mp::text_schema()),
                        (
                            "nickname",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "preferred_locale",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("meta", mp::json_summary_schema()),
                        (
                            "effective_display_role",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "permissions",
                            mp::object_schema([("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UserApiKeys")),
                (
                    "0",
                    mp::object_schema([(
                        "items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("name",mp::object_schema([("byte_count",mp::count_schema())])), ("key_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("role_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("creator_user_id",mp::text_schema()), ("tenant_id",mp::text_schema()), ("scope_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("scope_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("revoked",serde_json::json!({"type":"boolean"})), ("expires_at",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_used_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                    )]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UserApiKeyRoleOptions")),
                (
                    "0",
                    mp::object_schema([(
                        "items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("code",mp::text_schema()), ("name",mp::object_schema([("byte_count",mp::count_schema())])), ("scope_kind",mp::object_schema([("byte_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UserApiKeyCreated")),
                (
                    "0",
                    mp::object_schema([
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "key_kind",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "role_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("creator_user_id", mp::text_schema()),
                        ("tenant_id", mp::text_schema()),
                        (
                            "scope_kind",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("scope_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("revoked", serde_json::json!({"type":"boolean"})),
                        (
                            "expires_at",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "last_used_at",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UserApiKeyRevoked")),
                ("0", mp::object_schema([("id", mp::text_schema())])),
            ]),
            mp::object_schema([("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Session(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("Session".to_owned())),
                (
                    "0",
                    mp::object_value([
                        ("actor", mp::json_summary(&(_field_0).actor)),
                        ("available_roles", {
                            if (_field_0).available_roles.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .available_roles
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value([
                                            ("code", mp::text(&(item).code)?),
                                            (
                                                "name",
                                                mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).name.len()),
                                                )]),
                                            ),
                                            (
                                                "scope_kind",
                                                mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).scope_kind.len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "active_role_permissions",
                            mp::object_value([(
                                "item_count",
                                serde_json::json!((_field_0).active_role_permissions.len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Me(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("Me".to_owned())),
                (
                    "0",
                    mp::object_value([
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "nickname",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).nickname.len()),
                            )]),
                        ),
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
                            "preferred_locale",
                            match (_field_0).preferred_locale.as_ref() {
                                Some(item) => mp::object_value([(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("meta", mp::json_summary(&(_field_0).meta)),
                        (
                            "effective_display_role",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).effective_display_role.len()),
                            )]),
                        ),
                        (
                            "permissions",
                            mp::object_value([(
                                "item_count",
                                serde_json::json!((_field_0).permissions.len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UserApiKeys(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("UserApiKeys".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([("items", {
                        if (_field_0).items.len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (_field_0)
                                .items
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value([
                                        ("id", serde_json::Value::String((item).id.to_string())),
                                        (
                                            "name",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).name.len()),
                                            )]),
                                        ),
                                        (
                                            "key_kind",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).key_kind.len()),
                                            )]),
                                        ),
                                        (
                                            "role_code",
                                            match (item).role_code.as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "creator_user_id",
                                            serde_json::Value::String(
                                                (item).creator_user_id.to_string(),
                                            ),
                                        ),
                                        (
                                            "tenant_id",
                                            serde_json::Value::String((item).tenant_id.to_string()),
                                        ),
                                        (
                                            "scope_kind",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).scope_kind.len()),
                                            )]),
                                        ),
                                        (
                                            "scope_id",
                                            serde_json::Value::String((item).scope_id.to_string()),
                                        ),
                                        ("enabled", serde_json::Value::Bool((item).enabled)),
                                        ("revoked", serde_json::Value::Bool((item).revoked)),
                                        (
                                            "expires_at",
                                            match (item).expires_at.as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "last_used_at",
                                            match (item).last_used_at.as_ref() {
                                                Some(item) => mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        ("created_at", mp::text(&(item).created_at)?),
                                        ("updated_at", mp::text(&(item).updated_at)?),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::UserApiKeyRoleOptions(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyRoleOptions".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([("items", {
                        if (_field_0).items.len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (_field_0)
                                .items
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value([
                                        ("code", mp::text(&(item).code)?),
                                        (
                                            "name",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).name.len()),
                                            )]),
                                        ),
                                        (
                                            "scope_kind",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).scope_kind.len()),
                                            )]),
                                        ),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::UserApiKeyCreated(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyCreated".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([
                        ("id", serde_json::Value::String((_field_0).id.to_string())),
                        (
                            "name",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).name.len()),
                            )]),
                        ),
                        (
                            "key_kind",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).key_kind.len()),
                            )]),
                        ),
                        (
                            "role_code",
                            match (_field_0).role_code.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "creator_user_id",
                            serde_json::Value::String((_field_0).creator_user_id.to_string()),
                        ),
                        (
                            "tenant_id",
                            serde_json::Value::String((_field_0).tenant_id.to_string()),
                        ),
                        (
                            "scope_kind",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).scope_kind.len()),
                            )]),
                        ),
                        (
                            "scope_id",
                            serde_json::Value::String((_field_0).scope_id.to_string()),
                        ),
                        ("enabled", serde_json::Value::Bool((_field_0).enabled)),
                        ("revoked", serde_json::Value::Bool((_field_0).revoked)),
                        (
                            "expires_at",
                            match (_field_0).expires_at.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_used_at",
                            match (_field_0).last_used_at.as_ref() {
                                Some(item) => mp::object_value([(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::UserApiKeyRevoked(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyRevoked".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([(
                        "id",
                        serde_json::Value::String((_field_0).id.to_string()),
                    )]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value([("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-identity-output";
    const CONTRACT_VERSION: &'static str = "1";
}
