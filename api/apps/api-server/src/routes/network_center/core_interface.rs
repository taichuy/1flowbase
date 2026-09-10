use std::sync::Arc;

use control_plane::{
    network_egress::{
        CreateNetworkEgressProviderCommand, NetworkEgressProviderService,
        UpdateNetworkEgressProviderLifecycleCommand,
    },
    network_egress_route::{
        CreateNetworkEgressRouteCommand, NetworkEgressRouteService, UpdateNetworkEgressRouteCommand,
    },
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::{
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum NetworkCenterInput {
    ListProviders,
    ListProviderTypes,
    CreateProvider(CreateNetworkEgressProviderBody),
    UpdateProviderLifecycle {
        id: String,
        body: UpdateNetworkEgressProviderLifecycleBody,
    },
    SyncProvider {
        id: String,
    },
    ListRoutes,
    CreateRoute(CreateNetworkEgressRouteBody),
    UpdateRoute {
        route_id: String,
        body: UpdateNetworkEgressRouteBody,
    },
    DeleteRoute {
        route_id: String,
    },
}

impl InterfaceContract for NetworkCenterInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ListProviders"))]),
            mp::object_schema(&[("variant", mp::tag_schema("ListProviderTypes"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateProvider")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("config", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateProviderLifecycle")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("lifecycle", mp::text_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SyncProvider")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListRoutes"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateRoute")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "consumer_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "consumer_reference",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "pool_member_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateRoute")),
                ("route_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "pool_member_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteRoute")),
                ("route_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListProviders => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListProviders".to_owned()),
            )]),
            Self::ListProviderTypes => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListProviderTypes".to_owned()),
            )]),
            Self::CreateProvider(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateProvider".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("installation_id", mp::text(&(_field_0).installation_id)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        (
                            "description",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).description).len()),
                            )]),
                        ),
                        ("config", mp::json_summary(&(_field_0).config)),
                    ]),
                ),
            ]),
            Self::UpdateProviderLifecycle {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateProviderLifecycle".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[("lifecycle", mp::text(&(_field_body).lifecycle)?)]),
                ),
            ]),
            Self::SyncProvider { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SyncProvider".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::ListRoutes => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListRoutes".to_owned()),
            )]),
            Self::CreateRoute(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateRoute".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "consumer_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).consumer_kind).len()),
                            )]),
                        ),
                        (
                            "consumer_reference",
                            match (&(_field_0).consumer_reference).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("pool_member_ids", {
                            if (&(_field_0).pool_member_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).pool_member_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                    ]),
                ),
            ]),
            Self::UpdateRoute {
                route_id: _field_route_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateRoute".to_owned()),
                ),
                ("route_id", mp::text(_field_route_id)?),
                (
                    "body",
                    mp::object_value(&[
                        ("pool_member_ids", {
                            if (&(_field_body).pool_member_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).pool_member_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                    ]),
                ),
            ]),
            Self::DeleteRoute {
                route_id: _field_route_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteRoute".to_owned()),
                ),
                ("route_id", mp::text(_field_route_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-network-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum NetworkCenterOutput {
    Providers(Vec<NetworkEgressProviderResponse>),
    ProviderTypes(Vec<NetworkEgressProviderTypeResponse>),
    Provider(NetworkEgressProviderResponse),
    Routes(Vec<NetworkEgressRouteResponse>),
    Route(NetworkEgressRouteResponse),
    Deleted,
}

impl InterfaceContract for NetworkCenterOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Providers")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("extension_category",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("extension_organization",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("extension_artifact_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("lifecycle",mp::text_schema()), ("health_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("last_sync_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("last_synced_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("egresses",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tags",mp::object_schema(&[("item_count",mp::count_schema())])), ("availability",mp::object_schema(&[("byte_count",mp::count_schema())])), ("synced_at",mp::object_schema(&[("byte_count",mp::count_schema())]))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ProviderTypes")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("form_schema",mp::json_summary_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Provider")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "extension_category",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "extension_organization",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "extension_artifact_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("provider_code", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("lifecycle", mp::text_schema()),
                        (
                            "health_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_sync_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "last_synced_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "egresses",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tags",mp::object_schema(&[("item_count",mp::count_schema())])), ("availability",mp::object_schema(&[("byte_count",mp::count_schema())])), ("synced_at",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Routes")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("consumer_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("consumer_reference",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("pool_member_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()})), ("enabled",serde_json::json!({"type":"boolean"})), ("failure_policy",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Route")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "consumer_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "consumer_reference",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "pool_member_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "failure_policy",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Providers(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Providers".to_owned())),
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
                                        "extension_category",
                                        match (&(item).extension_category).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "extension_organization",
                                        match (&(item).extension_organization).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "extension_artifact_id",
                                        match (&(item).extension_artifact_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("provider_code", mp::text(&(item).provider_code)?),
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    (
                                        "description",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).description).len()),
                                        )]),
                                    ),
                                    ("lifecycle", mp::text(&(item).lifecycle)?),
                                    (
                                        "health_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).health_status).len()),
                                        )]),
                                    ),
                                    (
                                        "last_sync_error",
                                        match (&(item).last_sync_error).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "last_synced_at",
                                        match (&(item).last_synced_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("egresses", {
                                        if (&(item).egresses).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).egresses)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        (
                                                            "provider_egress_key",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).provider_egress_key)
                                                                        .len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "display_name",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).display_name).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "region",
                                                            match (&(item).region).as_ref() {
                                                                Some(item) => {
                                                                    mp::object_value(&[(
                                                                        "byte_count",
                                                                        serde_json::json!(
                                                                            (item).len()
                                                                        ),
                                                                    )])
                                                                }
                                                                None => serde_json::Value::Null,
                                                            },
                                                        ),
                                                        (
                                                            "tags",
                                                            mp::object_value(&[(
                                                                "item_count",
                                                                serde_json::json!(
                                                                    (&(item).tags).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "availability",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).availability).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "synced_at",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).synced_at).len()
                                                                ),
                                                            )]),
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::ProviderTypes(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ProviderTypes".to_owned()),
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
                                    (
                                        "installation_id",
                                        match (&(item).installation_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("provider_code", mp::text(&(item).provider_code)?),
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    ("form_schema", mp::json_summary(&(item).form_schema)),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Provider(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Provider".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "extension_category",
                            match (&(_field_0).extension_category).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "extension_organization",
                            match (&(_field_0).extension_organization).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "extension_artifact_id",
                            match (&(_field_0).extension_artifact_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        (
                            "description",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).description).len()),
                            )]),
                        ),
                        ("lifecycle", mp::text(&(_field_0).lifecycle)?),
                        (
                            "health_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).health_status).len()),
                            )]),
                        ),
                        (
                            "last_sync_error",
                            match (&(_field_0).last_sync_error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_synced_at",
                            match (&(_field_0).last_synced_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("egresses", {
                            if (&(_field_0).egresses).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).egresses)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "provider_egress_key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).provider_egress_key).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "display_name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).display_name).len()),
                                                )]),
                                            ),
                                            (
                                                "region",
                                                match (&(item).region).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "tags",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).tags).len()),
                                                )]),
                                            ),
                                            (
                                                "availability",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).availability).len()),
                                                )]),
                                            ),
                                            (
                                                "synced_at",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).synced_at).len()),
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
            Self::Routes(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Routes".to_owned())),
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
                                        "consumer_kind",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).consumer_kind).len()),
                                        )]),
                                    ),
                                    (
                                        "consumer_reference",
                                        match (&(item).consumer_reference).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("pool_member_ids", {
                                        if (&(item).pool_member_ids).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).pool_member_ids)
                                                .iter()
                                                .map(|item| Some(mp::text(item)?))
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                    (
                                        "failure_policy",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).failure_policy).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Route(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Route".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "consumer_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).consumer_kind).len()),
                            )]),
                        ),
                        (
                            "consumer_reference",
                            match (&(_field_0).consumer_reference).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("pool_member_ids", {
                            if (&(_field_0).pool_member_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).pool_member_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "failure_policy",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).failure_policy).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Deleted => {
                mp::object_value(&[("variant", serde_json::Value::String("Deleted".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-network-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkCenterDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
}

struct NetworkCenterAdapter(NetworkCenterDependencies);

impl NetworkCenterAdapter {
    fn provider_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
        NetworkEgressProviderService::new(
            self.0.store.clone(),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            ProviderRegistryNetworkEgressSecretResolver::new(
                self.0.store.clone(),
                self.0.secret_key.clone(),
            ),
            self.0.secret_key.clone(),
            self.0.api_node_id.clone(),
        )
    }

    fn route_service(&self) -> crate::app_state::ApiNetworkEgressRouteService {
        NetworkEgressRouteService::new(self.0.store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkCenterInput,
    ) -> Result<NetworkCenterOutput, ApiError> {
        let actor = principal.actor();
        match input {
            NetworkCenterInput::ListProviders => Ok(NetworkCenterOutput::Providers(
                self.provider_service()
                    .list()
                    .await?
                    .into_iter()
                    .map(response)
                    .collect(),
            )),
            NetworkCenterInput::ListProviderTypes => Ok(NetworkCenterOutput::ProviderTypes(
                self.provider_service()
                    .list_types()
                    .await?
                    .into_iter()
                    .map(type_response)
                    .collect(),
            )),
            NetworkCenterInput::CreateProvider(body) => {
                Ok(NetworkCenterOutput::Provider(response(
                    self.provider_service()
                        .create(CreateNetworkEgressProviderCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&body.installation_id, "installation_id")?,
                            display_name: body.display_name,
                            description: body.description,
                            secret_json: body.config,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::UpdateProviderLifecycle { id, body } => {
                Ok(NetworkCenterOutput::Provider(response(
                    self.provider_service()
                        .update_lifecycle(UpdateNetworkEgressProviderLifecycleCommand {
                            actor_user_id: actor.user_id,
                            provider_id: parse_uuid(&id, "provider_id")?,
                            lifecycle: lifecycle(&body.lifecycle)?,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::SyncProvider { id } => Ok(NetworkCenterOutput::Provider(response(
                self.provider_service()
                    .sync(actor.user_id, parse_uuid(&id, "provider_id")?)
                    .await?,
            ))),
            NetworkCenterInput::ListRoutes => Ok(NetworkCenterOutput::Routes(
                self.route_service()
                    .list(actor.current_workspace_id)
                    .await?
                    .into_iter()
                    .map(route_response)
                    .collect(),
            )),
            NetworkCenterInput::CreateRoute(body) => {
                Ok(NetworkCenterOutput::Route(route_response(
                    self.route_service()
                        .create(CreateNetworkEgressRouteCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            selector: consumer_selector(
                                body.consumer_kind,
                                body.consumer_reference,
                            )?,
                            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
                            enabled: body.enabled,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::UpdateRoute { route_id, body } => {
                Ok(NetworkCenterOutput::Route(route_response(
                    self.route_service()
                        .update(UpdateNetworkEgressRouteCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            route_id: parse_uuid(&route_id, "route_id")?,
                            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
                            enabled: body.enabled,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::DeleteRoute { route_id } => {
                self.route_service()
                    .delete(
                        actor.user_id,
                        actor.current_workspace_id,
                        parse_uuid(&route_id, "route_id")?,
                    )
                    .await?;
                Ok(NetworkCenterOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkCenterInput, NetworkCenterOutput> for NetworkCenterAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkCenterInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkCenterOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.list",
        binding_id: "http.console.network-egress-providers.list.v1",
        method: "GET",
        path: "/api/console/settings/network-center/providers",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_proxy_types.list",
        binding_id: "http.console.network-egress-provider-types.list.v1",
        method: "GET",
        path: "/api/console/settings/network-center/providers/types",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.create",
        binding_id: "http.console.network-egress-providers.create.v1",
        method: "POST",
        path: "/api/console/settings/network-center/providers",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.lifecycle.update",
        binding_id: "http.console.network-egress-providers.lifecycle.update.v1",
        method: "PATCH",
        path: "/api/console/settings/network-center/providers/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.sync",
        binding_id: "http.console.network-egress-providers.sync.v1",
        method: "POST",
        path: "/api/console/settings/network-center/providers/:id/sync",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.list",
        binding_id: "http.console.network-egress-routes.list.v1",
        method: "GET",
        path: "/api/console/network-center/routes",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.create",
        binding_id: "http.console.network-egress-routes.create.v1",
        method: "POST",
        path: "/api/console/network-center/routes",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.update",
        binding_id: "http.console.network-egress-routes.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/routes/:route_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.delete",
        binding_id: "http.console.network-egress-routes.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/routes/:route_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: NetworkCenterDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-center",
        "graph:console-network-center-v1",
        DECLARATIONS,
        Arc::new(NetworkCenterAdapter(dependencies)),
    )
}
