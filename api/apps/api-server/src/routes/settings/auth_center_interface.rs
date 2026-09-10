use std::sync::Arc;

use control_plane::auth::settings::{
    AuthCenterSettingsService, CopyAuthCenterLoginEntryCommand, CreateAuthCenterLoginEntryCommand,
    UpdateAuthCenterLoginEntryConfigCommand, UpdateAuthCenterLoginEntryEnabledCommand,
    UpdateAuthCenterLoginEntryPublicUiBlockCommand,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::auth_center;
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum AuthCenterInput {
    Overview {
        locale: ConsoleLocaleHints,
    },
    Create {
        locale: ConsoleLocaleHints,
        body: auth_center::CreateAuthCenterLoginEntryBody,
    },
    Copy {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::CopyAuthCenterLoginEntryBody,
    },
    Delete {
        id: Uuid,
    },
    Reorder {
        locale: ConsoleLocaleHints,
        body: auth_center::ReorderAuthCenterLoginEntriesBody,
    },
    UpdateEnabled {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::UpdateAuthCenterLoginEntryEnabledBody,
    },
    UpdateConfig {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::UpdateAuthCenterLoginEntryConfigBody,
    },
    UpdatePublicUiBlock {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::UpdateAuthCenterLoginEntryPublicUiBlockBody,
    },
}

impl InterfaceContract for AuthCenterInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Overview"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "body",
                    mp::object_schema(&[
                        ("auth_type", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "sort_order",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Copy")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "sort_order",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Reorder")),
                (
                    "body",
                    mp::object_schema(&[(
                        "ids",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateEnabled")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("enabled", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateConfig")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "description",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "self_registration_enabled",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        (
                            "extension_config",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdatePublicUiBlock")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "public_ui_block",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Overview { .. } => {
                mp::object_value(&[("variant", serde_json::Value::String("Overview".to_owned()))])
            }
            Self::Create {
                body: _field_body, ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "body",
                    mp::object_value(&[
                        ("auth_type", mp::text(&(_field_body).auth_type)?),
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "description",
                            match (&(_field_body).description).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        (
                            "sort_order",
                            match (&(_field_body).sort_order).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Copy {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Copy".to_owned())),
                ("id", serde_json::Value::String((_field_id).to_string())),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "sort_order",
                            match (&(_field_body).sort_order).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Delete { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                ("id", serde_json::Value::String((_field_id).to_string())),
            ]),
            Self::Reorder {
                body: _field_body, ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Reorder".to_owned())),
                (
                    "body",
                    mp::object_value(&[("ids", {
                        if (&(_field_body).ids).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_body).ids)
                                .iter()
                                .map(|item| Some(serde_json::Value::String((item).to_string())))
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::UpdateEnabled {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateEnabled".to_owned()),
                ),
                ("id", serde_json::Value::String((_field_id).to_string())),
                (
                    "body",
                    mp::object_value(&[(
                        "enabled",
                        serde_json::Value::Bool(*(&(_field_body).enabled)),
                    )]),
                ),
            ]),
            Self::UpdateConfig {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateConfig".to_owned()),
                ),
                ("id", serde_json::Value::String((_field_id).to_string())),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        (
                            "description",
                            match (&(_field_body).description).as_ref() {
                                Some(item) => match (item).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "self_registration_enabled",
                            serde_json::Value::Bool(*(&(_field_body).self_registration_enabled)),
                        ),
                        (
                            "extension_config",
                            match (&(_field_body).extension_config).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::UpdatePublicUiBlock {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdatePublicUiBlock".to_owned()),
                ),
                ("id", serde_json::Value::String((_field_id).to_string())),
                (
                    "body",
                    mp::object_value(&[(
                        "public_ui_block",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_body).public_ui_block).len()),
                        )]),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-auth-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed authentication output is projected immediately into the console response"
)]
pub(crate) enum AuthCenterOutput {
    Overview(auth_center::AuthCenterOverviewResponse),
    Authenticator(auth_center::AuthCenterLoginEntryResponse),
    NoContent,
}

impl InterfaceContract for AuthCenterOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Overview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("default_login_entry_id", mp::text_schema()),
                        (
                            "supported_auth_types",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "login_entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("auth_type",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("enabled",serde_json::json!({"type":"boolean"})), ("is_builtin",serde_json::json!({"type":"boolean"})), ("sort_order",serde_json::json!({"type":"integer"})), ("public_ui_block",mp::object_schema(&[("byte_count",mp::count_schema())])), ("default_public_ui_block",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("interface_path_prefixes",mp::object_schema(&[("item_count",mp::count_schema())])), ("public_variables",serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]})), ("context_variables",mp::object_schema(&[("item_count",mp::count_schema())])), ("config_schema",mp::object_schema(&[("item_count",mp::count_schema())])), ("config_values",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Authenticator")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("is_builtin", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                        (
                            "public_ui_block",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "default_public_ui_block",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "interface_path_prefixes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "public_variables",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "context_variables",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("group",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Configuration"))]), mp::object_schema(&[("variant",mp::tag_schema("Runtime"))])])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("member_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("schema",mp::json_summary_schema())])}),
                        ),
                        (
                            "config_schema",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("r#type",mp::object_schema(&[("byte_count",mp::count_schema())])), ("control",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("read_only",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("required",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("pattern",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "config_values",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Overview(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Overview".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "default_login_entry_id",
                            serde_json::Value::String(
                                (&(_field_0).default_login_entry_id).to_string(),
                            ),
                        ),
                        (
                            "supported_auth_types",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).supported_auth_types).len()),
                            )]),
                        ),
                        ("login_entries", {
                            if (&(_field_0).login_entries).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).login_entries)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                serde_json::Value::String((&(item).id).to_string()),
                                            ),
                                            ("auth_type", mp::text(&(item).auth_type)?),
                                            (
                                                "title",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).title).len()),
                                                )]),
                                            ),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "is_builtin",
                                                serde_json::Value::Bool(*(&(item).is_builtin)),
                                            ),
                                            (
                                                "sort_order",
                                                serde_json::json!(*(&(item).sort_order)),
                                            ),
                                            (
                                                "public_ui_block",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).public_ui_block).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "default_public_ui_block",
                                                match (&(item).default_public_ui_block).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "interface_path_prefixes",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).interface_path_prefixes).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "public_variables",
                                                match (&(item).public_variables).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "item_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "context_variables",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).context_variables).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "config_schema",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).config_schema).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "config_values",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(item).config_values).len()
                                                    ),
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
            Self::Authenticator(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Authenticator".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            serde_json::Value::String((&(_field_0).id).to_string()),
                        ),
                        ("auth_type", mp::text(&(_field_0).auth_type)?),
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).title).len()),
                            )]),
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "is_builtin",
                            serde_json::Value::Bool(*(&(_field_0).is_builtin)),
                        ),
                        ("sort_order", serde_json::json!(*(&(_field_0).sort_order))),
                        (
                            "public_ui_block",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).public_ui_block).len()),
                            )]),
                        ),
                        (
                            "default_public_ui_block",
                            match (&(_field_0).default_public_ui_block).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "interface_path_prefixes",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).interface_path_prefixes).len()),
                            )]),
                        ),
                        (
                            "public_variables",
                            match (&(_field_0).public_variables).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("context_variables", {
                            if (&(_field_0).context_variables).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).context_variables).iter().map(|item| Some(mp::object_value(&[("group",match &(item).group {crate::routes::settings_group::auth_center::AuthCenterContextVariableGroupResponse::Configuration => mp::object_value(&[("variant",serde_json::Value::String("Configuration".to_owned()))]), crate::routes::settings_group::auth_center::AuthCenterContextVariableGroupResponse::Runtime => mp::object_value(&[("variant",serde_json::Value::String("Runtime".to_owned()))])}), ("label",mp::object_value(&[("byte_count",serde_json::json!((&(item).label).len()))])), ("member_path",mp::object_value(&[("byte_count",serde_json::json!((&(item).member_path).len()))])), ("schema",mp::json_summary(&(item).schema))]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("config_schema", {
                            if (&(_field_0).config_schema).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).config_schema)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).key).len()),
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
                                                "r#type",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).r#type).len()),
                                                )]),
                                            ),
                                            (
                                                "control",
                                                match (&(item).control).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "read_only",
                                                match (&(item).read_only).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "required",
                                                match (&(item).required).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "pattern",
                                                match (&(item).pattern).as_ref() {
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
                        (
                            "config_values",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).config_values).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-auth-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct AuthCenterAdapter {
    store: MainDurableStore,
    registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
}

pub(crate) fn auth_center_port(
    store: MainDurableStore,
    registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput>> {
    Arc::new(AuthCenterAdapter {
        store,
        registry,
        bootstrap_workspace_id,
    })
}

impl AuthCenterAdapter {
    async fn locale(
        &self,
        principal: &UserPrincipal,
        hints: &ConsoleLocaleHints,
    ) -> Result<domain::CatalogLocale, ApiError> {
        let preferred = self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        Ok(hints.resolve(preferred))
    }

    async fn login_entry_response(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
        authenticator: domain::LoginEntryRecord,
    ) -> Result<auth_center::AuthCenterLoginEntryResponse, ApiError> {
        let locale = self.locale(principal, &hints).await?;
        let mut response =
            auth_center::to_auth_center_login_entry_response(authenticator, self.registry.as_ref());
        auth_center::localize_login_entry_response_with(
            &self.store,
            self.bootstrap_workspace_id,
            &locale,
            &mut response,
        )
        .await?;
        Ok(response)
    }

    async fn overview_response(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
        overview: control_plane::auth::settings::AuthCenterSettingsOverview,
    ) -> Result<auth_center::AuthCenterOverviewResponse, ApiError> {
        let locale = self.locale(principal, &hints).await?;
        let mut response =
            auth_center::auth_center_overview_response(overview, self.registry.as_ref());
        auth_center::localize_overview_response_with(
            &self.store,
            self.bootstrap_workspace_id,
            &locale,
            &mut response,
        )
        .await?;
        Ok(response)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AuthCenterInput,
    ) -> Result<AuthCenterOutput, ApiError> {
        let actor = principal.actor();
        let service = AuthCenterSettingsService::with_registry(
            self.store.clone(),
            Arc::clone(&self.registry),
        );
        match input {
            AuthCenterInput::Overview { locale } => {
                let overview = service.overview(actor).await?;
                Ok(AuthCenterOutput::Overview(
                    self.overview_response(principal, locale, overview).await?,
                ))
            }
            AuthCenterInput::Create { locale, body } => {
                let record = service
                    .create_login_entry(
                        actor,
                        CreateAuthCenterLoginEntryCommand {
                            auth_type: body.auth_type,
                            title: body.title,
                            description: body.description,
                            enabled: body.enabled,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.login_entry_response(principal, locale, record).await?,
                ))
            }
            AuthCenterInput::Copy { locale, id, body } => {
                let record = service
                    .copy_login_entry(
                        actor,
                        CopyAuthCenterLoginEntryCommand {
                            source_id: id,
                            title: body.title,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.login_entry_response(principal, locale, record).await?,
                ))
            }
            AuthCenterInput::Delete { id } => {
                service.delete_login_entry(actor, id).await?;
                Ok(AuthCenterOutput::NoContent)
            }
            AuthCenterInput::Reorder { locale, body } => {
                let overview = service.reorder_login_entries(actor, &body.ids).await?;
                Ok(AuthCenterOutput::Overview(
                    self.overview_response(principal, locale, overview).await?,
                ))
            }
            AuthCenterInput::UpdateEnabled { locale, id, body } => {
                let record = service
                    .update_login_entry_enabled(
                        actor,
                        UpdateAuthCenterLoginEntryEnabledCommand {
                            login_entry_id: id,
                            enabled: body.enabled,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.login_entry_response(principal, locale, record).await?,
                ))
            }
            AuthCenterInput::UpdateConfig { locale, id, body } => {
                let record = service
                    .update_login_entry(
                        actor,
                        UpdateAuthCenterLoginEntryConfigCommand {
                            login_entry_id: id,
                            title: body.title,
                            enabled: body.enabled,
                            description: body.description,
                            self_registration_enabled: body.self_registration_enabled,
                            extension_config: body.extension_config,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.login_entry_response(principal, locale, record).await?,
                ))
            }
            AuthCenterInput::UpdatePublicUiBlock { locale, id, body } => {
                let record = service
                    .update_login_entry_public_ui_block(
                        actor,
                        UpdateAuthCenterLoginEntryPublicUiBlockCommand {
                            login_entry_id: id,
                            public_ui_block: body.public_ui_block,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.login_entry_response(principal, locale, record).await?,
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput> for AuthCenterAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AuthCenterInput,
    ) -> ConsoleInterfaceFuture<'a, AuthCenterOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.overview.view",
        binding_id: "http.console.auth-center.overview.view.v1",
        method: "GET",
        path: "/api/console/settings/auth-center/overview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.create",
        binding_id: "http.console.auth-center.login_entries.create.v1",
        method: "POST",
        path: "/api/console/settings/auth-center/login-entries",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.order",
        binding_id: "http.console.auth-center.login_entries.order.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/login-entries/order",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.enabled.update",
        binding_id: "http.console.auth-center.login_entries.enabled.update.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/login-entries/:id/enabled",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.copy",
        binding_id: "http.console.auth-center.login_entries.copy.v1",
        method: "POST",
        path: "/api/console/settings/auth-center/login-entries/:id/copy",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.update.config",
        binding_id: "http.console.auth-center.login_entries.update.config.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/login-entries/:id/config",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.update.public-ui-block",
        binding_id: "http.console.auth-center.login_entries.update.public-ui-block.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/login-entries/:id/public-ui-block",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.login_entries.delete",
        binding_id: "http.console.auth-center.login_entries.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/auth-center/login-entries/:id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-auth-center",
        "graph:console-auth-center-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableAuthCenterPort;

#[cfg(test)]
impl ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput> for UnavailableAuthCenterPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AuthCenterInput,
    ) -> ConsoleInterfaceFuture<'a, AuthCenterOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("auth center fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f08c_registry_freezes_auth_center_bindings() {
        let registry = compile_registry(Arc::new(UnavailableAuthCenterPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(
                registry
                    .binding(&BindingId::new(declaration.binding_id).unwrap())
                    .is_some()
            );
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
