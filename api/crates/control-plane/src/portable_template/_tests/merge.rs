use super::*;
use serde_json::{json, Value};
use uuid::Uuid;

fn key() -> TemplateResourceKey {
    TemplateResourceKey {
        kind: "block".into(),
        source_id: "source".into(),
    }
}
fn baseline(applied: Option<&str>) -> TemplateResourceBaseline {
    TemplateResourceBaseline {
        key: key(),
        target_id: "target".into(),
        generation: 3,
        applied_fingerprint: applied.map(str::to_owned),
        pending: None,
        committed_operation_id: None,
        committed_fingerprint: None,
    }
}
fn projected_item(
    kind: &str,
    source: &str,
    target: &str,
    content: &str,
) -> ProjectedTemplateResource {
    let value = json!({"content":content});
    ProjectedTemplateResource {
        key: TemplateResourceKey {
            kind: kind.into(),
            source_id: source.into(),
        },
        target_id: target.into(),
        fingerprint: template_resource_fingerprint(&value).unwrap(),
        value,
    }
}
fn package() -> PortableTemplatePackage {
    serde_json::from_value(json!({
        "schema_version":PORTABLE_TEMPLATE_SCHEMA_VERSION,
        "pages":[],"applications":[],"data_models":[]
    }))
    .unwrap()
}

#[test]
fn decision_matrix_never_adopts_unknown_or_modified_content() {
    use TemplateMergeDecision::*;
    let known = baseline(Some("old"));
    let unknown = baseline(None);
    for (current, desired, history, tracked, expected) in [
        (None, "new", None, false, Initialize),
        (Some("new"), "new", None, false, SkipUnknownBaseline),
        (
            Some("new"),
            "new",
            Some(&unknown),
            false,
            SkipUnknownBaseline,
        ),
        (Some("old"), "new", Some(&known), true, Update),
        (Some("old"), "old", Some(&known), true, Unchanged),
        (Some("local"), "new", Some(&known), true, SkipUserModified),
        // A user may independently make the exact same edit as the next release.
        // Equality with desired must not silently advance the last-applied baseline.
        (Some("new"), "new", Some(&known), true, SkipUserModified),
        (None, "new", Some(&known), false, SkipUserDeleted),
        (None, "new", None, true, SkipUserDeleted),
    ] {
        assert_eq!(
            decide_template_resource(current, desired, history, tracked),
            expected
        );
    }
}

#[test]
fn interrupted_write_requires_atomic_owner_receipt_even_if_content_matches() {
    let intent = TemplateWriteIntent {
        operation_id: Uuid::from_u128(10),
        target_id: "target".into(),
        expected_fingerprint: Some("old".into()),
        desired_fingerprint: "new".into(),
    };
    let mut history = baseline(Some("old"));
    history.pending = Some(intent.clone());
    for current in [Some("old"), Some("new"), Some("tampered"), None] {
        assert_eq!(
            decide_template_resource(current, "new", Some(&history), true),
            TemplateMergeDecision::SkipPendingWrite
        );
    }
    history.committed_operation_id = Some(Uuid::from_u128(99));
    assert_eq!(
        decide_template_resource(Some("new"), "new", Some(&history), true),
        TemplateMergeDecision::SkipPendingWrite
    );
    history.committed_operation_id = Some(intent.operation_id);
    history.committed_fingerprint = Some("new".into());
    assert_eq!(
        decide_template_resource(Some("tampered"), "new", Some(&history), true),
        TemplateMergeDecision::RecoverCommitted
    );
    // After receipt finalization, edits made after the owner's commit remain local edits.
    history.pending = None;
    history.committed_operation_id = None;
    history.committed_fingerprint = None;
    history.applied_fingerprint = Some("new".into());
    assert_eq!(
        decide_template_resource(Some("tampered"), "next", Some(&history), true),
        TemplateMergeDecision::SkipUserModified
    );
}

#[test]
fn write_intent_enforces_preimage_and_absence_not_desired_equality() {
    let intent = TemplateWriteIntent {
        operation_id: Uuid::nil(),
        target_id: "target".into(),
        expected_fingerprint: Some("old".into()),
        desired_fingerprint: "new".into(),
    };
    assert!(template_intent_matches_current(&intent, Some("old")));
    assert!(!template_intent_matches_current(&intent, Some("new")));
    assert!(!template_intent_matches_current(
        &intent,
        Some("concurrent")
    ));
    assert!(!template_intent_matches_current(&intent, None));
    let create = TemplateWriteIntent {
        expected_fingerprint: None,
        ..intent
    };
    assert!(template_intent_matches_current(&create, None));
    assert!(!template_intent_matches_current(&create, Some("new")));
}

#[test]
fn local_page_change_does_not_block_new_block_and_target_only_is_preserved() {
    let old_page = projected_item("page", "page-source", "page-target", "old");
    let mut history = baseline(Some(&old_page.fingerprint));
    history.key = old_page.key.clone();
    history.target_id = old_page.target_id.clone();
    let current = vec![
        projected_item("page", "page-target", "page-target", "local"),
        projected_item("block", "target-only", "target-only", "keep"),
    ];
    let source = vec![
        projected_item("page", "page-source", "page-target", "upstream"),
        projected_item("block", "new", "new-target", "new block"),
    ];
    let before = history.clone();
    let plan =
        plan_template_resources(source, &current, &[history.clone()], &Default::default()).unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].decision, TemplateMergeDecision::SkipUserModified);
    assert!(plan[0].write_intent(Uuid::nil()).is_none());
    assert_eq!(plan[1].decision, TemplateMergeDecision::Initialize);
    assert_eq!(history, before);
    assert_eq!(current[1].value["content"], "keep");
}

#[test]
fn duplicate_or_changed_identity_is_structural_error_not_a_local_edit() {
    let item = projected_item("block", "source", "target", "a");
    assert!(plan_template_resources(
        vec![item.clone(), item.clone()],
        &[],
        &[],
        &Default::default()
    )
    .is_err());
    let mut wrong = baseline(Some("old"));
    wrong.target_id = "other".into();
    assert!(plan_template_resources(vec![item], &[], &[wrong], &Default::default()).is_err());
}

#[test]
fn source_and_target_identity_projection_match_without_generated_code_reference() {
    let page_id = Uuid::from_u128(1);
    let target_page_id = Uuid::from_u128(11);
    let tab_id = Uuid::from_u128(2);
    let target_tab_id = Uuid::from_u128(22);
    let mut source = package();
    source.pages.push(PortablePage {
        id: page_id, parent_id: None, kind: domain::FrontstagePageKind::Page,
        title: Some("Page".into()), icon: None, tooltip: None, is_hidden: false,
        placement: domain::frontstage::FrontstageNavigationPlacement::Sidebar,
        content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
        slug: None, rank: "a".into(), visibility_rules: vec![],
        tabs: vec![PortableTab {
            id: tab_id, title: None, rank: "a".into(), is_default: true, route_segment: None,
            document_root_uid: "source-root".into(),
            document_payload: json!({"uid":"source-root", "page_id":page_id, "created_by":"user schema content"}),
            blocks: vec![PortableBlock {
                block_id: "source-block".into(), parent_block_id: None, rank: "a".into(),
                presentation: domain::frontstage::FrontstageBlockPresentation::Inline,
                title: None, description: None, code_ref: "source-code-ref".into(), schema_version: 1,
                input_mapping: BTreeMap::new(), output_mapping: BTreeMap::new(),
                runtime_descriptor: json!({}), source_code: "return 1".into(),
            }],
        }],
    });
    let ids = BTreeMap::from([
        (page_id.to_string(), target_page_id.to_string()),
        (tab_id.to_string(), target_tab_id.to_string()),
        ("source-root".into(), "target-root".into()),
        ("source-block".into(), "target-block".into()),
    ]);
    let mut target = source.clone();
    target.pages[0].id = target_page_id;
    target.pages[0].tabs[0].id = target_tab_id;
    target.pages[0].tabs[0].document_root_uid = "target-root".into();
    target.pages[0].tabs[0].blocks[0].block_id = "target-block".into();
    target.pages[0].tabs[0].blocks[0].code_ref = "target-code-ref".into();
    rewrite_template_value(&mut target.pages[0].tabs[0].document_payload, &ids);
    let left = project_template_resources(&source, &ids).unwrap();
    let right = project_template_resources(&target, &BTreeMap::new()).unwrap();
    assert_eq!(left.len(), 4);
    for (left, right) in left.iter().zip(&right) {
        assert_eq!(left.target_id, right.target_id);
        assert_eq!(left.fingerprint, right.fingerprint);
    }
    target.pages[0].tabs[0].document_payload["created_by"] = json!("changed schema content");
    let changed = project_template_resources(&target, &BTreeMap::new()).unwrap();
    assert_eq!(right[0].fingerprint, changed[0].fingerprint);
    assert_eq!(right[1].fingerprint, changed[1].fingerprint);
    assert_ne!(right[2].fingerprint, changed[2].fingerprint);
}

#[test]
fn translations_are_per_locale_and_never_identity_rewritten() {
    let mut source = package();
    source.i18n_entries = vec![
        PortableI18nEntry {
            key: "title|key".into(),
            locale: "en".into(),
            translation: "source-id".into(),
        },
        PortableI18nEntry {
            key: "title|key".into(),
            locale: "zh-CN".into(),
            translation: "标题".into(),
        },
    ];
    let projection = project_template_resources(
        &source,
        &BTreeMap::from([("source-id".into(), "target-id".into())]),
    )
    .unwrap();
    assert_eq!(projection.len(), 2);
    assert_ne!(projection[0].key, projection[1].key);
    assert_eq!(projection[0].key.source_id, r#"["title|key","en"]"#);
    assert_eq!(projection[0].value["translation"], "source-id");
}

#[test]
fn fingerprints_sort_object_keys_but_keep_array_order() {
    let left: Value = serde_json::from_str(r#"{"b":2,"a":{"d":4,"c":3}}"#).unwrap();
    let right: Value = serde_json::from_str(r#"{"a":{"c":3,"d":4},"b":2}"#).unwrap();
    assert_eq!(
        template_resource_fingerprint(&left).unwrap(),
        template_resource_fingerprint(&right).unwrap()
    );
    assert_ne!(
        template_resource_fingerprint(&json!([1, 2])).unwrap(),
        template_resource_fingerprint(&json!([2, 1])).unwrap()
    );
}

#[test]
fn legacy_packages_default_translations_to_empty_without_emitting_new_field() {
    let legacy = package();
    assert!(legacy.i18n_entries.is_empty());
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("i18n_entries")
        .is_none());
}

#[test]
fn translations_require_new_inner_schema_and_have_distinct_locale_identity() {
    let mut value = package();
    value.i18n_entries.push(PortableI18nEntry {
        key: "title".into(),
        locale: "en_US".into(),
        translation: "Title".into(),
    });
    assert!(
        validate_portable_template(&value).contains(&"portable_template_i18n_requires_v2".into())
    );
    value.schema_version = PORTABLE_TEMPLATE_I18N_SCHEMA_VERSION.into();
    assert!(validate_portable_template(&value).is_empty());
    value.i18n_entries.push(value.i18n_entries[0].clone());
    assert!(validate_portable_template(&value)
        .contains(&"portable_template_invalid_i18n_identity".into()));
}

#[test]
fn mcp_metadata_group_and_binding_project_independently() {
    let mut source = package();
    source.mcp_bundle = Some(serde_json::from_value(json!({
        "manifest": {
            "schema_version":"1flowbase.mcp.bundle/v2","organization":"test","bundle_id":"test",
            "bundle_version":"1.0.0","locale":"en_US","minimum_host_version":"0.4.1",
            "exported_from_system_version":"0.4.1","exported_at":"2026-10-10T00:00:00Z","files":[]
        },
        "tools":[],"connections":[],"instances":[{
            "instance_id":"instance","name":"Instance","description_short":null,"status":"enabled",
            "default_entry_path":"/","groups":[{"path":"/","display_name":"Root","description_short":null,"enabled":true,"sort_order":0}],
            "bindings":[{"group_path":"/","tool_id":"tool","display_alias":null,"visible":true,"sort_order":0}],
            "discovery_policy":{"list_default_limit":20,"list_max_depth":5,"list_regex_enabled":false,"list_regex_max_length":100,"list_return_fields":[]}
        }]
    })).unwrap());
    let before = project_template_resources(&source, &BTreeMap::new()).unwrap();
    source.mcp_bundle.as_mut().unwrap().instances[0].groups[0].display_name = "Local name".into();
    let after = project_template_resources(&source, &BTreeMap::new()).unwrap();
    assert_eq!(before.len(), 4);
    assert_eq!(after.len(), 4);
    for kind in [
        "mcp_instance",
        "mcp_discovery_policy",
        "mcp_group",
        "mcp_binding",
    ] {
        let prior = before.iter().find(|entry| entry.key.kind == kind).unwrap();
        let current = after.iter().find(|entry| entry.key == prior.key).unwrap();
        if kind == "mcp_group" {
            assert_ne!(prior.fingerprint, current.fingerprint);
        } else {
            assert_eq!(
                prior.fingerprint, current.fingerprint,
                "unexpected change: {kind}"
            );
        }
    }
}
