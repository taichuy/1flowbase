//! Fixed schema for ExtensionCenterOutput; every call constructs a fresh owned Value.
use crate::extension_bus::managed_projection as mp;
use serde_json::Value;
use Schema::*;

enum Schema {
    Object(&'static [(&'static str, Schema)]),
    OneOf(&'static [Schema]),
    AnyOf(&'static [Schema]),
    Array {
        max_items: usize,
        items: &'static Schema,
    },
    Text,
    Tag(&'static str),
    Count,
    JsonSummary,
    Integer,
    Boolean,
    Null,
}

pub(super) fn build() -> Value {
    build_schema(&DESCRIPTION)
}

fn build_schema(schema: &Schema) -> Value {
    match schema {
        Object(fields) => mp::object_schema(
            fields
                .iter()
                .map(|(name, schema)| (*name, build_schema(schema))),
        ),
        OneOf(variants) => mp::union_schema(variants.iter().map(build_schema).collect()),
        AnyOf(variants) => mp::object_value([(
            "anyOf",
            Value::Array(variants.iter().map(build_schema).collect()),
        )]),
        Array { max_items, items } => mp::object_value([
            ("type", Value::String("array".to_owned())),
            ("maxItems", Value::from(*max_items)),
            ("items", build_schema(items)),
        ]),
        Text => compact_leaf(mp::text_schema()),
        Tag(tag) => mp::tag_schema(tag),
        Count => compact_leaf(mp::count_schema()),
        JsonSummary => mp::json_summary_schema(),
        Integer => serde_json::json!({"type": "integer"}),
        Boolean => serde_json::json!({"type": "boolean"}),
        Null => serde_json::json!({"type": "null"}),
    }
}

// Keep the authoritative leaf shape while reserving only its actual field count.
// This replaces the capacity compaction formerly implicit in json! serialization.
fn compact_leaf(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(fields.into_iter().collect()),
        value => value,
    }
}

// Repeated metadata shapes remain local to this owner's immutable description.
const BYTE_COUNT: Schema = Object(&[("byte_count", Count)]);
const ITEM_COUNT: Schema = Object(&[("item_count", Count)]);
const OPTIONAL_BYTE_COUNT: Schema = AnyOf(&[BYTE_COUNT, Null]);
const OPTIONAL_TEXT: Schema = AnyOf(&[Text, Null]);
const OPTIONAL_JSON_SUMMARY: Schema = AnyOf(&[JsonSummary, Null]);

const DESCRIPTION: Schema = OneOf(&[
    Object(&[
        ("variant", Tag("ManagedExecution")),
        (
            "0",
            Object(&[
                ("installation_id", Text),
                ("workspace_id", Text),
                (
                    "executions",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("frozen_reference_count", Integer),
                            (
                                "target",
                                Object(&[
                                    ("graph_fingerprint", BYTE_COUNT),
                                    ("handler_id", Text),
                                    ("handler_version", Text),
                                ]),
                            ),
                            ("current", Boolean),
                        ]),
                    },
                ),
                (
                    "deliveries",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("event_id", Text),
                            ("subscriber_id", Text),
                            (
                                "target",
                                Object(&[
                                    ("graph_fingerprint", BYTE_COUNT),
                                    ("handler_id", Text),
                                    ("handler_version", Text),
                                ]),
                            ),
                            ("status", Text),
                            ("pause_reason", OPTIONAL_BYTE_COUNT),
                            ("ownership", BYTE_COUNT),
                        ]),
                    },
                ),
                ("deliveries_truncated", Boolean),
                ("next_cursor", OPTIONAL_BYTE_COUNT),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("ContributionAuthorizations")),
        (
            "0",
            Object(&[
                ("installation_id", Text),
                ("workspace_id", Text),
                ("revision", Integer),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Installed")),
        (
            "0",
            Object(&[
                ("limit", Integer),
                ("total_entries", Integer),
                ("next_cursor", OPTIONAL_BYTE_COUNT),
                (
                    "entries",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("contract_version", OPTIONAL_TEXT),
                            ("id", Text),
                            ("catalog_id", Text),
                            ("category", BYTE_COUNT),
                            ("organization", BYTE_COUNT),
                            ("artifact_id", Text),
                            ("version", Text),
                            ("node_id", Text),
                            ("source_kind", BYTE_COUNT),
                            ("trust_level", BYTE_COUNT),
                            ("warnings", ITEM_COUNT),
                            ("expected_checksum", OPTIONAL_BYTE_COUNT),
                            ("local_checksum", OPTIONAL_BYTE_COUNT),
                            ("signature_status", BYTE_COUNT),
                            ("signature_algorithm", OPTIONAL_BYTE_COUNT),
                            ("signing_key_id", OPTIONAL_TEXT),
                            ("status", Text),
                            ("is_current", Boolean),
                            ("runtime_status", OPTIONAL_BYTE_COUNT),
                            ("desired_state", OPTIONAL_BYTE_COUNT),
                            ("availability_status", OPTIONAL_BYTE_COUNT),
                            ("application_action", BYTE_COUNT),
                            ("application_status", BYTE_COUNT),
                            ("created_by", BYTE_COUNT),
                            ("created_at", Text),
                            ("updated_at", Text),
                            ("installed_versions", ITEM_COUNT),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Installation")),
        (
            "0",
            Object(&[
                ("contract_version", OPTIONAL_TEXT),
                ("id", Text),
                ("catalog_id", Text),
                ("category", BYTE_COUNT),
                ("organization", BYTE_COUNT),
                ("artifact_id", Text),
                ("version", Text),
                ("node_id", Text),
                ("source_kind", BYTE_COUNT),
                ("trust_level", BYTE_COUNT),
                (
                    "warnings",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("code", Text),
                            ("message", BYTE_COUNT),
                            ("overridable", Boolean),
                        ]),
                    },
                ),
                ("expected_checksum", OPTIONAL_BYTE_COUNT),
                ("local_checksum", OPTIONAL_BYTE_COUNT),
                ("signature_status", BYTE_COUNT),
                ("signature_algorithm", OPTIONAL_BYTE_COUNT),
                ("signing_key_id", OPTIONAL_TEXT),
                ("status", Text),
                ("is_current", Boolean),
                ("runtime_status", OPTIONAL_BYTE_COUNT),
                ("desired_state", OPTIONAL_BYTE_COUNT),
                ("availability_status", OPTIONAL_BYTE_COUNT),
                ("application_action", BYTE_COUNT),
                ("application_status", BYTE_COUNT),
                ("created_by", BYTE_COUNT),
                ("created_at", Text),
                ("updated_at", Text),
                (
                    "installed_versions",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("contract_version", OPTIONAL_TEXT),
                            ("id", Text),
                            ("version", Text),
                            ("source_kind", BYTE_COUNT),
                            ("trust_level", BYTE_COUNT),
                            ("warnings", ITEM_COUNT),
                            ("expected_checksum", OPTIONAL_BYTE_COUNT),
                            ("local_checksum", OPTIONAL_BYTE_COUNT),
                            ("signature_status", BYTE_COUNT),
                            ("signature_algorithm", OPTIONAL_BYTE_COUNT),
                            ("signing_key_id", OPTIONAL_TEXT),
                            ("status", Text),
                            ("is_current", Boolean),
                            ("deletable", Boolean),
                            ("delete_reasons", ITEM_COUNT),
                            ("created_by", BYTE_COUNT),
                            ("created_at", Text),
                            ("updated_at", Text),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Task")),
        (
            "0",
            Object(&[
                ("id", Text),
                ("installation_id", OPTIONAL_TEXT),
                ("workspace_id", OPTIONAL_TEXT),
                ("provider_code", Text),
                ("task_kind", BYTE_COUNT),
                ("status", Text),
                ("status_message", OPTIONAL_BYTE_COUNT),
                ("detail_json", JsonSummary),
                ("created_at", Text),
                ("updated_at", Text),
                ("finished_at", OPTIONAL_BYTE_COUNT),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Catalog")),
        (
            "0",
            Object(&[
                ("category", BYTE_COUNT),
                ("freshness", BYTE_COUNT),
                ("catalog_page", BYTE_COUNT),
                ("catalog_page_number", Integer),
                ("catalog_page_checksum", BYTE_COUNT),
                ("catalog_page_locator", BYTE_COUNT),
                ("limit", Integer),
                ("next_cursor", OPTIONAL_BYTE_COUNT),
                ("total_entries", Integer),
                (
                    "entries",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("category", BYTE_COUNT),
                            ("id", Text),
                            ("name", BYTE_COUNT),
                            ("organization", BYTE_COUNT),
                            ("artifact", BYTE_COUNT),
                            ("version", Text),
                            ("description", BYTE_COUNT),
                            ("host_version_requirement", BYTE_COUNT),
                            ("slot_codes", ITEM_COUNT),
                            ("keywords", ITEM_COUNT),
                            ("source", JsonSummary),
                            ("signature", OPTIONAL_JSON_SUMMARY),
                            ("checksum", OPTIONAL_BYTE_COUNT),
                            ("download_locator", JsonSummary),
                            ("catalog_page", Integer),
                            ("catalog_source", BYTE_COUNT),
                            ("current_version", OPTIONAL_TEXT),
                            ("installation_status", BYTE_COUNT),
                            ("artifact_kind", OPTIONAL_BYTE_COUNT),
                            ("installation_source", OPTIONAL_BYTE_COUNT),
                            ("extension_installation_id", OPTIONAL_TEXT),
                            ("builtin_template_id", OPTIONAL_TEXT),
                            ("trust", BYTE_COUNT),
                            ("warnings", ITEM_COUNT),
                            (
                                "compatibility",
                                AnyOf(&[
                                    Object(&[
                                        ("reason", BYTE_COUNT),
                                        ("current_host_version", Text),
                                        ("minimum_host_version", Text),
                                    ]),
                                    Null,
                                ]),
                            ),
                            ("mcp_instances", ITEM_COUNT),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("CatalogEntry")),
        (
            "0",
            Object(&[
                ("category", BYTE_COUNT),
                ("id", Text),
                ("name", BYTE_COUNT),
                ("organization", BYTE_COUNT),
                ("artifact", BYTE_COUNT),
                ("version", Text),
                ("description", BYTE_COUNT),
                ("host_version_requirement", BYTE_COUNT),
                ("slot_codes", ITEM_COUNT),
                ("keywords", ITEM_COUNT),
                ("source", JsonSummary),
                ("signature", OPTIONAL_JSON_SUMMARY),
                ("checksum", OPTIONAL_BYTE_COUNT),
                ("download_locator", JsonSummary),
                ("catalog_page", Integer),
                ("catalog_source", BYTE_COUNT),
                ("current_version", OPTIONAL_TEXT),
                ("installation_status", BYTE_COUNT),
                ("artifact_kind", OPTIONAL_BYTE_COUNT),
                ("installation_source", OPTIONAL_BYTE_COUNT),
                ("extension_installation_id", OPTIONAL_TEXT),
                ("builtin_template_id", OPTIONAL_TEXT),
                ("trust", BYTE_COUNT),
                (
                    "warnings",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("code", Text),
                            ("message", BYTE_COUNT),
                            ("overridable", Boolean),
                        ]),
                    },
                ),
                (
                    "compatibility",
                    AnyOf(&[
                        Object(&[
                            ("reason", BYTE_COUNT),
                            ("current_host_version", Text),
                            ("minimum_host_version", Text),
                        ]),
                        Null,
                    ]),
                ),
                (
                    "mcp_instances",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("instance_id", Text),
                            ("name", BYTE_COUNT),
                            ("description_short", OPTIONAL_BYTE_COUNT),
                            ("workspace_status", BYTE_COUNT),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Updates")),
        (
            "0",
            Object(&[
                ("category", BYTE_COUNT),
                (
                    "items",
                    Array {
                        max_items: 32,
                        items: &Object(&[
                            ("catalog_id", Text),
                            ("current_version", Text),
                            ("latest_version", OPTIONAL_TEXT),
                            ("status", Text),
                        ]),
                    },
                ),
            ]),
        ),
    ]),
    Object(&[
        ("variant", Tag("Install")),
        (
            "0",
            OneOf(&[
                Object(&[
                    ("variant", Tag("Existing")),
                    (
                        "0",
                        Object(&[
                            (
                                "installation",
                                Object(&[
                                    ("contract_version", OPTIONAL_TEXT),
                                    ("id", Text),
                                    ("catalog_id", Text),
                                    ("category", BYTE_COUNT),
                                    ("organization", BYTE_COUNT),
                                    ("artifact_id", Text),
                                    ("version", Text),
                                    ("node_id", Text),
                                    ("source_kind", BYTE_COUNT),
                                    ("trust_level", BYTE_COUNT),
                                    ("warnings", ITEM_COUNT),
                                    ("expected_checksum", OPTIONAL_BYTE_COUNT),
                                    ("local_checksum", OPTIONAL_BYTE_COUNT),
                                    ("signature_status", BYTE_COUNT),
                                    ("signature_algorithm", OPTIONAL_BYTE_COUNT),
                                    ("signing_key_id", OPTIONAL_TEXT),
                                    ("status", Text),
                                    ("is_current", Boolean),
                                    ("runtime_status", OPTIONAL_BYTE_COUNT),
                                    ("desired_state", OPTIONAL_BYTE_COUNT),
                                    ("availability_status", OPTIONAL_BYTE_COUNT),
                                    ("application_action", BYTE_COUNT),
                                    ("application_status", BYTE_COUNT),
                                    ("created_by", BYTE_COUNT),
                                    ("created_at", Text),
                                    ("updated_at", Text),
                                    ("installed_versions", ITEM_COUNT),
                                ]),
                            ),
                            ("local_artifact_was_present", Boolean),
                            ("node_plugin_installation_id", OPTIONAL_TEXT),
                            ("application_action", BYTE_COUNT),
                            ("application_status", BYTE_COUNT),
                            (
                                "managed_schema_preview",
                                AnyOf(&[
                                    Object(&[
                                        ("owner_id", Text),
                                        ("fingerprint", BYTE_COUNT),
                                        ("entries", ITEM_COUNT),
                                    ]),
                                    Null,
                                ]),
                            ),
                            (
                                "managed_schema_receipt",
                                AnyOf(&[
                                    Object(&[
                                        ("receipt_id", Text),
                                        ("owner_id", Text),
                                        ("owner_version", Text),
                                        ("fingerprint", BYTE_COUNT),
                                        ("created_objects", Integer),
                                        ("existing_objects", Integer),
                                        ("retained_objects", Integer),
                                        ("applied_at", BYTE_COUNT),
                                    ]),
                                    Null,
                                ]),
                            ),
                        ]),
                    ),
                ]),
                Object(&[
                    ("variant", Tag("Created")),
                    (
                        "0",
                        Object(&[
                            (
                                "installation",
                                Object(&[
                                    ("contract_version", OPTIONAL_TEXT),
                                    ("id", Text),
                                    ("catalog_id", Text),
                                    ("category", BYTE_COUNT),
                                    ("organization", BYTE_COUNT),
                                    ("artifact_id", Text),
                                    ("version", Text),
                                    ("node_id", Text),
                                    ("source_kind", BYTE_COUNT),
                                    ("trust_level", BYTE_COUNT),
                                    ("warnings", ITEM_COUNT),
                                    ("expected_checksum", OPTIONAL_BYTE_COUNT),
                                    ("local_checksum", OPTIONAL_BYTE_COUNT),
                                    ("signature_status", BYTE_COUNT),
                                    ("signature_algorithm", OPTIONAL_BYTE_COUNT),
                                    ("signing_key_id", OPTIONAL_TEXT),
                                    ("status", Text),
                                    ("is_current", Boolean),
                                    ("runtime_status", OPTIONAL_BYTE_COUNT),
                                    ("desired_state", OPTIONAL_BYTE_COUNT),
                                    ("availability_status", OPTIONAL_BYTE_COUNT),
                                    ("application_action", BYTE_COUNT),
                                    ("application_status", BYTE_COUNT),
                                    ("created_by", BYTE_COUNT),
                                    ("created_at", Text),
                                    ("updated_at", Text),
                                    ("installed_versions", ITEM_COUNT),
                                ]),
                            ),
                            ("local_artifact_was_present", Boolean),
                            ("node_plugin_installation_id", OPTIONAL_TEXT),
                            ("application_action", BYTE_COUNT),
                            ("application_status", BYTE_COUNT),
                            (
                                "managed_schema_preview",
                                AnyOf(&[
                                    Object(&[
                                        ("owner_id", Text),
                                        ("fingerprint", BYTE_COUNT),
                                        ("entries", ITEM_COUNT),
                                    ]),
                                    Null,
                                ]),
                            ),
                            (
                                "managed_schema_receipt",
                                AnyOf(&[
                                    Object(&[
                                        ("receipt_id", Text),
                                        ("owner_id", Text),
                                        ("owner_version", Text),
                                        ("fingerprint", BYTE_COUNT),
                                        ("created_objects", Integer),
                                        ("existing_objects", Integer),
                                        ("retained_objects", Integer),
                                        ("applied_at", BYTE_COUNT),
                                    ]),
                                    Null,
                                ]),
                            ),
                        ]),
                    ),
                ]),
                Object(&[
                    ("variant", Tag("Challenge")),
                    (
                        "0",
                        Object(&[
                            ("warnings", ITEM_COUNT),
                            (
                                "compatibility",
                                AnyOf(&[
                                    Object(&[
                                        ("reason", BYTE_COUNT),
                                        ("current_host_version", Text),
                                        ("minimum_host_version", Text),
                                    ]),
                                    Null,
                                ]),
                            ),
                        ]),
                    ),
                ]),
            ]),
        ),
    ]),
]);
