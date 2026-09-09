use std::sync::Arc;

use control_plane::{
    network_egress::{CreateNetworkEgressProxyCommand, NetworkEgressProviderService},
    network_egress_pool::{
        AddProviderEgressesToPoolCommand, AddStaticHttpProxyToPoolCommand,
        CreateNetworkEgressPoolCommand, CreateNetworkEgressPoolMemberCommand,
        NetworkEgressPoolService, RecordNetworkEgressPoolMemberProbeCommand,
        UpdateNetworkEgressPoolCommand, UpdateNetworkEgressPoolMemberCommand,
    },
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::pools::*;
use crate::{
    error_response::ApiError,
    network_egress_probe::test_network_egress_connection,
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum NetworkPoolsInput {
    List,
    CreateProxy(CreateNetworkEgressProxyBody),
    TestMember {
        pool_id: String,
        member_id: String,
    },
    Create(CreateNetworkEgressPoolBody),
    Update {
        pool_id: String,
        body: UpdateNetworkEgressPoolBody,
    },
    Delete {
        pool_id: String,
    },
    CreateMember {
        pool_id: String,
        body: CreateNetworkEgressPoolMemberBody,
    },
    AddStatic {
        pool_id: String,
        body: AddStaticHttpProxyToPoolBody,
    },
    AddProvider {
        pool_id: String,
        body: AddProviderEgressesToPoolBody,
    },
    UpdateMember {
        pool_id: String,
        member_id: String,
        body: UpdateNetworkEgressPoolMemberBody,
    },
    DeleteMember {
        pool_id: String,
        member_id: String,
    },
    DeleteMembers {
        pool_id: String,
        body: DeleteNetworkEgressPoolMembersBody,
    },
}

impl InterfaceContract for NetworkPoolsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateProxy")),
                (
                    "0",
                    mp::object_schema(&[
                        ("provider_code", mp::text_schema()),
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
                ("variant", mp::tag_schema("TestMember")),
                ("pool_id", mp::text_schema()),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[(
                        "display_name",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("pool_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "display_name",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("pool_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateMember")),
                ("pool_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("provider_id", mp::text_schema()),
                        (
                            "provider_egress_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sequence", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("AddStatic")),
                ("pool_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "host",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("port", serde_json::json!({"type":"integer"})),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sequence", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("AddProvider")),
                ("pool_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("provider_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sequence", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateMember")),
                ("pool_id", mp::text_schema()),
                ("member_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sequence", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteMember")),
                ("pool_id", mp::text_schema()),
                ("member_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteMembers")),
                ("pool_id", mp::text_schema()),
                (
                    "body",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Selected")),
                            (
                                "member_ids",
                                serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                            ),
                        ]),
                        mp::object_schema(&[("variant", mp::tag_schema("All"))]),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::List => mp::object_value(&[("variant",serde_json::Value::String("List".to_owned()))]), Self::CreateProxy(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("CreateProxy".to_owned())), ("0",mp::object_value(&[("provider_code",mp::text(&(_field_0).provider_code)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).display_name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).description).len()))])), ("config",mp::json_summary(&(_field_0).config))]))]), Self::TestMember {pool_id: _field_pool_id, member_id: _field_member_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("TestMember".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("member_id",mp::text(_field_member_id)?)]), Self::Create(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned())), ("0",mp::object_value(&[("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).display_name).len()))]))]))]), Self::Update {pool_id: _field_pool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("body",mp::object_value(&[("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).display_name).len()))]))]))]), Self::Delete {pool_id: _field_pool_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("pool_id",mp::text(_field_pool_id)?)]), Self::CreateMember {pool_id: _field_pool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("CreateMember".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("body",mp::object_value(&[("provider_id",mp::text(&(_field_body).provider_id)?), ("provider_egress_key",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).provider_egress_key).len()))])), ("enabled",serde_json::Value::Bool(*(&(_field_body).enabled))), ("sequence",serde_json::json!(*(&(_field_body).sequence)))]))]), Self::AddStatic {pool_id: _field_pool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("AddStatic".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("body",mp::object_value(&[("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).display_name).len()))])), ("host",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).host).len()))])), ("port",serde_json::json!(*(&(_field_body).port))), ("enabled",serde_json::Value::Bool(*(&(_field_body).enabled))), ("sequence",serde_json::json!(*(&(_field_body).sequence)))]))]), Self::AddProvider {pool_id: _field_pool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("AddProvider".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("body",mp::object_value(&[("provider_id",mp::text(&(_field_body).provider_id)?), ("enabled",serde_json::Value::Bool(*(&(_field_body).enabled))), ("sequence",serde_json::json!(*(&(_field_body).sequence)))]))]), Self::UpdateMember {pool_id: _field_pool_id, member_id: _field_member_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("UpdateMember".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("member_id",mp::text(_field_member_id)?), ("body",mp::object_value(&[("enabled",serde_json::Value::Bool(*(&(_field_body).enabled))), ("sequence",serde_json::json!(*(&(_field_body).sequence)))]))]), Self::DeleteMember {pool_id: _field_pool_id, member_id: _field_member_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("DeleteMember".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("member_id",mp::text(_field_member_id)?)]), Self::DeleteMembers {pool_id: _field_pool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("DeleteMembers".to_owned())), ("pool_id",mp::text(_field_pool_id)?), ("body",match _field_body {crate::routes::network_center::pools::DeleteNetworkEgressPoolMembersBody::Selected {member_ids: _field_member_ids, .. } => mp::object_value(&[("variant",serde_json::Value::String("Selected".to_owned())), ("member_ids",{ if (_field_member_ids).len() > 32 { return None; } serde_json::Value::Array((_field_member_ids).iter().map(|item| Some(mp::text(item)?)).collect::<Option<Vec<_>>>()?) })]), crate::routes::network_center::pools::DeleteNetworkEgressPoolMembersBody::All => mp::object_value(&[("variant",serde_json::Value::String("All".to_owned()))])})])})
    }

    const CONTRACT_ID: &'static str = "console-network-pools-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum NetworkPoolsOutput {
    Pools(Vec<NetworkEgressPoolResponse>),
    Pool(NetworkEgressPoolResponse),
    Provider(super::NetworkEgressProviderResponse),
    Member(NetworkEgressPoolMemberResponse),
    Members(Vec<NetworkEgressPoolMemberResponse>),
    Deleted,
}

impl InterfaceContract for NetworkPoolsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Pools")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("owner_provider_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("selection_strategy",mp::object_schema(&[("byte_count",mp::count_schema())])), ("members",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("provider_id",mp::text_schema()), ("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("enabled",serde_json::json!({"type":"boolean"})), ("sequence",serde_json::json!({"type":"integer"})), ("health",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("address_summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_http_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_https_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_latency_ms",serde_json::json!({"type":"integer"})), ("probe_exit_ip",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_exit_region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_error_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_probed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Pool")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "owner_provider_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "selection_strategy",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "members",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("provider_id",mp::text_schema()), ("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("enabled",serde_json::json!({"type":"boolean"})), ("sequence",serde_json::json!({"type":"integer"})), ("health",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("address_summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_http_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_https_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_latency_ms",serde_json::json!({"type":"integer"})), ("probe_exit_ip",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_exit_region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_error_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_probed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
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
                ("variant", mp::tag_schema("Member")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("provider_id", mp::text_schema()),
                        (
                            "provider_egress_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sequence", serde_json::json!({"type":"integer"})),
                        (
                            "health",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("provider_code", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "address_summary",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "region",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "probe_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "probe_http_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "probe_https_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("probe_latency_ms", serde_json::json!({"type":"integer"})),
                        (
                            "probe_exit_ip",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "probe_exit_region",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "probe_error_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "last_probed_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Members")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("provider_id",mp::text_schema()), ("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("enabled",serde_json::json!({"type":"boolean"})), ("sequence",serde_json::json!({"type":"integer"})), ("health",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("address_summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_http_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_https_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("probe_latency_ms",serde_json::json!({"type":"integer"})), ("probe_exit_ip",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_exit_region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("probe_error_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("last_probed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Pools(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Pools".to_owned())),
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
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    (
                                        "owner_provider_id",
                                        match (&(item).owner_provider_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "selection_strategy",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).selection_strategy).len()),
                                        )]),
                                    ),
                                    ("members", {
                                        if (&(item).members).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).members)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        ("id", mp::text(&(item).id)?),
                                                        (
                                                            "provider_id",
                                                            mp::text(&(item).provider_id)?,
                                                        ),
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
                                                            "enabled",
                                                            serde_json::Value::Bool(
                                                                *(&(item).enabled),
                                                            ),
                                                        ),
                                                        (
                                                            "sequence",
                                                            serde_json::json!(*(&(item).sequence)),
                                                        ),
                                                        (
                                                            "health",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).health).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "provider_code",
                                                            mp::text(&(item).provider_code)?,
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
                                                            "address_summary",
                                                            match (&(item).address_summary).as_ref()
                                                            {
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
                                                            "probe_status",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).probe_status).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "probe_http_status",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).probe_http_status)
                                                                        .len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "probe_https_status",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).probe_https_status)
                                                                        .len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "probe_latency_ms",
                                                            serde_json::json!(
                                                                *(&(item).probe_latency_ms)
                                                            ),
                                                        ),
                                                        (
                                                            "probe_exit_ip",
                                                            match (&(item).probe_exit_ip).as_ref() {
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
                                                            "probe_exit_region",
                                                            match (&(item).probe_exit_region)
                                                                .as_ref()
                                                            {
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
                                                            "probe_error_code",
                                                            match (&(item).probe_error_code)
                                                                .as_ref()
                                                            {
                                                                Some(item) => mp::text(item)?,
                                                                None => serde_json::Value::Null,
                                                            },
                                                        ),
                                                        (
                                                            "last_probed_at",
                                                            match (&(item).last_probed_at).as_ref()
                                                            {
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
            Self::Pool(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Pool".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        (
                            "owner_provider_id",
                            match (&(_field_0).owner_provider_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "selection_strategy",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).selection_strategy).len()),
                            )]),
                        ),
                        ("members", {
                            if (&(_field_0).members).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).members)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("provider_id", mp::text(&(item).provider_id)?),
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
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            ("sequence", serde_json::json!(*(&(item).sequence))),
                                            (
                                                "health",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).health).len()),
                                                )]),
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
                                                "address_summary",
                                                match (&(item).address_summary).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
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
                                                "probe_status",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).probe_status).len()),
                                                )]),
                                            ),
                                            (
                                                "probe_http_status",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).probe_http_status).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "probe_https_status",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).probe_https_status).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "probe_latency_ms",
                                                serde_json::json!(*(&(item).probe_latency_ms)),
                                            ),
                                            (
                                                "probe_exit_ip",
                                                match (&(item).probe_exit_ip).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "probe_exit_region",
                                                match (&(item).probe_exit_region).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "probe_error_code",
                                                match (&(item).probe_error_code).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "last_probed_at",
                                                match (&(item).last_probed_at).as_ref() {
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
            Self::Member(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Member".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("provider_id", mp::text(&(_field_0).provider_id)?),
                        (
                            "provider_egress_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).provider_egress_key).len()),
                            )]),
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        ("sequence", serde_json::json!(*(&(_field_0).sequence))),
                        (
                            "health",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).health).len()),
                            )]),
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
                            "address_summary",
                            match (&(_field_0).address_summary).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "region",
                            match (&(_field_0).region).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "probe_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).probe_status).len()),
                            )]),
                        ),
                        (
                            "probe_http_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).probe_http_status).len()),
                            )]),
                        ),
                        (
                            "probe_https_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).probe_https_status).len()),
                            )]),
                        ),
                        (
                            "probe_latency_ms",
                            serde_json::json!(*(&(_field_0).probe_latency_ms)),
                        ),
                        (
                            "probe_exit_ip",
                            match (&(_field_0).probe_exit_ip).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "probe_exit_region",
                            match (&(_field_0).probe_exit_region).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "probe_error_code",
                            match (&(_field_0).probe_error_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_probed_at",
                            match (&(_field_0).last_probed_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
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
                                    ("provider_id", mp::text(&(item).provider_id)?),
                                    (
                                        "provider_egress_key",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).provider_egress_key).len()),
                                        )]),
                                    ),
                                    ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                    ("sequence", serde_json::json!(*(&(item).sequence))),
                                    (
                                        "health",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).health).len()),
                                        )]),
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
                                        "address_summary",
                                        match (&(item).address_summary).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
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
                                        "probe_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).probe_status).len()),
                                        )]),
                                    ),
                                    (
                                        "probe_http_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).probe_http_status).len()),
                                        )]),
                                    ),
                                    (
                                        "probe_https_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).probe_https_status).len()),
                                        )]),
                                    ),
                                    (
                                        "probe_latency_ms",
                                        serde_json::json!(*(&(item).probe_latency_ms)),
                                    ),
                                    (
                                        "probe_exit_ip",
                                        match (&(item).probe_exit_ip).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "probe_exit_region",
                                        match (&(item).probe_exit_region).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "probe_error_code",
                                        match (&(item).probe_error_code).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "last_probed_at",
                                        match (&(item).last_probed_at).as_ref() {
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
            Self::Deleted => {
                mp::object_value(&[("variant", serde_json::Value::String("Deleted".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-network-pools-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkPoolsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
}

struct NetworkPoolsAdapter(NetworkPoolsDependencies);

impl NetworkPoolsAdapter {
    fn service(&self) -> crate::app_state::ApiNetworkEgressPoolService {
        NetworkEgressPoolService::with_secret_master_key(
            self.0.store.clone(),
            self.0.secret_key.clone(),
        )
    }

    fn proxy_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
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

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkPoolsInput,
    ) -> Result<NetworkPoolsOutput, ApiError> {
        let user_id = principal.actor().user_id;
        match input {
            NetworkPoolsInput::List => Ok(NetworkPoolsOutput::Pools(
                self.service()
                    .list_global(user_id)
                    .await?
                    .into_iter()
                    .map(response)
                    .collect(),
            )),
            NetworkPoolsInput::CreateProxy(body) => {
                Ok(NetworkPoolsOutput::Provider(super::response(
                    self.proxy_service()
                        .create_proxy(CreateNetworkEgressProxyCommand {
                            actor_user_id: user_id,
                            provider_code: body.provider_code,
                            display_name: body.display_name,
                            description: body.description,
                            config: body.config,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::TestMember { pool_id, member_id } => {
                let pool_id = parse_uuid(&pool_id, "pool_id")?;
                let member_id = parse_uuid(&member_id, "member_id")?;
                let member = self.service().member(pool_id, member_id).await?;
                let probe = test_network_egress_connection(
                    self.0.store.clone(),
                    self.0.provider_runtime.clone(),
                    self.0.secret_key.clone(),
                    self.0.api_node_id.clone(),
                    control_plane::network_egress_pool::NetworkEgressPoolSelection {
                        pool_id,
                        member_id,
                        provider_id: member.provider_id,
                        provider_egress_key: member.provider_egress_key,
                    },
                )
                .await;
                let member = self
                    .service()
                    .record_probe(RecordNetworkEgressPoolMemberProbeCommand {
                        actor_user_id: user_id,
                        pool_id,
                        member_id,
                        status: probe.status,
                        http_status: probe.http_status,
                        https_status: probe.https_status,
                        latency_ms: probe.latency_ms,
                        exit_ip: probe.exit_ip,
                        exit_region: probe.exit_region,
                        error_code: probe.error_code,
                    })
                    .await?;
                Ok(NetworkPoolsOutput::Member(member_response(member)))
            }
            NetworkPoolsInput::Create(body) => Ok(NetworkPoolsOutput::Pool(response(
                self.service()
                    .create(CreateNetworkEgressPoolCommand {
                        actor_user_id: user_id,
                        display_name: body.display_name,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::Update { pool_id, body } => Ok(NetworkPoolsOutput::Pool(response(
                self.service()
                    .update(UpdateNetworkEgressPoolCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        display_name: body.display_name,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::Delete { pool_id } => {
                self.service()
                    .delete(user_id, parse_uuid(&pool_id, "pool_id")?)
                    .await?;
                Ok(NetworkPoolsOutput::Deleted)
            }
            NetworkPoolsInput::CreateMember { pool_id, body } => {
                Ok(NetworkPoolsOutput::Member(member_response(
                    self.service()
                        .add_member(CreateNetworkEgressPoolMemberCommand {
                            actor_user_id: user_id,
                            pool_id: parse_uuid(&pool_id, "pool_id")?,
                            provider_id: parse_uuid(&body.provider_id, "provider_id")?,
                            provider_egress_key: body.provider_egress_key,
                            enabled: body.enabled,
                            sequence: body.sequence,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::AddStatic { pool_id, body } => {
                Ok(NetworkPoolsOutput::Member(member_response(
                    self.service()
                        .add_static_http_proxy(AddStaticHttpProxyToPoolCommand {
                            actor_user_id: user_id,
                            pool_id: parse_uuid(&pool_id, "pool_id")?,
                            display_name: body.display_name,
                            host: body.host,
                            port: body.port,
                            username: body.username,
                            password: body.password,
                            enabled: body.enabled,
                            sequence: body.sequence,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::AddProvider { pool_id, body } => Ok(NetworkPoolsOutput::Members(
                self.service()
                    .add_provider_egresses(AddProviderEgressesToPoolCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        provider_id: parse_uuid(&body.provider_id, "provider_id")?,
                        enabled: body.enabled,
                        sequence: body.sequence,
                    })
                    .await?
                    .into_iter()
                    .map(member_response)
                    .collect(),
            )),
            NetworkPoolsInput::UpdateMember {
                pool_id,
                member_id,
                body,
            } => Ok(NetworkPoolsOutput::Member(member_response(
                self.service()
                    .update_member(UpdateNetworkEgressPoolMemberCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        member_id: parse_uuid(&member_id, "member_id")?,
                        enabled: body.enabled,
                        sequence: body.sequence,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::DeleteMember { pool_id, member_id } => {
                self.service()
                    .delete_member(
                        user_id,
                        parse_uuid(&pool_id, "pool_id")?,
                        parse_uuid(&member_id, "member_id")?,
                    )
                    .await?;
                Ok(NetworkPoolsOutput::Deleted)
            }
            NetworkPoolsInput::DeleteMembers { pool_id, body } => {
                let pool_id = parse_uuid(&pool_id, "pool_id")?;
                match body {
                    DeleteNetworkEgressPoolMembersBody::Selected { member_ids } => {
                        let member_ids = member_ids
                            .iter()
                            .map(|member_id| parse_uuid(member_id, "member_ids"))
                            .collect::<Result<Vec<_>, _>>()?;
                        self.service()
                            .delete_members(user_id, pool_id, member_ids)
                            .await?;
                    }
                    DeleteNetworkEgressPoolMembersBody::All => {
                        self.service().delete_all_members(user_id, pool_id).await?;
                    }
                }
                Ok(NetworkPoolsOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkPoolsInput, NetworkPoolsOutput> for NetworkPoolsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkPoolsInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkPoolsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.list",
        binding_id: "http.console.network-egress-pools.list.v1",
        method: "GET",
        path: "/api/console/network-center/pools",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_proxies.create",
        binding_id: "http.console.network-egress-proxies.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools/proxies",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.test_connection",
        binding_id: "http.console.network-egress-pool-members.test-connection.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id/test-connection",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.create",
        binding_id: "http.console.network-egress-pools.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.update",
        binding_id: "http.console.network-egress-pools.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/pools/:pool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.delete",
        binding_id: "http.console.network-egress-pools.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/pools/:pool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.create",
        binding_id: "http.console.network-egress-pool-members.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.create_static_http",
        binding_id: "http.console.network-egress-pool-members.create-static-http.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/static-http",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.add_provider_egresses",
        binding_id: "http.console.network-egress-pool-members.add-provider-egresses.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/provider",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.update",
        binding_id: "http.console.network-egress-pool-members.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.delete",
        binding_id: "http.console.network-egress-pool-members.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.batch_delete",
        binding_id: "http.console.network-egress-pool-members.batch-delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/pools/:pool_id/members/batch",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: NetworkPoolsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-pools",
        "graph:console-network-pools-v1",
        DECLARATIONS,
        Arc::new(NetworkPoolsAdapter(dependencies)),
    )
}
