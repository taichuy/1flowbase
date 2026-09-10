use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::{
    auth::{
        ApiKeyService, CreateUserApiKeyCommand, ListUserApiKeysCommand, RevokeUserApiKeyCommand,
    },
    ports::{RoleRepository, SessionStore},
    profile::{ProfileService, UpdateMeCommand, UpdateMeMetaCommand},
    session_security::{
        ChangeOwnPasswordCommand, LogoutCurrentSessionCommand, RevokeAllSessionsCommand,
        SessionSecurityService,
    },
    workspace_session::{SwitchActiveRoleCommand, SwitchWorkspaceCommand, WorkspaceSessionService},
};
use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract,
    InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution,
    InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceProtocol,
    InterfaceScope, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
    UserPrincipal,
};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{me, session, user_api_keys};
use crate::{app_state::ApiState, error_response::ApiError};

const OWNER: &str = "api-server.console-identity";
const AUTHENTICATION_ADAPTER: &str = "api-server.console.require-session";
const AUTHENTICATION_ACTIVATION: &str = "api-server.console.require-session.activation.v1";
const AUTHORIZATION_ADAPTER: &str = "api-server.console.compiled-operation";

pub(crate) enum ConsoleIdentityInput {
    GetSession,
    DeleteSession,
    RevokeAllSessions,
    SwitchWorkspace { workspace_id: String },
    SwitchRole { role_code: String },
    GetMe,
    PatchMe(me::PatchMeBody),
    PatchMeMeta(me::PatchMeMetaBody),
    ChangePassword(me::ChangePasswordBody),
    ListUserApiKeys,
    ListUserApiKeyRoleOptions,
    CreateUserApiKey(user_api_keys::CreateUserApiKeyRequest),
    RevokeUserApiKey { api_key_id: Uuid },
}

impl InterfaceContract for ConsoleIdentityInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("GetSession"))]),
            mp::object_schema(&[("variant", mp::tag_schema("DeleteSession"))]),
            mp::object_schema(&[("variant", mp::tag_schema("RevokeAllSessions"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SwitchWorkspace")),
                ("workspace_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SwitchRole")),
                ("role_code", mp::text_schema()),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("GetMe"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PatchMe")),
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
                        (
                            "preferred_locale",
                            mp::union_schema(vec![
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("Value")),
                                    (
                                        "0",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema(&[("variant", mp::tag_schema("Null"))]),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PatchMeMeta")),
                (
                    "0",
                    mp::object_schema(&[("meta", mp::json_summary_schema())]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ChangePassword"))]),
            mp::object_schema(&[("variant", mp::tag_schema("ListUserApiKeys"))]),
            mp::object_schema(&[("variant", mp::tag_schema("ListUserApiKeyRoleOptions"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateUserApiKey")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "role_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "expiration_policy",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RevokeUserApiKey")),
                ("api_key_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::GetSession => mp::object_value(&[(
                "variant",
                serde_json::Value::String("GetSession".to_owned()),
            )]),
            Self::DeleteSession => mp::object_value(&[(
                "variant",
                serde_json::Value::String("DeleteSession".to_owned()),
            )]),
            Self::RevokeAllSessions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("RevokeAllSessions".to_owned()),
            )]),
            Self::SwitchWorkspace {
                workspace_id: _field_workspace_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SwitchWorkspace".to_owned()),
                ),
                ("workspace_id", mp::text(_field_workspace_id)?),
            ]),
            Self::SwitchRole {
                role_code: _field_role_code,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SwitchRole".to_owned()),
                ),
                ("role_code", mp::text(_field_role_code)?),
            ]),
            Self::GetMe => {
                mp::object_value(&[("variant", serde_json::Value::String("GetMe".to_owned()))])
            }
            Self::PatchMe(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("PatchMe".to_owned())),
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
                        (
                            "preferred_locale",
                            match &(_field_0).preferred_locale {
                                crate::routes::identity_group::me::PreferredLocalePatch::Value(
                                    _field_0,
                                ) => mp::object_value(&[
                                    ("variant", serde_json::Value::String("Value".to_owned())),
                                    (
                                        "0",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((_field_0).len()),
                                        )]),
                                    ),
                                ]),
                                crate::routes::identity_group::me::PreferredLocalePatch::Null => {
                                    mp::object_value(&[(
                                        "variant",
                                        serde_json::Value::String("Null".to_owned()),
                                    )])
                                }
                            },
                        ),
                    ]),
                ),
            ]),
            Self::PatchMeMeta(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PatchMeMeta".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("meta", mp::json_summary(&(_field_0).meta))]),
                ),
            ]),
            Self::ChangePassword(_) => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ChangePassword".to_owned()),
            )]),
            Self::ListUserApiKeys => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListUserApiKeys".to_owned()),
            )]),
            Self::ListUserApiKeyRoleOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListUserApiKeyRoleOptions".to_owned()),
            )]),
            Self::CreateUserApiKey(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateUserApiKey".to_owned()),
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
                            "role_code",
                            match (&(_field_0).role_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "expiration_policy",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).expiration_policy).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::RevokeUserApiKey { api_key_id } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RevokeUserApiKey".to_owned()),
                ),
                (
                    "api_key_id",
                    serde_json::Value::String(api_key_id.to_string()),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-identity-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ConsoleIdentityOutput {
    Session(session::SessionResponse),
    Me(me::MeResponse),
    UserApiKeys(user_api_keys::UserApiKeyListResponse),
    UserApiKeyRoleOptions(user_api_keys::UserApiKeyRoleOptionsResponse),
    UserApiKeyCreated(user_api_keys::UserApiKeyResponse),
    UserApiKeyRevoked(user_api_keys::RevokeUserApiKeyResponse),
    NoContent,
}

impl InterfaceContract for ConsoleIdentityOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Session")),
                (
                    "0",
                    mp::object_schema(&[
                        ("actor", mp::json_summary_schema()),
                        (
                            "available_roles",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "active_role_permissions",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Me")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "nickname",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "introduction",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "preferred_locale",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("meta", mp::json_summary_schema()),
                        (
                            "effective_display_role",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "permissions",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UserApiKeys")),
                (
                    "0",
                    mp::object_schema(&[(
                        "items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("key_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("role_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("creator_user_id",mp::text_schema()), ("tenant_id",mp::text_schema()), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("revoked",serde_json::json!({"type":"boolean"})), ("expires_at",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_used_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UserApiKeyRoleOptions")),
                (
                    "0",
                    mp::object_schema(&[(
                        "items",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UserApiKeyCreated")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "key_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "role_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("creator_user_id", mp::text_schema()),
                        ("tenant_id", mp::text_schema()),
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
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
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UserApiKeyRevoked")),
                ("0", mp::object_schema(&[("id", mp::text_schema())])),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Session(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Session".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("actor", mp::json_summary(&(_field_0).actor)),
                        ("available_roles", {
                            if (&(_field_0).available_roles).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).available_roles)
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
                                                "scope_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).scope_kind).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "active_role_permissions",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).active_role_permissions).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Me(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Me".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "nickname",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).nickname).len()),
                            )]),
                        ),
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
                            "preferred_locale",
                            match (&(_field_0).preferred_locale).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("meta", mp::json_summary(&(_field_0).meta)),
                        (
                            "effective_display_role",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).effective_display_role).len()),
                            )]),
                        ),
                        (
                            "permissions",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).permissions).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UserApiKeys(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UserApiKeys".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("items", {
                        if (&(_field_0).items).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).items)
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value(&[
                                        ("id", serde_json::Value::String((&(item).id).to_string())),
                                        (
                                            "name",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).name).len()),
                                            )]),
                                        ),
                                        (
                                            "key_kind",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).key_kind).len()),
                                            )]),
                                        ),
                                        (
                                            "role_code",
                                            match (&(item).role_code).as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "creator_user_id",
                                            serde_json::Value::String(
                                                (&(item).creator_user_id).to_string(),
                                            ),
                                        ),
                                        (
                                            "tenant_id",
                                            serde_json::Value::String(
                                                (&(item).tenant_id).to_string(),
                                            ),
                                        ),
                                        (
                                            "scope_kind",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).scope_kind).len()),
                                            )]),
                                        ),
                                        (
                                            "scope_id",
                                            serde_json::Value::String(
                                                (&(item).scope_id).to_string(),
                                            ),
                                        ),
                                        ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                        ("revoked", serde_json::Value::Bool(*(&(item).revoked))),
                                        (
                                            "expires_at",
                                            match (&(item).expires_at).as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "last_used_at",
                                            match (&(item).last_used_at).as_ref() {
                                                Some(item) => mp::object_value(&[(
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
            Self::UserApiKeyRoleOptions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyRoleOptions".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("items", {
                        if (&(_field_0).items).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).items)
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
                                            "scope_kind",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).scope_kind).len()),
                                            )]),
                                        ),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::UserApiKeyCreated(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyCreated".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            serde_json::Value::String((&(_field_0).id).to_string()),
                        ),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "key_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key_kind).len()),
                            )]),
                        ),
                        (
                            "role_code",
                            match (&(_field_0).role_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "creator_user_id",
                            serde_json::Value::String((&(_field_0).creator_user_id).to_string()),
                        ),
                        (
                            "tenant_id",
                            serde_json::Value::String((&(_field_0).tenant_id).to_string()),
                        ),
                        (
                            "scope_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).scope_kind).len()),
                            )]),
                        ),
                        (
                            "scope_id",
                            serde_json::Value::String((&(_field_0).scope_id).to_string()),
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        ("revoked", serde_json::Value::Bool(*(&(_field_0).revoked))),
                        (
                            "expires_at",
                            match (&(_field_0).expires_at).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_used_at",
                            match (&(_field_0).last_used_at).as_ref() {
                                Some(item) => mp::object_value(&[(
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
            Self::UserApiKeyRevoked(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UserApiKeyRevoked".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "id",
                        serde_json::Value::String((&(_field_0).id).to_string()),
                    )]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-identity-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ConsoleIdentityTargetError(pub(crate) ApiError);

impl InterfaceContract for ConsoleIdentityTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("ConsoleIdentityTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("ConsoleIdentityTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-identity-error";
    const CONTRACT_VERSION: &'static str = "1";
}

type ConsoleIdentityFuture<'a> = Pin<
    Box<dyn Future<Output = Result<ConsoleIdentityOutput, ConsoleIdentityTargetError>> + Send + 'a>,
>;

pub(crate) trait ConsoleIdentityPort: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ConsoleIdentityInput,
    ) -> ConsoleIdentityFuture<'a>;
}

struct ConsoleIdentityAdapter {
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    cookie_name: String,
}

pub(crate) fn console_identity_port(
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    cookie_name: String,
) -> Arc<dyn ConsoleIdentityPort> {
    Arc::new(ConsoleIdentityAdapter {
        store,
        session_store,
        cookie_name,
    })
}

impl ConsoleIdentityAdapter {
    async fn authenticated_session(
        &self,
        principal: &UserPrincipal,
    ) -> Result<domain::SessionRecord, ApiError> {
        let identity = principal.authenticated_session().ok_or(
            control_plane::errors::ControlPlaneError::PermissionDenied("cookie_session_required"),
        )?;
        self.session_store
            .get(identity.expose_to_trusted_handler())
            .await?
            .ok_or_else(|| control_plane::errors::ControlPlaneError::NotAuthenticated.into())
    }

    async fn user(&self, principal: &UserPrincipal) -> Result<domain::UserRecord, ApiError> {
        self.store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or_else(|| control_plane::errors::ControlPlaneError::NotAuthenticated.into())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ConsoleIdentityInput,
    ) -> Result<ConsoleIdentityOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ConsoleIdentityInput::GetSession => {
                let current_session = self.authenticated_session(principal).await?;
                let user = self.user(principal).await?;
                Ok(ConsoleIdentityOutput::Session(
                    session::to_session_response(&user, actor, &current_session, &self.cookie_name),
                ))
            }
            ConsoleIdentityInput::DeleteSession => {
                let current_session = self.authenticated_session(principal).await?;
                SessionSecurityService::new(self.store.clone(), Arc::clone(&self.session_store))
                    .logout_current_session(LogoutCurrentSessionCommand {
                        session_id: current_session.session_id,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::NoContent)
            }
            ConsoleIdentityInput::RevokeAllSessions => {
                let current_session = self.authenticated_session(principal).await?;
                SessionSecurityService::new(self.store.clone(), Arc::clone(&self.session_store))
                    .revoke_all_sessions(RevokeAllSessionsCommand {
                        actor_user_id: actor.user_id,
                        session_id: current_session.session_id,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::NoContent)
            }
            ConsoleIdentityInput::SwitchWorkspace { workspace_id } => {
                let current_session = self.authenticated_session(principal).await?;
                let workspace_id = Uuid::parse_str(&workspace_id).map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("workspace_id")
                })?;
                let result = WorkspaceSessionService::new(
                    self.store.clone(),
                    self.store.clone(),
                    Arc::clone(&self.session_store),
                )
                .switch_workspace(SwitchWorkspaceCommand {
                    actor_user_id: actor.user_id,
                    session_id: current_session.session_id,
                    target_workspace_id: workspace_id,
                })
                .await?;
                let user = self.user(principal).await?;
                Ok(ConsoleIdentityOutput::Session(
                    session::to_session_response(
                        &user,
                        &result.actor,
                        &result.session,
                        &self.cookie_name,
                    ),
                ))
            }
            ConsoleIdentityInput::SwitchRole { role_code } => {
                let current_session = self.authenticated_session(principal).await?;
                let role_code = role_code.trim();
                if role_code.is_empty() {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "role_code",
                    )
                    .into());
                }
                let result = WorkspaceSessionService::new(
                    self.store.clone(),
                    self.store.clone(),
                    Arc::clone(&self.session_store),
                )
                .switch_active_role(SwitchActiveRoleCommand {
                    actor_user_id: actor.user_id,
                    session_id: current_session.session_id,
                    active_role_code: role_code.to_string(),
                })
                .await?;
                let user = self.user(principal).await?;
                Ok(ConsoleIdentityOutput::Session(
                    session::to_session_response(
                        &user,
                        &result.actor,
                        &result.session,
                        &self.cookie_name,
                    ),
                ))
            }
            ConsoleIdentityInput::GetMe => {
                let profile = ProfileService::new(self.store.clone())
                    .get_me(actor.user_id, actor.tenant_id, actor.current_workspace_id)
                    .await?;
                Ok(ConsoleIdentityOutput::Me(me::to_me_response(
                    profile, actor,
                )))
            }
            ConsoleIdentityInput::PatchMe(body) => {
                let profile = ProfileService::new(self.store.clone())
                    .update_me(UpdateMeCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        workspace_id: actor.current_workspace_id,
                        name: body.name,
                        nickname: body.nickname,
                        email: body.email,
                        phone: body.phone,
                        avatar_url: body.avatar_url,
                        introduction: body.introduction,
                        preferred_locale: match body.preferred_locale {
                            me::PreferredLocalePatch::Value(value) => Some(value),
                            me::PreferredLocalePatch::Null => None,
                        },
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::Me(me::to_me_response(
                    profile, actor,
                )))
            }
            ConsoleIdentityInput::PatchMeMeta(body) => {
                let profile = ProfileService::new(self.store.clone())
                    .update_me_meta(UpdateMeMetaCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        workspace_id: actor.current_workspace_id,
                        meta_patch: body.meta,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::Me(me::to_me_response(
                    profile, actor,
                )))
            }
            ConsoleIdentityInput::ChangePassword(body) => {
                let current_session = self.authenticated_session(principal).await?;
                SessionSecurityService::new(self.store.clone(), Arc::clone(&self.session_store))
                    .change_own_password(ChangeOwnPasswordCommand {
                        actor_user_id: actor.user_id,
                        session_id: current_session.session_id,
                        old_password: body.old_password,
                        new_password_hash: me::hash_password(&body.new_password)?,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::NoContent)
            }
            ConsoleIdentityInput::ListUserApiKeys => {
                let items = ApiKeyService::new(self.store.clone())
                    .list_user_api_keys(ListUserApiKeysCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        current_workspace_id: actor.current_workspace_id,
                    })
                    .await?
                    .into_iter()
                    .map(|api_key| user_api_keys::user_api_key_response(api_key, None))
                    .collect();
                Ok(ConsoleIdentityOutput::UserApiKeys(
                    user_api_keys::UserApiKeyListResponse { items },
                ))
            }
            ConsoleIdentityInput::ListUserApiKeyRoleOptions => {
                let user = self.user(principal).await?;
                let workspace_roles = self.store.list_roles(actor.current_workspace_id).await?;
                Ok(ConsoleIdentityOutput::UserApiKeyRoleOptions(
                    user_api_keys::role_options_response(&user, workspace_roles, actor),
                ))
            }
            ConsoleIdentityInput::CreateUserApiKey(payload) => {
                let user = self.user(principal).await?;
                let role_code = payload
                    .role_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(actor.effective_display_role.as_str())
                    .to_string();
                if !user.roles.iter().any(|role| {
                    role.code == role_code
                        && user_api_keys::is_role_bound_to_current_workspace(
                            role,
                            actor.current_workspace_id,
                        )
                }) {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "role_code",
                    )
                    .into());
                }
                let result = ApiKeyService::new(self.store.clone())
                    .create_user_api_key(CreateUserApiKeyCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        current_workspace_id: actor.current_workspace_id,
                        name: payload.name,
                        role_code,
                        expiration_policy: user_api_keys::parse_expiration_policy(
                            &payload.expiration_policy,
                        )?,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::UserApiKeyCreated(
                    user_api_keys::user_api_key_response(result.api_key, Some(result.token)),
                ))
            }
            ConsoleIdentityInput::RevokeUserApiKey { api_key_id } => {
                ApiKeyService::new(self.store.clone())
                    .revoke_user_api_key(RevokeUserApiKeyCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        current_workspace_id: actor.current_workspace_id,
                        api_key_id,
                    })
                    .await?;
                Ok(ConsoleIdentityOutput::UserApiKeyRevoked(
                    user_api_keys::RevokeUserApiKeyResponse { id: api_key_id },
                ))
            }
        }
    }
}

impl ConsoleIdentityPort for ConsoleIdentityAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ConsoleIdentityInput,
    ) -> ConsoleIdentityFuture<'a> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleIdentityTargetError)
        })
    }
}

struct ConsoleIdentityHandler {
    port: Arc<dyn ConsoleIdentityPort>,
}

impl
    InterfaceHandler<
        ConsoleIdentityInput,
        ConsoleIdentityOutput,
        ConsoleIdentityTargetError,
        UserPrincipal,
    > for ConsoleIdentityHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: ConsoleIdentityInput,
    ) -> InterfaceHandlerFuture<ConsoleIdentityOutput, ConsoleIdentityTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("console_identity", error))
        })
    }
}

struct Declaration {
    interface_id: &'static str,
    binding_id: &'static str,
    method: &'static str,
    path: &'static str,
    mutating: bool,
}

const DECLARATIONS: &[Declaration] = &[
    Declaration {
        interface_id: "console.identity.session.get",
        binding_id: "http.console.identity.session.get.v1",
        method: "GET",
        path: "/api/console/session",
        mutating: false,
    },
    Declaration {
        interface_id: "console.identity.session.delete",
        binding_id: "http.console.identity.session.delete.v1",
        method: "DELETE",
        path: "/api/console/session",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.session.revoke-all",
        binding_id: "http.console.identity.session.revoke-all.v1",
        method: "POST",
        path: "/api/console/session/actions/revoke-all",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.session.switch-workspace",
        binding_id: "http.console.identity.session.switch-workspace.v1",
        method: "POST",
        path: "/api/console/session/actions/switch-workspace",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.session.switch-role",
        binding_id: "http.console.identity.session.switch-role.v1",
        method: "POST",
        path: "/api/console/session/actions/switch-role",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.me.get",
        binding_id: "http.console.identity.me.get.v1",
        method: "GET",
        path: "/api/console/me",
        mutating: false,
    },
    Declaration {
        interface_id: "console.identity.me.patch",
        binding_id: "http.console.identity.me.patch.v1",
        method: "PATCH",
        path: "/api/console/me",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.me.meta.patch",
        binding_id: "http.console.identity.me.meta.patch.v1",
        method: "PATCH",
        path: "/api/console/me/meta",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.me.change-password",
        binding_id: "http.console.identity.me.change-password.v1",
        method: "POST",
        path: "/api/console/me/actions/change-password",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.user-api-keys.list",
        binding_id: "http.console.identity.user-api-keys.list.v1",
        method: "GET",
        path: "/api/console/user-api-keys",
        mutating: false,
    },
    Declaration {
        interface_id: "console.identity.user-api-keys.create",
        binding_id: "http.console.identity.user-api-keys.create.v1",
        method: "POST",
        path: "/api/console/user-api-keys",
        mutating: true,
    },
    Declaration {
        interface_id: "console.identity.user-api-keys.role-options",
        binding_id: "http.console.identity.user-api-keys.role-options.v1",
        method: "GET",
        path: "/api/console/user-api-keys/role-options",
        mutating: false,
    },
    Declaration {
        interface_id: "console.identity.user-api-keys.revoke",
        binding_id: "http.console.identity.user-api-keys.revoke.v1",
        method: "POST",
        path: "/api/console/user-api-keys/:api_key_id/revoke",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleIdentityPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new(OWNER).expect("static console identity owner is valid");
    let operations = DECLARATIONS
        .iter()
        .map(|declaration| AuthorizationOperation::new(declaration.interface_id))
        .collect::<Result<Vec<_>, _>>()
        .expect("static console identity operations are valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:console-identity-v1")
            .expect("static console identity graph is valid"),
        operations.clone(),
        [owner.clone()],
    );
    for (declaration, operation) in DECLARATIONS.iter().zip(operations) {
        let interface_id = InterfaceId::new(declaration.interface_id)
            .expect("static console identity interface is valid");
        let identity = InterfaceIdentity::new(
            interface_id.clone(),
            InterfaceVersion::new("1").expect("static version is valid"),
        );
        let contracts = InterfaceContracts::unary(
            contract::<ConsoleIdentityInput>(),
            contract::<ConsoleIdentityOutput>(),
            contract::<ConsoleIdentityTargetError>(),
        );
        let handler = HandlerReference::new(format!("{}.handler", declaration.interface_id))
            .expect("static console identity handler is valid");
        compiler.register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                interface_runtime::PrincipalProfile::User,
                InterfaceAuthenticationPolicy::Authenticated,
                operation,
                InterfaceScope::Workspace,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new(format!("control-plane.{}", declaration.interface_id))
                    .expect("static console identity target is valid"),
            ),
            if declaration.mutating {
                InterfaceAuditPolicy::Mutating
            } else {
                InterfaceAuditPolicy::ReadOnly
            },
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        register_authentication(&mut compiler, &interface_id)?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(declaration.binding_id)
                    .expect("static console identity binding is valid"),
                identity,
                contracts,
                ProtocolProjection::http(
                    RouteIdentity::new(declaration.method, declaration.path)
                        .expect("static console identity route is valid"),
                ),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                    .expect("static console authentication adapter is valid"),
                AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                    .expect("static console authorization adapter is valid"),
                None,
            ),
        )?;
        compiler.bind_handler::<ConsoleIdentityInput, ConsoleIdentityOutput, ConsoleIdentityTargetError, UserPrincipal>(
            &interface_id,
            handler,
            Arc::new(ConsoleIdentityHandler { port: Arc::clone(&port) }),
        )?;
    }
    compiler.compile()
}

fn register_authentication(
    compiler: &mut RegistryCompiler,
    interface_id: &InterfaceId,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    compiler.register_authentication_adapter(
        interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static console authentication plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("console authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static console authentication plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static console authentication adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(AUTHENTICATION_ACTIVATION)
                .expect("static console authentication activation is valid"),
            interface_runtime::PrincipalProfile::User,
        ),
    )
}

pub(crate) async fn invoke(
    state: Arc<ApiState>,
    binding_id: &'static str,
    credential: crate::extension_bus::ConsoleAuthenticationCredential,
    input: ConsoleIdentityInput,
) -> Result<ConsoleIdentityOutput, ApiError> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("interface_operation"),
    )?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "interface_operation",
        ))?;
    let binding_id = BindingId::new(binding_id).expect("static console binding is valid");
    let _activated = snapshot.authentication(&binding_id).cloned().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("authentication_activation"),
    )?;
    let authenticated = boot_snapshot
        .authenticate_invocation::<_, UserPrincipal>(
            Arc::clone(&snapshot),
            &binding_id,
            InterfaceProtocol::Http,
            credential,
        )
        .await
        .map_err(ApiError::from)?;
    let kernel = crate::routes::host_infrastructure::interface_operation::invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    );
    match kernel
        .invoke::<ConsoleIdentityInput, ConsoleIdentityOutput, ConsoleIdentityTargetError>(
            snapshot,
            authenticated.into_envelope(input),
        )
        .await
    {
        Ok(outcome) => {
            let _receipt = outcome.receipt().clone().projected();
            Ok(outcome.into_value())
        }
        Err(failure) => match failure.into_error() {
            interface_runtime::InterfaceInvocationError::TargetFailed(error) => Err(error
                .into_source::<ConsoleIdentityTargetError>()
                .map(|error| error.0)
                .unwrap_or_else(|| {
                    anyhow::anyhow!("console identity target contract mismatch").into()
                })),
            interface_runtime::InterfaceInvocationError::AuthorizationRejected(error) => {
                Err(error.into_source::<ApiError>().unwrap_or_else(|| {
                    anyhow::anyhow!("console identity authorization failed").into()
                }))
            }
            _ => Err(anyhow::anyhow!("console identity invocation failed").into()),
        },
    }
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static console identity contract is valid")
}

#[cfg(test)]
struct UnavailableConsoleIdentityPort;

#[cfg(test)]
impl ConsoleIdentityPort for UnavailableConsoleIdentityPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ConsoleIdentityInput,
    ) -> ConsoleIdentityFuture<'a> {
        Box::pin(async {
            Err(ConsoleIdentityTargetError(
                anyhow::anyhow!("console identity fixture is unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f08a_registry_freezes_every_identity_binding() {
        let registry = compile_registry(Arc::new(UnavailableConsoleIdentityPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .expect("declared Console identity binding must be frozen");
            let route = binding
                .projection()
                .http_route()
                .expect("Console identity binding must project an HTTP route");
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
