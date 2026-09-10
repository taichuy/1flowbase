use super::*;

impl InterfaceContract for ExtensionCenterInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("QueryManagedExecution")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResumeManagedDelivery")),
                ("0", mp::text_schema()),
                (
                    "1",
                    mp::object_schema(&[
                        ("event_id", mp::text_schema()),
                        ("subscriber_id", mp::text_schema()),
                        (
                            "expected",
                            mp::object_schema(&[
                                (
                                    "graph_fingerprint",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("handler_id", mp::text_schema()),
                                ("handler_version", mp::text_schema()),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RetireManagedExecution")),
                ("0", mp::text_schema()),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "graph_fingerprint",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("handler_id", mp::text_schema()),
                        ("handler_version", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GrantContributionPermission")),
                ("0", mp::text_schema()),
                (
                    "1",
                    mp::object_schema(&[
                        ("contribution_id", mp::text_schema()),
                        (
                            "permission",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "resource_scope",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Workspace"))]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("OwnedCollection")),
                                    ("collection_code", mp::text_schema()),
                                ]),
                            ]),
                        ),
                        ("permission_contract_id", mp::text_schema()),
                        ("permission_contract_version", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RevokeContributionPermission")),
                ("0", mp::text_schema()),
                (
                    "1",
                    mp::object_schema(&[(
                        "expected_revision",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("QueryContributionAuthorizations")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListInstalled")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Select")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Enable")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Disable")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListCatalog")),
                (
                    "category",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "slot_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "q",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetCatalog")),
                (
                    "category",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("catalog_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CheckUpdates")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("catalog_id",mp::text_schema()), ("current_version",mp::text_schema()), ("installed_versions",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstallOfficial")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("catalog_id", mp::text_schema()),
                        ("version", mp::text_schema()),
                        (
                            "compatibility_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_current_host_version",mp::text_schema()), ("acknowledged_minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "risk_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateOfficial")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("catalog_id", mp::text_schema()),
                        ("version", mp::text_schema()),
                        (
                            "compatibility_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_current_host_version",mp::text_schema()), ("acknowledged_minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "risk_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("InstallUploaded"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::QueryManagedExecution(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("QueryManagedExecution".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::ResumeManagedDelivery(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("ResumeManagedDelivery".to_owned())), ("0",serde_json::Value::String((_field_0).to_string())), ("1",mp::object_value(&[("event_id",serde_json::Value::String((&(_field_1).event_id).to_string())), ("subscriber_id",mp::text(&(_field_1).subscriber_id)?), ("expected",mp::object_value(&[("graph_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_1).expected).graph_fingerprint).len()))])), ("handler_id",mp::text(&(&(_field_1).expected).handler_id)?), ("handler_version",mp::text(&(&(_field_1).expected).handler_version)?)]))]))]), Self::RetireManagedExecution(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("RetireManagedExecution".to_owned())), ("0",serde_json::Value::String((_field_0).to_string())), ("1",mp::object_value(&[("graph_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).graph_fingerprint).len()))])), ("handler_id",mp::text(&(_field_1).handler_id)?), ("handler_version",mp::text(&(_field_1).handler_version)?)]))]), Self::GrantContributionPermission(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("GrantContributionPermission".to_owned())), ("0",serde_json::Value::String((_field_0).to_string())), ("1",mp::object_value(&[("contribution_id",mp::text(&(_field_1).contribution_id)?), ("permission",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).permission).len()))])), ("resource_scope",match &(_field_1).resource_scope {crate::routes::plugins_and_models_group::plugins::extension_center::dto::ContributionResourceScopeDto::Workspace => mp::object_value(&[("variant",serde_json::Value::String("Workspace".to_owned()))]), crate::routes::plugins_and_models_group::plugins::extension_center::dto::ContributionResourceScopeDto::OwnedCollection {collection_code: _field_collection_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("OwnedCollection".to_owned())), ("collection_code",mp::text(_field_collection_code)?)])}), ("permission_contract_id",mp::text(&(_field_1).permission_contract_id)?), ("permission_contract_version",mp::text(&(_field_1).permission_contract_version)?)]))]), Self::RevokeContributionPermission(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("RevokeContributionPermission".to_owned())), ("0",serde_json::Value::String((_field_0).to_string())), ("1",mp::object_value(&[("expected_revision",serde_json::json!(*(&(_field_1).expected_revision)))]))]), Self::QueryContributionAuthorizations(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("QueryContributionAuthorizations".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::ListInstalled(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ListInstalled".to_owned())), ("0",mp::object_value(&[("category",match (&(_field_0).category).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("cursor",match (&(_field_0).cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("limit",match (&(_field_0).limit).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]))]), Self::Select(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Select".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::Enable(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Enable".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::Disable(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Disable".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::Delete(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::ListCatalog {category: _field_category, query: _field_query, .. } => mp::object_value(&[("variant",serde_json::Value::String("ListCatalog".to_owned())), ("category",mp::object_value(&[("byte_count",serde_json::json!((_field_category).len()))])), ("query",mp::object_value(&[("slot_code",match (&(_field_query).slot_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("q",match (&(_field_query).q).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("limit",match (&(_field_query).limit).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("cursor",match (&(_field_query).cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::GetCatalog {category: _field_category, catalog_id: _field_catalog_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("GetCatalog".to_owned())), ("category",mp::object_value(&[("byte_count",serde_json::json!((_field_category).len()))])), ("catalog_id",mp::text(_field_catalog_id)?)]), Self::CheckUpdates(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("CheckUpdates".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("catalog_id",mp::text(&(item).catalog_id)?), ("current_version",mp::text(&(item).current_version)?), ("installed_versions",mp::object_value(&[("item_count",serde_json::json!((&(item).installed_versions).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::InstallOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("InstallOfficial".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("catalog_id",mp::text(&(_field_0).catalog_id)?), ("version",mp::text(&(_field_0).version)?), ("compatibility_override",match (&(_field_0).compatibility_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_current_host_version",mp::text(&(item).acknowledged_current_host_version)?), ("acknowledged_minimum_host_version",mp::text(&(item).acknowledged_minimum_host_version)?)]), None => serde_json::Value::Null }), ("risk_override",match (&(_field_0).risk_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).acknowledged_warnings).len()))]))]), None => serde_json::Value::Null })]))]), Self::UpdateOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("UpdateOfficial".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("catalog_id",mp::text(&(_field_0).catalog_id)?), ("version",mp::text(&(_field_0).version)?), ("compatibility_override",match (&(_field_0).compatibility_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_current_host_version",mp::text(&(item).acknowledged_current_host_version)?), ("acknowledged_minimum_host_version",mp::text(&(item).acknowledged_minimum_host_version)?)]), None => serde_json::Value::Null }), ("risk_override",match (&(_field_0).risk_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).acknowledged_warnings).len()))]))]), None => serde_json::Value::Null })]))]), Self::InstallUploaded(_) => mp::object_value(&[("variant",serde_json::Value::String("InstallUploaded".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-extension-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for ExtensionCenterOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ManagedExecution")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        (
                            "executions",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("frozen_reference_count",serde_json::json!({"type":"integer"})), ("target",mp::object_schema(&[("graph_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("handler_id",mp::text_schema()), ("handler_version",mp::text_schema())])), ("current",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "deliveries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("event_id",mp::text_schema()), ("subscriber_id",mp::text_schema()), ("target",mp::object_schema(&[("graph_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("handler_id",mp::text_schema()), ("handler_version",mp::text_schema())])), ("status",mp::text_schema()), ("pause_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("ownership",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "deliveries_truncated",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ContributionAuthorizations")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Installed")),
                (
                    "0",
                    mp::object_schema(&[
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("total_entries", serde_json::json!({"type":"integer"})),
                        (
                            "next_cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("contract_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("id",mp::text_schema()), ("catalog_id",mp::text_schema()), ("category",mp::object_schema(&[("byte_count",mp::count_schema())])), ("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_id",mp::text_schema()), ("version",mp::text_schema()), ("node_id",mp::text_schema()), ("source_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("trust_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("warnings",mp::object_schema(&[("item_count",mp::count_schema())])), ("expected_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("local_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signature_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("signature_algorithm",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signing_key_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("is_current",serde_json::json!({"type":"boolean"})), ("runtime_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})),
("desired_state",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("availability_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("application_action",mp::object_schema(&[("byte_count",mp::count_schema())])), ("application_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema()), ("installed_versions",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Installation")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "contract_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("id", mp::text_schema()),
                        ("catalog_id", mp::text_schema()),
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "organization",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("artifact_id", mp::text_schema()),
                        ("version", mp::text_schema()),
                        ("node_id", mp::text_schema()),
                        (
                            "source_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "trust_level",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "warnings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("overridable",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "expected_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "local_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "signature_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "signature_algorithm",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "signing_key_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        ("is_current", serde_json::json!({"type":"boolean"})),
                        (
                            "runtime_status",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "desired_state",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "availability_status",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "application_action",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "application_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "created_by",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                        (
                            "installed_versions",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("contract_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("id",mp::text_schema()), ("version",mp::text_schema()), ("source_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("trust_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("warnings",mp::object_schema(&[("item_count",mp::count_schema())])), ("expected_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("local_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signature_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("signature_algorithm",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signing_key_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("is_current",serde_json::json!({"type":"boolean"})), ("deletable",serde_json::json!({"type":"boolean"})), ("delete_reasons",mp::object_schema(&[("item_count",mp::count_schema())])), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Task")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "installation_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "workspace_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("provider_code", mp::text_schema()),
                        (
                            "task_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "status_message",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("detail_json", mp::json_summary_schema()),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                        (
                            "finished_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "freshness",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "catalog_page",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("catalog_page_number", serde_json::json!({"type":"integer"})),
                        (
                            "catalog_page_checksum",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "catalog_page_locator",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("limit", serde_json::json!({"type":"integer"})),
                        (
                            "next_cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("total_entries", serde_json::json!({"type":"integer"})),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("category",mp::object_schema(&[("byte_count",mp::count_schema())])), ("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema()), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("host_version_requirement",mp::object_schema(&[("byte_count",mp::count_schema())])), ("slot_codes",mp::object_schema(&[("item_count",mp::count_schema())])), ("keywords",mp::object_schema(&[("item_count",mp::count_schema())])), ("source",mp::json_summary_schema()), ("signature",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("download_locator",mp::json_summary_schema()), ("catalog_page",serde_json::json!({"type":"integer"})), ("catalog_source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("installation_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("installation_source",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("extension_installation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("builtin_template_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("trust",mp::object_schema(&[("byte_count",mp::count_schema())])), ("warnings",mp::object_schema(&[("item_count",mp::count_schema())])), ("compatibility",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]})), ("mcp_instances",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogEntry")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "organization",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "artifact",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "host_version_requirement",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "slot_codes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "keywords",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        ("source", mp::json_summary_schema()),
                        (
                            "signature",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        (
                            "checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("download_locator", mp::json_summary_schema()),
                        ("catalog_page", serde_json::json!({"type":"integer"})),
                        (
                            "catalog_source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "current_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "installation_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "artifact_kind",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "installation_source",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "extension_installation_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "builtin_template_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "trust",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "warnings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("overridable",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "compatibility",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "mcp_instances",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("instance_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("workspace_status",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Updates")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "category",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("catalog_id",mp::text_schema()), ("current_version",mp::text_schema()), ("latest_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Install")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Existing")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "installation",
                                        mp::object_schema(&[
                                            (
                                                "contract_version",
                                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                            ),
                                            ("id", mp::text_schema()),
                                            ("catalog_id", mp::text_schema()),
                                            (
                                                "category",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "organization",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("artifact_id", mp::text_schema()),
                                            ("version", mp::text_schema()),
                                            ("node_id", mp::text_schema()),
                                            (
                                                "source_kind",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "trust_level",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "warnings",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "expected_checksum",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "local_checksum",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "signature_status",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "signature_algorithm",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "signing_key_id",
                                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                            ),
                                            ("status", mp::text_schema()),
                                            ("is_current", serde_json::json!({"type":"boolean"})),
                                            (
                                                "runtime_status",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "desired_state",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "availability_status",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "application_action",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "application_status",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "created_by",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("created_at", mp::text_schema()),
                                            ("updated_at", mp::text_schema()),
                                            (
                                                "installed_versions",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                    (
                                        "local_artifact_was_present",
                                        serde_json::json!({"type":"boolean"}),
                                    ),
                                    (
                                        "node_plugin_installation_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "application_action",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "managed_schema_preview",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("owner_id",mp::text_schema()), ("fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("entries",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                    (
                                        "managed_schema_receipt",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("receipt_id",mp::text_schema()), ("owner_id",mp::text_schema()), ("owner_version",mp::text_schema()), ("fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_objects",serde_json::json!({"type":"integer"})), ("existing_objects",serde_json::json!({"type":"integer"})), ("retained_objects",serde_json::json!({"type":"integer"})), ("applied_at",mp::object_schema(&[("byte_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Created")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "installation",
                                        mp::object_schema(&[
                                            (
                                                "contract_version",
                                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                            ),
                                            ("id", mp::text_schema()),
                                            ("catalog_id", mp::text_schema()),
                                            (
                                                "category",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "organization",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("artifact_id", mp::text_schema()),
                                            ("version", mp::text_schema()),
                                            ("node_id", mp::text_schema()),
                                            (
                                                "source_kind",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "trust_level",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "warnings",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "expected_checksum",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "local_checksum",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "signature_status",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "signature_algorithm",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "signing_key_id",
                                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                            ),
                                            ("status", mp::text_schema()),
                                            ("is_current", serde_json::json!({"type":"boolean"})),
                                            (
                                                "runtime_status",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "desired_state",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "availability_status",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "application_action",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "application_status",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "created_by",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("created_at", mp::text_schema()),
                                            ("updated_at", mp::text_schema()),
                                            (
                                                "installed_versions",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                    (
                                        "local_artifact_was_present",
                                        serde_json::json!({"type":"boolean"}),
                                    ),
                                    (
                                        "node_plugin_installation_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "application_action",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "managed_schema_preview",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("owner_id",mp::text_schema()), ("fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("entries",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                    (
                                        "managed_schema_receipt",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("receipt_id",mp::text_schema()), ("owner_id",mp::text_schema()), ("owner_version",mp::text_schema()), ("fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_objects",serde_json::json!({"type":"integer"})), ("existing_objects",serde_json::json!({"type":"integer"})), ("retained_objects",serde_json::json!({"type":"integer"})), ("applied_at",mp::object_schema(&[("byte_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Challenge")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "warnings",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "compatibility",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::ManagedExecution(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ManagedExecution".to_owned())), ("0",mp::object_value(&[("installation_id",serde_json::Value::String((&(_field_0).installation_id).to_string())), ("workspace_id",serde_json::Value::String((&(_field_0).workspace_id).to_string())), ("executions",{ if (&(_field_0).executions).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).executions).iter().map(|item| Some(mp::object_value(&[("frozen_reference_count",serde_json::json!(*(&(item).frozen_reference_count))), ("target",mp::object_value(&[("graph_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).target).graph_fingerprint).len()))])), ("handler_id",mp::text(&(&(item).target).handler_id)?), ("handler_version",mp::text(&(&(item).target).handler_version)?)])), ("current",serde_json::Value::Bool(*(&(item).current)))]))).collect::<Option<Vec<_>>>()?) }), ("deliveries",{ if (&(_field_0).deliveries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).deliveries).iter().map(|item| Some(mp::object_value(&[("event_id",serde_json::Value::String((&(item).event_id).to_string())), ("subscriber_id",mp::text(&(item).subscriber_id)?), ("target",mp::object_value(&[("graph_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).target).graph_fingerprint).len()))])), ("handler_id",mp::text(&(&(item).target).handler_id)?), ("handler_version",mp::text(&(&(item).target).handler_version)?)])), ("status",mp::text(&(item).status)?), ("pause_reason",match (&(item).pause_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("ownership",mp::object_value(&[("byte_count",serde_json::json!((&(item).ownership).len()))]))]))).collect::<Option<Vec<_>>>()?) }), ("deliveries_truncated",serde_json::Value::Bool(*(&(_field_0).deliveries_truncated)))]))]), Self::ContributionAuthorizations(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ContributionAuthorizations".to_owned())), ("0",mp::object_value(&[("installation_id",serde_json::Value::String((&(_field_0).installation_id).to_string())), ("workspace_id",serde_json::Value::String((&(_field_0).workspace_id).to_string())), ("revision",serde_json::json!(*(&(_field_0).revision)))]))]), Self::Installed(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Installed".to_owned())), ("0",mp::object_value(&[("limit",serde_json::json!(*(&(_field_0).limit))), ("total_entries",serde_json::json!(*(&(_field_0).total_entries))), ("next_cursor",match (&(_field_0).next_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("contract_version",match (&(item).contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("id",mp::text(&(item).id)?), ("catalog_id",mp::text(&(item).catalog_id)?), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(item).category).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("artifact_id",mp::text(&(item).artifact_id)?), ("version",mp::text(&(item).version)?), ("node_id",mp::text(&(item).node_id)?), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).trust_level).len()))])), ("warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).warnings).len()))])), ("expected_checksum",match (&(item).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_checksum",match (&(item).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).signature_status).len()))])), ("signature_algorithm",match (&(item).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(item).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("is_current",serde_json::Value::Bool(*(&(item).is_current))), ("runtime_status",match (&(item).runtime_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }),
("desired_state",match (&(item).desired_state).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("availability_status",match (&(item).availability_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(item).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).application_status).len()))])), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(item).created_by).len()))])), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?), ("installed_versions",mp::object_value(&[("item_count",serde_json::json!((&(item).installed_versions).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Installation(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Installation".to_owned())), ("0",mp::object_value(&[("contract_version",match (&(_field_0).contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("id",mp::text(&(_field_0).id)?), ("catalog_id",mp::text(&(_field_0).catalog_id)?), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("artifact_id",mp::text(&(_field_0).artifact_id)?), ("version",mp::text(&(_field_0).version)?), ("node_id",mp::text(&(_field_0).node_id)?), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).trust_level).len()))])), ("warnings",{ if (&(_field_0).warnings).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).warnings).iter().map(|item| Some(mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("overridable",serde_json::Value::Bool(*(&(item).overridable)))]))).collect::<Option<Vec<_>>>()?) }), ("expected_checksum",match (&(_field_0).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_checksum",match (&(_field_0).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).signature_status).len()))])), ("signature_algorithm",match (&(_field_0).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(_field_0).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(_field_0).status)?), ("is_current",serde_json::Value::Bool(*(&(_field_0).is_current))), ("runtime_status",match (&(_field_0).runtime_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }),
("desired_state",match (&(_field_0).desired_state).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("availability_status",match (&(_field_0).availability_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_status).len()))])), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).created_by).len()))])), ("created_at",mp::text(&(_field_0).created_at)?), ("updated_at",mp::text(&(_field_0).updated_at)?), ("installed_versions",{ if (&(_field_0).installed_versions).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).installed_versions).iter().map(|item| Some(mp::object_value(&[("contract_version",match (&(item).contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("id",mp::text(&(item).id)?), ("version",mp::text(&(item).version)?), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).trust_level).len()))])), ("warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).warnings).len()))])), ("expected_checksum",match (&(item).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_checksum",match (&(item).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).signature_status).len()))])), ("signature_algorithm",match (&(item).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(item).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("is_current",serde_json::Value::Bool(*(&(item).is_current))), ("deletable",serde_json::Value::Bool(*(&(item).deletable))), ("delete_reasons",mp::object_value(&[("item_count",serde_json::json!((&(item).delete_reasons).len()))])), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(item).created_by).len()))])), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Task(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Task".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("installation_id",match (&(_field_0).installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("workspace_id",match (&(_field_0).workspace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(_field_0).provider_code)?), ("task_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).task_kind).len()))])), ("status",mp::text(&(_field_0).status)?), ("status_message",match (&(_field_0).status_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("detail_json",mp::json_summary(&(_field_0).detail_json)), ("created_at",mp::text(&(_field_0).created_at)?), ("updated_at",mp::text(&(_field_0).updated_at)?), ("finished_at",match (&(_field_0).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Catalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Catalog".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("freshness",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).freshness).len()))])), ("catalog_page",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).catalog_page).len()))])), ("catalog_page_number",serde_json::json!(*(&(_field_0).catalog_page_number))), ("catalog_page_checksum",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).catalog_page_checksum).len()))])), ("catalog_page_locator",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).catalog_page_locator).len()))])), ("limit",serde_json::json!(*(&(_field_0).limit))), ("next_cursor",match (&(_field_0).next_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("total_entries",serde_json::json!(*(&(_field_0).total_entries))), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(item).category).len()))])), ("id",mp::text(&(item).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("artifact",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact).len()))])), ("version",mp::text(&(item).version)?), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("host_version_requirement",mp::object_value(&[("byte_count",serde_json::json!((&(item).host_version_requirement).len()))])), ("slot_codes",mp::object_value(&[("item_count",serde_json::json!((&(item).slot_codes).len()))])), ("keywords",mp::object_value(&[("item_count",serde_json::json!((&(item).keywords).len()))])), ("source",mp::json_summary(&(item).source)), ("signature",match (&(item).signature).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("checksum",match (&(item).checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("download_locator",mp::json_summary(&(item).download_locator)), ("catalog_page",serde_json::json!(*(&(item).catalog_page))), ("catalog_source",mp::object_value(&[("byte_count",serde_json::json!((&(item).catalog_source).len()))])), ("current_version",match (&(item).current_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).installation_status).len()))])), ("artifact_kind",match (&(item).artifact_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("installation_source",match (&(item).installation_source).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("extension_installation_id",match (&(item).extension_installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("builtin_template_id",match (&(item).builtin_template_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("trust",mp::object_value(&[("byte_count",serde_json::json!((&(item).trust).len()))])), ("warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).warnings).len()))])), ("compatibility",match (&(item).compatibility).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("current_host_version",mp::text(&(item).current_host_version)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?)]), None => serde_json::Value::Null }), ("mcp_instances",mp::object_value(&[("item_count",serde_json::json!((&(item).mcp_instances).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::CatalogEntry(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("CatalogEntry".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("id",mp::text(&(_field_0).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("artifact",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact).len()))])), ("version",mp::text(&(_field_0).version)?), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).description).len()))])), ("host_version_requirement",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).host_version_requirement).len()))])), ("slot_codes",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).slot_codes).len()))])), ("keywords",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).keywords).len()))])), ("source",mp::json_summary(&(_field_0).source)), ("signature",match (&(_field_0).signature).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("checksum",match (&(_field_0).checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("download_locator",mp::json_summary(&(_field_0).download_locator)), ("catalog_page",serde_json::json!(*(&(_field_0).catalog_page))), ("catalog_source",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).catalog_source).len()))])), ("current_version",match (&(_field_0).current_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).installation_status).len()))])), ("artifact_kind",match (&(_field_0).artifact_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("installation_source",match (&(_field_0).installation_source).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("extension_installation_id",match (&(_field_0).extension_installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("builtin_template_id",match (&(_field_0).builtin_template_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("trust",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).trust).len()))])), ("warnings",{ if (&(_field_0).warnings).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).warnings).iter().map(|item| Some(mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("overridable",serde_json::Value::Bool(*(&(item).overridable)))]))).collect::<Option<Vec<_>>>()?) }), ("compatibility",match (&(_field_0).compatibility).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("current_host_version",mp::text(&(item).current_host_version)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?)]), None => serde_json::Value::Null }), ("mcp_instances",{ if (&(_field_0).mcp_instances).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).mcp_instances).iter().map(|item| Some(mp::object_value(&[("instance_id",mp::text(&(item).instance_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("description_short",match (&(item).description_short).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("workspace_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).workspace_status).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Updates(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Updates".to_owned())), ("0",mp::object_value(&[("category",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).category).len()))])), ("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("catalog_id",mp::text(&(item).catalog_id)?), ("current_version",mp::text(&(item).current_version)?), ("latest_version",match (&(item).latest_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?)]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Install(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Install".to_owned())), ("0",match _field_0 {crate::routes::plugins_and_models_group::plugins::extension_center::ExtensionInstallOutcome::Existing(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Existing".to_owned())), ("0",mp::object_value(&[("installation",mp::object_value(&[("contract_version",match (&(&(_field_0).installation).contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("id",mp::text(&(&(_field_0).installation).id)?), ("catalog_id",mp::text(&(&(_field_0).installation).catalog_id)?), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).category).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).organization).len()))])), ("artifact_id",mp::text(&(&(_field_0).installation).artifact_id)?), ("version",mp::text(&(&(_field_0).installation).version)?), ("node_id",mp::text(&(&(_field_0).installation).node_id)?), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).trust_level).len()))])), ("warnings",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).installation).warnings).len()))])), ("expected_checksum",match (&(&(_field_0).installation).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_checksum",match (&(&(_field_0).installation).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).signature_status).len()))])), ("signature_algorithm",match (&(&(_field_0).installation).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(_field_0).installation).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(&(_field_0).installation).status)?), ("is_current",serde_json::Value::Bool(*(&(&(_field_0).installation).is_current))), ("runtime_status",match (&(&(_field_0).installation).runtime_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }),
("desired_state",match (&(&(_field_0).installation).desired_state).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("availability_status",match (&(&(_field_0).installation).availability_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).application_status).len()))])), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).created_by).len()))])), ("created_at",mp::text(&(&(_field_0).installation).created_at)?), ("updated_at",mp::text(&(&(_field_0).installation).updated_at)?), ("installed_versions",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).installation).installed_versions).len()))]))])), ("local_artifact_was_present",serde_json::Value::Bool(*(&(_field_0).local_artifact_was_present))), ("node_plugin_installation_id",match (&(_field_0).node_plugin_installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_status).len()))])), ("managed_schema_preview",match (&(_field_0).managed_schema_preview).as_ref() { Some(item) => mp::object_value(&[("owner_id",mp::text(&(item).owner_id)?), ("fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).fingerprint).len()))])), ("entries",mp::object_value(&[("item_count",serde_json::json!((&(item).entries).len()))]))]), None => serde_json::Value::Null }), ("managed_schema_receipt",match (&(_field_0).managed_schema_receipt).as_ref() { Some(item) => mp::object_value(&[("receipt_id",mp::text(&(item).receipt_id)?), ("owner_id",mp::text(&(item).owner_id)?), ("owner_version",mp::text(&(item).owner_version)?), ("fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).fingerprint).len()))])), ("created_objects",serde_json::json!(*(&(item).created_objects))), ("existing_objects",serde_json::json!(*(&(item).existing_objects))), ("retained_objects",serde_json::json!(*(&(item).retained_objects))), ("applied_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).applied_at).len()))]))]), None => serde_json::Value::Null })]))]), crate::routes::plugins_and_models_group::plugins::extension_center::ExtensionInstallOutcome::Created(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned())), ("0",mp::object_value(&[("installation",mp::object_value(&[("contract_version",match (&(&(_field_0).installation).contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("id",mp::text(&(&(_field_0).installation).id)?), ("catalog_id",mp::text(&(&(_field_0).installation).catalog_id)?), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).category).len()))])), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).organization).len()))])), ("artifact_id",mp::text(&(&(_field_0).installation).artifact_id)?), ("version",mp::text(&(&(_field_0).installation).version)?), ("node_id",mp::text(&(&(_field_0).installation).node_id)?), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).trust_level).len()))])), ("warnings",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).installation).warnings).len()))])), ("expected_checksum",match (&(&(_field_0).installation).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_checksum",match (&(&(_field_0).installation).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).signature_status).len()))])), ("signature_algorithm",match (&(&(_field_0).installation).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(_field_0).installation).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("status",mp::text(&(&(_field_0).installation).status)?), ("is_current",serde_json::Value::Bool(*(&(&(_field_0).installation).is_current))), ("runtime_status",match (&(&(_field_0).installation).runtime_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }),
("desired_state",match (&(&(_field_0).installation).desired_state).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("availability_status",match (&(&(_field_0).installation).availability_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).application_status).len()))])), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).created_by).len()))])), ("created_at",mp::text(&(&(_field_0).installation).created_at)?), ("updated_at",mp::text(&(&(_field_0).installation).updated_at)?), ("installed_versions",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).installation).installed_versions).len()))]))])), ("local_artifact_was_present",serde_json::Value::Bool(*(&(_field_0).local_artifact_was_present))), ("node_plugin_installation_id",match (&(_field_0).node_plugin_installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("application_action",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_action).len()))])), ("application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).application_status).len()))])), ("managed_schema_preview",match (&(_field_0).managed_schema_preview).as_ref() { Some(item) => mp::object_value(&[("owner_id",mp::text(&(item).owner_id)?), ("fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).fingerprint).len()))])), ("entries",mp::object_value(&[("item_count",serde_json::json!((&(item).entries).len()))]))]), None => serde_json::Value::Null }), ("managed_schema_receipt",match (&(_field_0).managed_schema_receipt).as_ref() { Some(item) => mp::object_value(&[("receipt_id",mp::text(&(item).receipt_id)?), ("owner_id",mp::text(&(item).owner_id)?), ("owner_version",mp::text(&(item).owner_version)?), ("fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).fingerprint).len()))])), ("created_objects",serde_json::json!(*(&(item).created_objects))), ("existing_objects",serde_json::json!(*(&(item).existing_objects))), ("retained_objects",serde_json::json!(*(&(item).retained_objects))), ("applied_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).applied_at).len()))]))]), None => serde_json::Value::Null })]))]), crate::routes::plugins_and_models_group::plugins::extension_center::ExtensionInstallOutcome::Challenge(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Challenge".to_owned())), ("0",mp::object_value(&[("warnings",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).warnings).len()))])), ("compatibility",match (&(_field_0).compatibility).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("current_host_version",mp::text(&(item).current_host_version)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?)]), None => serde_json::Value::Null })]))])})])})
    }

    const CONTRACT_ID: &'static str = "console-extension-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}
