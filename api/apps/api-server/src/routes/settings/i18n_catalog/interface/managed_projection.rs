use super::*;

impl InterfaceContract for I18nCatalogInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("GetState"))]),
            mp::object_schema(&[("variant", mp::tag_schema("CheckUpdate"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ActivateOfficial")),
                (
                    "0",
                    mp::object_schema(&[(
                        "expected_revision",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewInstalled")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ActivateInstalled")),
                ("installation_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                        (
                            "integrity_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListEntries")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "locale",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "search",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetEntry")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("locale", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpsertOfficialOverride")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("locale", mp::text_schema()),
                        (
                            "translation",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RestoreOfficialOverride")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("locale", mp::text_schema()),
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpsertCustomTranslation")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("locale", mp::text_schema()),
                        (
                            "translation",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteCustomKey")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RestoreAllOfficialOverrides")),
                (
                    "0",
                    mp::object_schema(&[(
                        "expected_revision",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::GetState => {
                mp::object_value(&[("variant", serde_json::Value::String("GetState".to_owned()))])
            }
            Self::CheckUpdate => mp::object_value(&[(
                "variant",
                serde_json::Value::String("CheckUpdate".to_owned()),
            )]),
            Self::ActivateOfficial(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ActivateOfficial".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "expected_revision",
                        serde_json::json!(*(&(_field_0).expected_revision)),
                    )]),
                ),
            ]),
            Self::PreviewInstalled {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PreviewInstalled".to_owned()),
                ),
                (
                    "installation_id",
                    serde_json::Value::String((_field_installation_id).to_string()),
                ),
            ]),
            Self::ActivateInstalled {
                installation_id: _field_installation_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ActivateInstalled".to_owned()),
                ),
                (
                    "installation_id",
                    serde_json::Value::String((_field_installation_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_body).expected_revision)),
                        ),
                        (
                            "integrity_override",
                            match (&(_field_body).integrity_override).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reason).len()),
                                        )]),
                                    ),
                                    (
                                        "acknowledged_warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).acknowledged_warnings).len()),
                                        )]),
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ListEntries(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListEntries".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            match (&(_field_0).key).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (&(_field_0).locale).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "search",
                            match (&(_field_0).search).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_0).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
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
            Self::GetEntry(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("GetEntry".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        ("locale", mp::text(&(_field_0).locale)?),
                    ]),
                ),
            ]),
            Self::UpsertOfficialOverride(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpsertOfficialOverride".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        ("locale", mp::text(&(_field_0).locale)?),
                        (
                            "translation",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).translation).len()),
                            )]),
                        ),
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_0).expected_revision)),
                        ),
                    ]),
                ),
            ]),
            Self::RestoreOfficialOverride(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RestoreOfficialOverride".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        ("locale", mp::text(&(_field_0).locale)?),
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_0).expected_revision)),
                        ),
                    ]),
                ),
            ]),
            Self::UpsertCustomTranslation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpsertCustomTranslation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        ("locale", mp::text(&(_field_0).locale)?),
                        (
                            "translation",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).translation).len()),
                            )]),
                        ),
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_0).expected_revision)),
                        ),
                    ]),
                ),
            ]),
            Self::DeleteCustomKey(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteCustomKey".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        (
                            "expected_revision",
                            serde_json::json!(*(&(_field_0).expected_revision)),
                        ),
                    ]),
                ),
            ]),
            Self::RestoreAllOfficialOverrides(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RestoreAllOfficialOverrides".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "expected_revision",
                        serde_json::json!(*(&(_field_0).expected_revision)),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-i18n-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for I18nCatalogOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("State")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "active_catalog_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("revision", serde_json::json!({"type":"integer"})),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_locale",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "locales",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateStatus")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", mp::text_schema()),
                        (
                            "active_catalog_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("latest_catalog_version", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Activation")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", mp::text_schema()),
                        ("catalog_version", mp::text_schema()),
                        ("revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstalledPreview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("extension_installation_id", mp::text_schema()),
                        (
                            "application_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "active_catalog_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("installed_catalog_version", mp::text_schema()),
                        ("revision", serde_json::json!({"type":"integer"})),
                        (
                            "integrity_warnings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("overridable",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "required_integrity_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("warnings",mp::object_schema(&[("item_count",mp::count_schema())])), ("compatibility",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]}))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Entries")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("locale",mp::text_schema()), ("official_translation",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("override_translation",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("custom_translation",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("effective_value",mp::object_schema(&[("byte_count",mp::count_schema())])), ("missing",serde_json::json!({"type":"boolean"})), ("obsolete",serde_json::json!({"type":"boolean"})), ("revision",serde_json::json!({"type":"integer"}))])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Entry")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("locale", mp::text_schema()),
                        (
                            "official_translation",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "override_translation",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "custom_translation",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "effective_value",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("missing", serde_json::json!({"type":"boolean"})),
                        ("obsolete", serde_json::json!({"type":"boolean"})),
                        ("revision", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("EntryMutation")),
                (
                    "0",
                    mp::object_schema(&[
                        ("revision", serde_json::json!({"type":"integer"})),
                        (
                            "entry",
                            mp::object_schema(&[
                                (
                                    "key",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("locale", mp::text_schema()),
                                (
                                    "official_translation",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "override_translation",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "custom_translation",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "effective_value",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("missing", serde_json::json!({"type":"boolean"})),
                                ("obsolete", serde_json::json!({"type":"boolean"})),
                                ("revision", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Revision")),
                (
                    "0",
                    mp::object_schema(&[("revision", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::State(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("State".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "active_catalog_version",
                            match (&(_field_0).active_catalog_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source).len()),
                            )]),
                        ),
                        (
                            "source_locale",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_locale).len()),
                            )]),
                        ),
                        (
                            "locales",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).locales).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateStatus(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateStatus".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "active_catalog_version",
                            match (&(_field_0).active_catalog_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "latest_catalog_version",
                            mp::text(&(_field_0).latest_catalog_version)?,
                        ),
                    ]),
                ),
            ]),
            Self::Activation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Activation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("status", mp::text(&(_field_0).status)?),
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                    ]),
                ),
            ]),
            Self::InstalledPreview(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstalledPreview".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "extension_installation_id",
                            mp::text(&(_field_0).extension_installation_id)?,
                        ),
                        (
                            "application_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).application_status).len()),
                            )]),
                        ),
                        (
                            "active_catalog_version",
                            match (&(_field_0).active_catalog_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "installed_catalog_version",
                            mp::text(&(_field_0).installed_catalog_version)?,
                        ),
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                        ("integrity_warnings", {
                            if (&(_field_0).integrity_warnings).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).integrity_warnings)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("code", mp::text(&(item).code)?),
                                            (
                                                "message",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).message).len()),
                                                )]),
                                            ),
                                            (
                                                "overridable",
                                                serde_json::Value::Bool(*(&(item).overridable)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "required_integrity_override",
                            match (&(_field_0).required_integrity_override).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).warnings).len()),
                                        )]),
                                    ),
                                    (
                                        "compatibility",
                                        match (&(item).compatibility).as_ref() {
                                            Some(item) => mp::object_value(&[
                                                (
                                                    "reason",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((&(item).reason).len()),
                                                    )]),
                                                ),
                                                (
                                                    "current_host_version",
                                                    mp::text(&(item).current_host_version)?,
                                                ),
                                                (
                                                    "minimum_host_version",
                                                    mp::text(&(item).minimum_host_version)?,
                                                ),
                                            ]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Entries(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Entries".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("entries", {
                            if (&(_field_0).entries).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).entries)
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
                                            ("locale", mp::text(&(item).locale)?),
                                            (
                                                "official_translation",
                                                match (&(item).official_translation).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "override_translation",
                                                match (&(item).override_translation).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "custom_translation",
                                                match (&(item).custom_translation).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "effective_value",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).effective_value).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "missing",
                                                serde_json::Value::Bool(*(&(item).missing)),
                                            ),
                                            (
                                                "obsolete",
                                                serde_json::Value::Bool(*(&(item).obsolete)),
                                            ),
                                            ("revision", serde_json::json!(*(&(item).revision))),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total", serde_json::json!(*(&(_field_0).total))),
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                    ]),
                ),
            ]),
            Self::Entry(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Entry".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        ("locale", mp::text(&(_field_0).locale)?),
                        (
                            "official_translation",
                            match (&(_field_0).official_translation).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "override_translation",
                            match (&(_field_0).override_translation).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "custom_translation",
                            match (&(_field_0).custom_translation).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "effective_value",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).effective_value).len()),
                            )]),
                        ),
                        ("missing", serde_json::Value::Bool(*(&(_field_0).missing))),
                        ("obsolete", serde_json::Value::Bool(*(&(_field_0).obsolete))),
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                    ]),
                ),
            ]),
            Self::EntryMutation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("EntryMutation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("revision", serde_json::json!(*(&(_field_0).revision))),
                        (
                            "entry",
                            mp::object_value(&[
                                (
                                    "key",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).entry).key).len()),
                                    )]),
                                ),
                                ("locale", mp::text(&(&(_field_0).entry).locale)?),
                                (
                                    "official_translation",
                                    match (&(&(_field_0).entry).official_translation).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "override_translation",
                                    match (&(&(_field_0).entry).override_translation).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "custom_translation",
                                    match (&(&(_field_0).entry).custom_translation).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "effective_value",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).entry).effective_value).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "missing",
                                    serde_json::Value::Bool(*(&(&(_field_0).entry).missing)),
                                ),
                                (
                                    "obsolete",
                                    serde_json::Value::Bool(*(&(&(_field_0).entry).obsolete)),
                                ),
                                (
                                    "revision",
                                    serde_json::json!(*(&(&(_field_0).entry).revision)),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            Self::Revision(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Revision".to_owned())),
                (
                    "0",
                    mp::object_value(&[("revision", serde_json::json!(*(&(_field_0).revision)))]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-i18n-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}
