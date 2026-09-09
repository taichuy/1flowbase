use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum McpCatalogInput {
    Catalog,
    Interfaces(McpInterfaceCatalogQuery),
    List(McpListQuery),
    Export,
}

impl InterfaceContract for McpCatalogInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Catalog"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Interfaces")),
                (
                    "0",
                    mp::object_schema(&[(
                        "bindable_only",
                        serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "instance_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "path",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "keywords",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "depth",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "path_regex",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Export"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog => {
                mp::object_value(&[("variant", serde_json::Value::String("Catalog".to_owned()))])
            }
            Self::Interfaces(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Interfaces".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "bindable_only",
                        match (&(_field_0).bindable_only).as_ref() {
                            Some(item) => serde_json::Value::Bool(*(item)),
                            None => serde_json::Value::Null,
                        },
                    )]),
                ),
            ]),
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "instance_id",
                            match (&(_field_0).instance_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "path",
                            match (&(_field_0).path).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "keywords",
                            match (&(_field_0).keywords).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "depth",
                            match (&(_field_0).depth).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "path_regex",
                            match (&(_field_0).path_regex).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_0).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Export => {
                mp::object_value(&[("variant", serde_json::Value::String("Export".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpCatalogOutput {
    Catalog(McpCatalogResponse),
    Interfaces(Vec<McpInterfaceCatalogEntryResponse>),
    List(Vec<McpListItemSummaryResponse>),
    Export(McpExportPackageResponse),
}

impl InterfaceContract for McpCatalogOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "instances",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("instance_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("default_entry_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("webmcp_exposure",mp::object_schema(&[("byte_count",mp::count_schema())])), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema()), ("llm_tool_registration",mp::object_schema(&[("prefix",mp::object_schema(&[("byte_count",mp::count_schema())])), ("tools",mp::object_schema(&[("item_count",mp::count_schema())]))]))])}),
                        ),
                        (
                            "groups",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("enabled",serde_json::json!({"type":"boolean"})), ("sort_order",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "tools",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("tool_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("short_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("full_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("execution_target",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("InterfaceWrapper")), ("interface_id",mp::text_schema())]), mp::object_schema(&[("variant",mp::tag_schema("McpProxy")), ("upstream_connection_id",mp::text_schema()), ("remote_tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_schema_hash",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantClient")), ("capability_code",mp::text_schema())])])), ("operation",mp::text_schema()), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema()), ("input_mapping",mp::json_summary_schema()), ("output_mapping",mp::json_summary_schema()), ("permission_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("risk_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("des_id",mp::text_schema()), ("des_id_required",serde_json::json!({"type":"boolean"})), ("status",mp::text_schema()), ("availability_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Available"))]), mp::object_schema(&[("variant",mp::tag_schema("InterfaceMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamDisabled"))]), mp::object_schema(&[("variant",mp::tag_schema("CredentialsMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamToolMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("MappingInvalid"))])])), ("availability_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("revision",serde_json::json!({"type":"integer"})), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "bindings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("tool_record_id",mp::text_schema()), ("group_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("tool_id",mp::text_schema()), ("display_alias",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("visible",serde_json::json!({"type":"boolean"})), ("sort_order",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "discovery_policies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("instance_id",mp::text_schema()), ("list_default_limit",serde_json::json!({"type":"integer"})), ("list_max_depth",serde_json::json!({"type":"integer"})), ("list_regex_enabled",serde_json::json!({"type":"boolean"})), ("list_regex_max_length",serde_json::json!({"type":"integer"})), ("list_return_fields",mp::json_summary_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Interfaces")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("interface_id",mp::text_schema()), ("method",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("short_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("parameter_descriptors",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("field_type",mp::text_schema()), ("parameter_type",mp::text_schema()), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("required",serde_json::json!({"type":"boolean"})), ("schema",mp::json_summary_schema())])})), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema()), ("permission_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("security",mp::json_summary_schema()), ("risk_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bindable",serde_json::json!({"type":"boolean"})), ("disabled_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("item_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("path",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("name",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("children_count",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("risk_level",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Export")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "instances",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("instance_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("default_entry_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("webmcp_exposure",mp::object_schema(&[("byte_count",mp::count_schema())])), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema()), ("llm_tool_registration",mp::object_schema(&[("prefix",mp::object_schema(&[("byte_count",mp::count_schema())])), ("tools",mp::object_schema(&[("item_count",mp::count_schema())]))]))])}),
                        ),
                        (
                            "groups",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("enabled",serde_json::json!({"type":"boolean"})), ("sort_order",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "tools",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("tool_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("short_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("full_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("execution_target",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("InterfaceWrapper")), ("interface_id",mp::text_schema())]), mp::object_schema(&[("variant",mp::tag_schema("McpProxy")), ("upstream_connection_id",mp::text_schema()), ("remote_tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_schema_hash",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantClient")), ("capability_code",mp::text_schema())])])), ("operation",mp::text_schema()), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema()), ("input_mapping",mp::json_summary_schema()), ("output_mapping",mp::json_summary_schema()), ("permission_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("risk_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("des_id",mp::text_schema()), ("des_id_required",serde_json::json!({"type":"boolean"})), ("status",mp::text_schema()), ("availability_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Available"))]), mp::object_schema(&[("variant",mp::tag_schema("InterfaceMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamDisabled"))]), mp::object_schema(&[("variant",mp::tag_schema("CredentialsMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamToolMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("MappingInvalid"))])])), ("availability_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("revision",serde_json::json!({"type":"integer"})), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "bindings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("tool_record_id",mp::text_schema()), ("group_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("tool_id",mp::text_schema()), ("display_alias",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("visible",serde_json::json!({"type":"boolean"})), ("sort_order",serde_json::json!({"type":"integer"}))])}),
                        ),
                        (
                            "discovery_policies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("instance_record_id",mp::text_schema()), ("instance_id",mp::text_schema()), ("list_default_limit",serde_json::json!({"type":"integer"})), ("list_max_depth",serde_json::json!({"type":"integer"})), ("list_regex_enabled",serde_json::json!({"type":"boolean"})), ("list_regex_max_length",serde_json::json!({"type":"integer"})), ("list_return_fields",mp::json_summary_schema())])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("instances", {
                            if (&(_field_0).instances).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).instances)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("workspace_id", mp::text(&(item).workspace_id)?),
                                            ("instance_id", mp::text(&(item).instance_id)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                            (
                                                "description_short",
                                                match (&(item).description_short).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "default_entry_path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).default_entry_path).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "webmcp_exposure",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).webmcp_exposure).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "managed_by",
                                                match (&(item).managed_by).as_ref() {
                                                    Some(item) => mp::object_value(&[
                                                        (
                                                            "organization",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).organization).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        ("bundle_id", mp::text(&(item).bundle_id)?),
                                                        (
                                                            "bundle_version",
                                                            mp::text(&(item).bundle_version)?,
                                                        ),
                                                    ]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "created_by",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).created_by).len()),
                                                )]),
                                            ),
                                            (
                                                "updated_by",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).updated_by).len()),
                                                )]),
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                            ("updated_at", mp::text(&(item).updated_at)?),
                                            (
                                                "llm_tool_registration",
                                                mp::object_value(&[
                                                    (
                                                        "prefix",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!(
                                                                (&(&(item).llm_tool_registration)
                                                                    .prefix)
                                                                    .len()
                                                            ),
                                                        )]),
                                                    ),
                                                    (
                                                        "tools",
                                                        mp::object_value(&[(
                                                            "item_count",
                                                            serde_json::json!(
                                                                (&(&(item).llm_tool_registration)
                                                                    .tools)
                                                                    .len()
                                                            ),
                                                        )]),
                                                    ),
                                                ]),
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
                            serde_json::Value::Array(
                                (&(_field_0).groups)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            (
                                                "path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).path).len()),
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
                                                "description_short",
                                                match (&(item).description_short).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "sort_order",
                                                serde_json::json!(*(&(item).sort_order)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("tools", {
                            if (&(_field_0).tools).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).tools).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("tool_id",mp::text(&(item).tool_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).short_description).len()))])), ("full_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).full_description).len()))])), ("execution_target",match &(item).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("operation",mp::text(&(item).operation)?), ("parameter_schema",mp::json_summary(&(item).parameter_schema)), ("result_schema",mp::json_summary(&(item).result_schema)), ("input_mapping",mp::json_summary(&(item).input_mapping)), ("output_mapping",mp::json_summary(&(item).output_mapping)), ("permission_code",match (&(item).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).risk_level).len()))])), ("des_id",mp::text(&(item).des_id)?), ("des_id_required",serde_json::Value::Bool(*(&(item).des_id_required))), ("status",mp::text(&(item).status)?), ("availability_status",match &(item).availability_status {crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::InterfaceMissing => mp::object_value(&[("variant",serde_json::Value::String("InterfaceMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamDisabled => mp::object_value(&[("variant",serde_json::Value::String("UpstreamDisabled".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::CredentialsMissing => mp::object_value(&[("variant",serde_json::Value::String("CredentialsMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamToolMissing => mp::object_value(&[("variant",serde_json::Value::String("UpstreamToolMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::MappingInvalid => mp::object_value(&[("variant",serde_json::Value::String("MappingInvalid".to_owned()))])}), ("availability_reason",match (&(item).availability_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("revision",serde_json::json!(*(&(item).revision))), ("managed_by",match (&(item).managed_by).as_ref() { Some(item) => mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("bundle_version",mp::text(&(item).bundle_version)?)]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("bindings", {
                            if (&(_field_0).bindings).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).bindings)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            ("tool_record_id", mp::text(&(item).tool_record_id)?),
                                            (
                                                "group_path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group_path).len()),
                                                )]),
                                            ),
                                            ("tool_id", mp::text(&(item).tool_id)?),
                                            (
                                                "display_alias",
                                                match (&(item).display_alias).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "visible",
                                                serde_json::Value::Bool(*(&(item).visible)),
                                            ),
                                            (
                                                "sort_order",
                                                serde_json::json!(*(&(item).sort_order)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("discovery_policies", {
                            if (&(_field_0).discovery_policies).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).discovery_policies)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("workspace_id", mp::text(&(item).workspace_id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            ("instance_id", mp::text(&(item).instance_id)?),
                                            (
                                                "list_default_limit",
                                                serde_json::json!(*(&(item).list_default_limit)),
                                            ),
                                            (
                                                "list_max_depth",
                                                serde_json::json!(*(&(item).list_max_depth)),
                                            ),
                                            (
                                                "list_regex_enabled",
                                                serde_json::Value::Bool(
                                                    *(&(item).list_regex_enabled),
                                                ),
                                            ),
                                            (
                                                "list_regex_max_length",
                                                serde_json::json!(*(&(item).list_regex_max_length)),
                                            ),
                                            (
                                                "list_return_fields",
                                                mp::json_summary(&(item).list_return_fields),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::Interfaces(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Interfaces".to_owned()),
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
                                    ("interface_id", mp::text(&(item).interface_id)?),
                                    ("method", mp::text(&(item).method)?),
                                    (
                                        "path",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).path).len()),
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
                                        "short_description",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).short_description).len()),
                                        )]),
                                    ),
                                    ("parameter_descriptors", {
                                        if (&(item).parameter_descriptors).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).parameter_descriptors)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        (
                                                            "name",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).name).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "field_type",
                                                            mp::text(&(item).field_type)?,
                                                        ),
                                                        (
                                                            "parameter_type",
                                                            mp::text(&(item).parameter_type)?,
                                                        ),
                                                        (
                                                            "description",
                                                            match (&(item).description).as_ref() {
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
                                                            "required",
                                                            serde_json::Value::Bool(
                                                                *(&(item).required),
                                                            ),
                                                        ),
                                                        (
                                                            "schema",
                                                            mp::json_summary(&(item).schema),
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    (
                                        "parameter_schema",
                                        mp::json_summary(&(item).parameter_schema),
                                    ),
                                    ("result_schema", mp::json_summary(&(item).result_schema)),
                                    (
                                        "permission_code",
                                        match (&(item).permission_code).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("security", mp::json_summary(&(item).security)),
                                    (
                                        "risk_level",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).risk_level).len()),
                                        )]),
                                    ),
                                    ("bindable", serde_json::Value::Bool(*(&(item).bindable))),
                                    (
                                        "disabled_reason",
                                        match (&(item).disabled_reason).as_ref() {
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
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
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
                                        "id",
                                        match (&(item).id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "item_kind",
                                        match (&(item).item_kind).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "path",
                                        match (&(item).path).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "name",
                                        match (&(item).name).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "description_short",
                                        match (&(item).description_short).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "children_count",
                                        match (&(item).children_count).as_ref() {
                                            Some(item) => serde_json::json!(*(item)),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "risk_level",
                                        match (&(item).risk_level).as_ref() {
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
            Self::Export(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Export".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("instances", {
                            if (&(_field_0).instances).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).instances)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("workspace_id", mp::text(&(item).workspace_id)?),
                                            ("instance_id", mp::text(&(item).instance_id)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                            (
                                                "description_short",
                                                match (&(item).description_short).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "default_entry_path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).default_entry_path).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "webmcp_exposure",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).webmcp_exposure).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "managed_by",
                                                match (&(item).managed_by).as_ref() {
                                                    Some(item) => mp::object_value(&[
                                                        (
                                                            "organization",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).organization).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        ("bundle_id", mp::text(&(item).bundle_id)?),
                                                        (
                                                            "bundle_version",
                                                            mp::text(&(item).bundle_version)?,
                                                        ),
                                                    ]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "created_by",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).created_by).len()),
                                                )]),
                                            ),
                                            (
                                                "updated_by",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).updated_by).len()),
                                                )]),
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                            ("updated_at", mp::text(&(item).updated_at)?),
                                            (
                                                "llm_tool_registration",
                                                mp::object_value(&[
                                                    (
                                                        "prefix",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!(
                                                                (&(&(item).llm_tool_registration)
                                                                    .prefix)
                                                                    .len()
                                                            ),
                                                        )]),
                                                    ),
                                                    (
                                                        "tools",
                                                        mp::object_value(&[(
                                                            "item_count",
                                                            serde_json::json!(
                                                                (&(&(item).llm_tool_registration)
                                                                    .tools)
                                                                    .len()
                                                            ),
                                                        )]),
                                                    ),
                                                ]),
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
                            serde_json::Value::Array(
                                (&(_field_0).groups)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            (
                                                "path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).path).len()),
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
                                                "description_short",
                                                match (&(item).description_short).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "sort_order",
                                                serde_json::json!(*(&(item).sort_order)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("tools", {
                            if (&(_field_0).tools).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).tools).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("tool_id",mp::text(&(item).tool_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).short_description).len()))])), ("full_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).full_description).len()))])), ("execution_target",match &(item).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("operation",mp::text(&(item).operation)?), ("parameter_schema",mp::json_summary(&(item).parameter_schema)), ("result_schema",mp::json_summary(&(item).result_schema)), ("input_mapping",mp::json_summary(&(item).input_mapping)), ("output_mapping",mp::json_summary(&(item).output_mapping)), ("permission_code",match (&(item).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).risk_level).len()))])), ("des_id",mp::text(&(item).des_id)?), ("des_id_required",serde_json::Value::Bool(*(&(item).des_id_required))), ("status",mp::text(&(item).status)?), ("availability_status",match &(item).availability_status {crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::InterfaceMissing => mp::object_value(&[("variant",serde_json::Value::String("InterfaceMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamDisabled => mp::object_value(&[("variant",serde_json::Value::String("UpstreamDisabled".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::CredentialsMissing => mp::object_value(&[("variant",serde_json::Value::String("CredentialsMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamToolMissing => mp::object_value(&[("variant",serde_json::Value::String("UpstreamToolMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::MappingInvalid => mp::object_value(&[("variant",serde_json::Value::String("MappingInvalid".to_owned()))])}), ("availability_reason",match (&(item).availability_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("revision",serde_json::json!(*(&(item).revision))), ("managed_by",match (&(item).managed_by).as_ref() { Some(item) => mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("bundle_version",mp::text(&(item).bundle_version)?)]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("bindings", {
                            if (&(_field_0).bindings).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).bindings)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            ("tool_record_id", mp::text(&(item).tool_record_id)?),
                                            (
                                                "group_path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group_path).len()),
                                                )]),
                                            ),
                                            ("tool_id", mp::text(&(item).tool_id)?),
                                            (
                                                "display_alias",
                                                match (&(item).display_alias).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "visible",
                                                serde_json::Value::Bool(*(&(item).visible)),
                                            ),
                                            (
                                                "sort_order",
                                                serde_json::json!(*(&(item).sort_order)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("discovery_policies", {
                            if (&(_field_0).discovery_policies).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).discovery_policies)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("workspace_id", mp::text(&(item).workspace_id)?),
                                            (
                                                "instance_record_id",
                                                mp::text(&(item).instance_record_id)?,
                                            ),
                                            ("instance_id", mp::text(&(item).instance_id)?),
                                            (
                                                "list_default_limit",
                                                serde_json::json!(*(&(item).list_default_limit)),
                                            ),
                                            (
                                                "list_max_depth",
                                                serde_json::json!(*(&(item).list_max_depth)),
                                            ),
                                            (
                                                "list_regex_enabled",
                                                serde_json::Value::Bool(
                                                    *(&(item).list_regex_enabled),
                                                ),
                                            ),
                                            (
                                                "list_regex_max_length",
                                                serde_json::json!(*(&(item).list_regex_max_length)),
                                            ),
                                            (
                                                "list_return_fields",
                                                mp::json_summary(&(item).list_return_fields),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpCatalogAdapter(interface_catalog::McpInterfaceCatalogDependencies);

impl McpCatalogAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpCatalogInput,
    ) -> Result<McpCatalogOutput, ApiError> {
        let actor = principal.actor();
        let service = McpManagementService::new(self.0.store.clone());
        match input {
            McpCatalogInput::Catalog => {
                let snapshot = service.read_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpCatalogOutput::Catalog(to_catalog_response(
                    snapshot,
                    &operations,
                )?))
            }
            McpCatalogInput::Interfaces(query) => {
                service
                    .authorize_interface_catalog_view(actor.user_id)
                    .await?;
                let mut entries =
                    interface_catalog::mcp_interface_catalog_entries_with(&self.0, actor).await?;
                if query.bindable_only.unwrap_or(false) {
                    entries.retain(|entry| entry.bindable);
                }
                Ok(McpCatalogOutput::Interfaces(
                    entries.into_iter().map(to_interface_response).collect(),
                ))
            }
            McpCatalogInput::List(query) => {
                let items = service
                    .list_items_for_actor(
                        actor,
                        query.instance_id.as_deref(),
                        query.path.as_deref(),
                        query.path_regex.as_deref(),
                        query.keywords.as_deref(),
                        query.depth,
                        query.limit,
                    )
                    .await?;
                let instance_id = query.instance_id.as_deref().ok_or(
                    control_plane::errors::ControlPlaneError::InvalidInput("instance_id"),
                )?;
                let discovery_policy = service
                    .get_instance_discovery_policy_for_actor(actor, instance_id)
                    .await?;
                let return_fields = list_response_field_set(&discovery_policy.list_return_fields)?;
                Ok(McpCatalogOutput::List(
                    items
                        .into_iter()
                        .map(|item| to_list_item_response(item, &return_fields))
                        .collect(),
                ))
            }
            McpCatalogInput::Export => {
                let export = service.export_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpCatalogOutput::Export(to_export_response(
                    export,
                    &operations,
                )?))
            }
        }
    }
}

impl ConsoleInterfacePort<McpCatalogInput, McpCatalogOutput> for McpCatalogAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, McpCatalogOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.catalog.get.v1",
        method: "GET",
        path: "/api/console/mcp/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.interfaces.get.v1",
        method: "GET",
        path: "/api/console/mcp/interface-capabilities",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.list.get.v1",
        method: "GET",
        path: "/api/console/mcp/list",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.export",
        binding_id: "http.console.mcp.export.get.v1",
        method: "GET",
        path: "/api/console/mcp/export",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: interface_catalog::McpInterfaceCatalogDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-catalog",
        "graph:console-mcp-catalog-v1",
        DECLARATIONS,
        Arc::new(McpCatalogAdapter(dependencies)),
    )
}
