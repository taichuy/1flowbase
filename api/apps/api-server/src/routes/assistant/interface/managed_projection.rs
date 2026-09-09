use super::*;

impl InterfaceContract for AssistantSettingsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Get"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "application_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "mcp_instance_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "model",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reasoning_effort",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "enabled_client_tools",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("GetClientContext"))]), mp::object_schema(&[("variant",mp::tag_schema("RefreshClientView"))]), mp::object_schema(&[("variant",mp::tag_schema("ListPageBlocks"))]), mp::object_schema(&[("variant",mp::tag_schema("InspectBlockRender"))]), mp::object_schema(&[("variant",mp::tag_schema("SearchBlockRender"))]), mp::object_schema(&[("variant",mp::tag_schema("ReadBlockRenderFragment"))]), mp::object_schema(&[("variant",mp::tag_schema("ClickBlockElement"))]), mp::object_schema(&[("variant",mp::tag_schema("RecompileBlock"))])])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Get => {
                mp::object_value(&[("variant", serde_json::Value::String("Get".to_owned()))])
            }
            Self::Update(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Update".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "application_id",
                            match (&(_field_0).application_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("mcp_instance_ids", {
                            if (&(_field_0).mcp_instance_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).mcp_instance_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "model",
                            match (&(_field_0).model).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reasoning_effort",
                            match (&(_field_0).reasoning_effort).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("enabled_client_tools", {
                            if (&(_field_0).enabled_client_tools).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).enabled_client_tools).iter().map(|item| Some(match item {crate::routes::assistant::AssistantClientToolId::GetClientContext => mp::object_value(&[("variant",serde_json::Value::String("GetClientContext".to_owned()))]), crate::routes::assistant::AssistantClientToolId::RefreshClientView => mp::object_value(&[("variant",serde_json::Value::String("RefreshClientView".to_owned()))]), crate::routes::assistant::AssistantClientToolId::ListPageBlocks => mp::object_value(&[("variant",serde_json::Value::String("ListPageBlocks".to_owned()))]), crate::routes::assistant::AssistantClientToolId::InspectBlockRender => mp::object_value(&[("variant",serde_json::Value::String("InspectBlockRender".to_owned()))]), crate::routes::assistant::AssistantClientToolId::SearchBlockRender => mp::object_value(&[("variant",serde_json::Value::String("SearchBlockRender".to_owned()))]), crate::routes::assistant::AssistantClientToolId::ReadBlockRenderFragment => mp::object_value(&[("variant",serde_json::Value::String("ReadBlockRenderFragment".to_owned()))]), crate::routes::assistant::AssistantClientToolId::ClickBlockElement => mp::object_value(&[("variant",serde_json::Value::String("ClickBlockElement".to_owned()))]), crate::routes::assistant::AssistantClientToolId::RecompileBlock => mp::object_value(&[("variant",serde_json::Value::String("RecompileBlock".to_owned()))])})).collect::<Option<Vec<_>>>()?)
                        }),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-settings-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantSettingsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![mp::object_schema(&[
            ("variant", mp::tag_schema("Settings")),
            (
                "0",
                mp::object_schema(&[
                    (
                        "preference",
                        mp::object_schema(&[
                            (
                                "application_id",
                                serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                            ),
                            (
                                "mcp_instance_ids",
                                serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                            ),
                            (
                                "model",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "reasoning_effort",
                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                            ),
                            (
                                "enabled_client_tools",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                        ]),
                    ),
                    (
                        "published_agent_flows",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("application_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    ),
                    (
                        "enabled_mcp_instances",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("instance_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    ),
                    (
                        "page_reference_max_bytes",
                        serde_json::json!({"type":"integer"}),
                    ),
                    (
                        "page_reference_max_count",
                        serde_json::json!({"type":"integer"}),
                    ),
                    (
                        "page_reference_max_total_bytes",
                        serde_json::json!({"type":"integer"}),
                    ),
                    (
                        "run_capabilities",
                        mp::object_schema(&[
                            (
                                "model_selection_enabled",
                                serde_json::json!({"type":"boolean"}),
                            ),
                            (
                                "reasoning_effort_enabled",
                                serde_json::json!({"type":"boolean"}),
                            ),
                            (
                                "models",
                                mp::object_schema(&[("item_count", mp::count_schema())]),
                            ),
                        ]),
                    ),
                ]),
            ),
        ])]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Settings(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Settings".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "preference",
                            mp::object_value(&[
                                (
                                    "application_id",
                                    match (&(&(_field_0).preference).application_id).as_ref() {
                                        Some(item) => serde_json::Value::String((item).to_string()),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                ("mcp_instance_ids", {
                                    if (&(&(_field_0).preference).mcp_instance_ids).len() > 32 {
                                        return None;
                                    }
                                    serde_json::Value::Array(
                                        (&(&(_field_0).preference).mcp_instance_ids)
                                            .iter()
                                            .map(|item| Some(mp::text(item)?))
                                            .collect::<Option<Vec<_>>>()?,
                                    )
                                }),
                                (
                                    "model",
                                    match (&(&(_field_0).preference).model).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "reasoning_effort",
                                    match (&(&(_field_0).preference).reasoning_effort).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "enabled_client_tools",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).preference).enabled_client_tools).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                        ("published_agent_flows", {
                            if (&(_field_0).published_agent_flows).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).published_agent_flows)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "application_id",
                                                serde_json::Value::String(
                                                    (&(item).application_id).to_string(),
                                                ),
                                            ),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled_mcp_instances", {
                            if (&(_field_0).enabled_mcp_instances).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).enabled_mcp_instances)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("instance_id", mp::text(&(item).instance_id)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "page_reference_max_bytes",
                            serde_json::json!(*(&(_field_0).page_reference_max_bytes)),
                        ),
                        (
                            "page_reference_max_count",
                            serde_json::json!(*(&(_field_0).page_reference_max_count)),
                        ),
                        (
                            "page_reference_max_total_bytes",
                            serde_json::json!(*(&(_field_0).page_reference_max_total_bytes)),
                        ),
                        (
                            "run_capabilities",
                            mp::object_value(&[
                                (
                                    "model_selection_enabled",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).run_capabilities).model_selection_enabled),
                                    ),
                                ),
                                (
                                    "reasoning_effort_enabled",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).run_capabilities).reasoning_effort_enabled),
                                    ),
                                ),
                                (
                                    "models",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).run_capabilities).models).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-settings-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantConversationsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetRunActivity")),
                ("flow_run_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        (
                            "after_sequence",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateConversation")),
                (
                    "0",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        (
                            "seed_legacy_flow_run_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListConversations")),
                (
                    "0",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        (
                            "page",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetConversationMessages")),
                ("conversation_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[("application_id", mp::text_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetLegacySnapshotMessages")),
                ("flow_run_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[("application_id", mp::text_schema())]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::GetRunActivity {
                flow_run_id: _field_flow_run_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetRunActivity".to_owned()),
                ),
                (
                    "flow_run_id",
                    serde_json::Value::String((_field_flow_run_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "application_id",
                            serde_json::Value::String((&(_field_query).application_id).to_string()),
                        ),
                        (
                            "after_sequence",
                            match (&(_field_query).after_sequence).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page_size",
                            match (&(_field_query).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::CreateConversation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateConversation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "application_id",
                            serde_json::Value::String((&(_field_0).application_id).to_string()),
                        ),
                        (
                            "seed_legacy_flow_run_id",
                            match (&(_field_0).seed_legacy_flow_run_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ListConversations(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListConversations".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "application_id",
                            serde_json::Value::String((&(_field_0).application_id).to_string()),
                        ),
                        (
                            "page",
                            match (&(_field_0).page).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page_size",
                            match (&(_field_0).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetConversationMessages {
                conversation_id: _field_conversation_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetConversationMessages".to_owned()),
                ),
                (
                    "conversation_id",
                    serde_json::Value::String((_field_conversation_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[(
                        "application_id",
                        serde_json::Value::String((&(_field_query).application_id).to_string()),
                    )]),
                ),
            ]),
            Self::GetLegacySnapshotMessages {
                flow_run_id: _field_flow_run_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetLegacySnapshotMessages".to_owned()),
                ),
                (
                    "flow_run_id",
                    serde_json::Value::String((_field_flow_run_id).to_string()),
                ),
                (
                    "query",
                    mp::object_value(&[(
                        "application_id",
                        serde_json::Value::String((&(_field_query).application_id).to_string()),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-conversations-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantConversationsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("RunActivity")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", mp::text_schema()),
                        (
                            "started_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "finished_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "duration_ms",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Reasoning")), ("event_id",mp::text_schema()), ("sequence_start",serde_json::json!({"type":"integer"})), ("sequence_end",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("text",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("Output")), ("event_id",mp::text_schema()), ("sequence_start",serde_json::json!({"type":"integer"})), ("sequence_end",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("text",mp::object_schema(&[("byte_count",mp::count_schema())])), ("segment_index",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))]), mp::object_schema(&[("variant",mp::tag_schema("Tool")), ("event_id",mp::text_schema()), ("sequence_start",serde_json::json!({"type":"integer"})), ("sequence_end",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("tool_call_id",mp::text_schema()), ("tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("input",mp::json_summary_schema()), ("output",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("is_error",serde_json::json!({"type":"boolean"})), ("status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Running"))]), mp::object_schema(&[("variant",mp::tag_schema("Succeeded"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])]))]), mp::object_schema(&[("variant",mp::tag_schema("Error")), ("event_id",mp::text_schema()), ("sequence_start",serde_json::json!({"type":"integer"})), ("sequence_end",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("error",mp::object_schema(&[("byte_count",mp::count_schema())]))])])}),
                        ),
                        (
                            "trace_events",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("event_id",mp::text_schema()), ("run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("event_type",mp::text_schema()), ("sequence",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("payload",mp::json_summary_schema()), ("delta_index",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("content_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("text",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                        (
                            "next_sequence",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Conversation")),
                (
                    "0",
                    mp::object_schema(&[
                        ("conversation_id", mp::text_schema()),
                        ("application_id", mp::text_schema()),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ConversationPage")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("legacy_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("latest_flow_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("latest_flow_run_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ConversationMessages")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("role",mp::object_schema(&[("byte_count",mp::count_schema())])), ("content",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("page_references",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("page_title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("outer_html",mp::object_schema(&[("byte_count",mp::count_schema())]))])})), ("created_at",mp::text_schema())])}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::RunActivity(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RunActivity".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "started_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).started_at).len()),
                            )]),
                        ),
                        (
                            "finished_at",
                            match (&(_field_0).finished_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "duration_ms",
                            match (&(_field_0).duration_ms).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(match item {crate::routes::assistant::run_activity::AssistantRunActivityItem::Reasoning {event_id: _field_event_id, sequence_start: _field_sequence_start, sequence_end: _field_sequence_end, created_at: _field_created_at, text: _field_text, .. } => mp::object_value(&[("variant",serde_json::Value::String("Reasoning".to_owned())), ("event_id",mp::text(_field_event_id)?), ("sequence_start",serde_json::json!(*(_field_sequence_start))), ("sequence_end",serde_json::json!(*(_field_sequence_end))), ("created_at",mp::text(_field_created_at)?), ("text",mp::object_value(&[("byte_count",serde_json::json!((_field_text).len()))]))]), crate::routes::assistant::run_activity::AssistantRunActivityItem::Output {event_id: _field_event_id, sequence_start: _field_sequence_start, sequence_end: _field_sequence_end, created_at: _field_created_at, text: _field_text, segment_index: _field_segment_index, .. } => mp::object_value(&[("variant",serde_json::Value::String("Output".to_owned())), ("event_id",mp::text(_field_event_id)?), ("sequence_start",serde_json::json!(*(_field_sequence_start))), ("sequence_end",serde_json::json!(*(_field_sequence_end))), ("created_at",mp::text(_field_created_at)?), ("text",mp::object_value(&[("byte_count",serde_json::json!((_field_text).len()))])), ("segment_index",match (_field_segment_index).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]), crate::routes::assistant::run_activity::AssistantRunActivityItem::Tool {event_id: _field_event_id, sequence_start: _field_sequence_start, sequence_end: _field_sequence_end, created_at: _field_created_at, tool_call_id: _field_tool_call_id, tool_name: _field_tool_name, input: _field_input, output: _field_output, duration_ms: _field_duration_ms, is_error: _field_is_error, status: _field_status, .. } => mp::object_value(&[("variant",serde_json::Value::String("Tool".to_owned())), ("event_id",mp::text(_field_event_id)?), ("sequence_start",serde_json::json!(*(_field_sequence_start))), ("sequence_end",serde_json::json!(*(_field_sequence_end))), ("created_at",mp::text(_field_created_at)?), ("tool_call_id",mp::text(_field_tool_call_id)?), ("tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_tool_name).len()))])), ("input",mp::json_summary(_field_input)), ("output",match (_field_output).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("duration_ms",match (_field_duration_ms).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("is_error",serde_json::Value::Bool(*(_field_is_error))), ("status",match _field_status {crate::routes::assistant::run_activity::AssistantRunToolStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), crate::routes::assistant::run_activity::AssistantRunToolStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), crate::routes::assistant::run_activity::AssistantRunToolStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])})]), crate::routes::assistant::run_activity::AssistantRunActivityItem::Error {event_id: _field_event_id, sequence_start: _field_sequence_start, sequence_end: _field_sequence_end, created_at: _field_created_at, error: _field_error, .. } => mp::object_value(&[("variant",serde_json::Value::String("Error".to_owned())), ("event_id",mp::text(_field_event_id)?), ("sequence_start",serde_json::json!(*(_field_sequence_start))), ("sequence_end",serde_json::json!(*(_field_sequence_end))), ("created_at",mp::text(_field_created_at)?), ("error",mp::object_value(&[("byte_count",serde_json::json!((_field_error).len()))]))])})).collect::<Option<Vec<_>>>()?)
                        }),
                        ("trace_events", {
                            if (&(_field_0).trace_events).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).trace_events)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("event_id", mp::text(&(item).event_id)?),
                                            ("run_id", mp::text(&(item).run_id)?),
                                            (
                                                "node_run_id",
                                                match (&(item).node_run_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("event_type", mp::text(&(item).event_type)?),
                                            ("sequence", serde_json::json!(*(&(item).sequence))),
                                            ("created_at", mp::text(&(item).created_at)?),
                                            ("payload", mp::json_summary(&(item).payload)),
                                            (
                                                "delta_index",
                                                match (&(item).delta_index).as_ref() {
                                                    Some(item) => serde_json::json!(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "content_type",
                                                match (&(item).content_type).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "text",
                                                match (&(item).text).as_ref() {
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
                        ("has_more", serde_json::Value::Bool(*(&(_field_0).has_more))),
                        (
                            "next_sequence",
                            match (&(_field_0).next_sequence).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Conversation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Conversation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "conversation_id",
                            serde_json::Value::String((&(_field_0).conversation_id).to_string()),
                        ),
                        (
                            "application_id",
                            serde_json::Value::String((&(_field_0).application_id).to_string()),
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::ConversationPage(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ConversationPage".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "conversation_id",
                                                match (&(item).conversation_id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "legacy_flow_run_id",
                                                match (&(item).legacy_flow_run_id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "latest_flow_run_id",
                                                match (&(item).latest_flow_run_id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "latest_flow_run_status",
                                                match (&(item).latest_flow_run_status).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
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
                                            ("created_at", mp::text(&(item).created_at)?),
                                            ("updated_at", mp::text(&(item).updated_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total", serde_json::json!(*(&(_field_0).total))),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                    ]),
                ),
            ]),
            Self::ConversationMessages(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ConversationMessages".to_owned()),
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
                                    ("id", mp::text(&(item).id)?),
                                    (
                                        "flow_run_id",
                                        serde_json::Value::String(
                                            (&(item).flow_run_id).to_string(),
                                        ),
                                    ),
                                    (
                                        "role",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).role).len()),
                                        )]),
                                    ),
                                    (
                                        "content",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).content).len()),
                                        )]),
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    ("page_references", {
                                        if (&(item).page_references).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).page_references)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        (
                                                            "page_title",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).page_title).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "outer_html",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).outer_html).len()
                                                                ),
                                                            )]),
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    ("created_at", mp::text(&(item).created_at)?),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-assistant-conversations-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantRunInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "body",
            mp::object_schema(&[
                ("application_id", mp::text_schema()),
                (
                    "conversation_id",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
                (
                    "query",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "page_references",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("page_title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("outer_html",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
                (
                    "title",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "body",
            mp::object_value(&[
                (
                    "application_id",
                    serde_json::Value::String((&(&(self).body).application_id).to_string()),
                ),
                (
                    "conversation_id",
                    match (&(&(self).body).conversation_id).as_ref() {
                        Some(item) => serde_json::Value::String((item).to_string()),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "query",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).body).query).len()),
                    )]),
                ),
                ("page_references", {
                    if (&(&(self).body).page_references).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (&(&(self).body).page_references)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    (
                                        "page_title",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).page_title).len()),
                                        )]),
                                    ),
                                    (
                                        "outer_html",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).outer_html).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
                (
                    "title",
                    match (&(&(self).body).title).as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-assistant-run-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantRunOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("conversation_id", mp::text_schema()),
                ("status", mp::text_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                ("output_payload", mp::json_summary_schema()),
                (
                    "error_payload",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                (
                    "id",
                    serde_json::Value::String((&(&(self).0).id).to_string()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((&(&(self).0).application_id).to_string()),
                ),
                (
                    "conversation_id",
                    serde_json::Value::String((&(&(self).0).conversation_id).to_string()),
                ),
                ("status", mp::text(&(&(self).0).status)?),
                (
                    "answer",
                    match (&(&(self).0).answer).as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "output_payload",
                    mp::json_summary(&(&(self).0).output_payload),
                ),
                (
                    "error_payload",
                    match (&(&(self).0).error_payload).as_ref() {
                        Some(item) => mp::json_summary(item),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-assistant-run-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantRunStreamEvent {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[("is_ok", serde_json::json!({"type":"boolean"}))]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[("is_ok", serde_json::Value::Bool((&(self).0).is_ok()))]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-assistant-run-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for AssistantRunStreamOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("AssistantRunStreamOutput"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("AssistantRunStreamOutput".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-assistant-run-stream-output";
    const CONTRACT_VERSION: &'static str = "1";
}
