use super::*;
use TemplateMergeDecision::*;

fn history(package: &PortableTemplatePackage) -> Vec<TemplateResourceBaseline> {
    project_template_resources(package, &BTreeMap::new())
        .unwrap()
        .into_iter()
        .map(|item| TemplateResourceBaseline {
            key: item.key,
            target_id: item.target_id,
            generation: 7,
            applied_fingerprint: Some(item.fingerprint),
            pending: None,
            committed_operation_id: None,
            committed_fingerprint: None,
        })
        .collect()
}

fn plan(
    source: &PortableTemplatePackage,
    target: &PortableTemplatePackage,
    baselines: &[TemplateResourceBaseline],
) -> Vec<PlannedTemplateResource> {
    plan_native_template(
        source,
        target,
        &BTreeMap::new(),
        baselines,
        &BTreeMap::new(),
    )
    .unwrap()
}

fn tab() -> PortableTab {
    serde_json::from_value(json!({
        "id": Uuid::from_u128(20), "title": null, "rank": "a", "is_default": true,
        "route_segment": null, "document_root_uid": "root", "document_payload": {},
        "blocks": [{"block_id":"child", "parent_block_id":null, "rank":"a",
            "presentation":"inline", "title":null, "description":null, "code_ref":"code",
            "schema_version":1, "input_mapping":{}, "output_mapping":{},
            "runtime_descriptor":{}, "source_code":""}]
    }))
    .unwrap()
}

#[test]
fn native_plan_reverse_chain_preserves_projection_order_and_metadata() {
    let mut source = snapshot();
    source.pages = (1..=256)
        .rev()
        .map(|id| group(id, (id > 1).then_some(id - 1)))
        .collect();
    let baselines = history(&source);
    let result = plan(&source, &snapshot(), &baselines[255..]);
    let desired = project_template_resources(&source, &BTreeMap::new()).unwrap();
    for (index, (item, projection)) in result.iter().zip(desired).enumerate() {
        assert_eq!(item.decision, SkipUserDeleted);
        assert_eq!(item.desired.key, projection.key);
        assert_eq!(item.desired.fingerprint, projection.fingerprint);
        assert_eq!(item.desired.value, projection.value);
        assert_eq!(item.current_fingerprint, None);
        assert_eq!(item.baseline_generation, (index == 255).then_some(7));
        assert!(item.write_intent(Uuid::nil()).is_none());
    }
    assert_eq!(result.len(), 256);
}

#[test]
fn native_plan_cycles_require_a_seed_and_keep_existing_skip_reasons() {
    let mut source = snapshot();
    source.pages = vec![group(1, Some(2)), group(2, Some(1)), group(3, Some(1))];
    assert!(plan(&source, &snapshot(), &[])
        .iter()
        .all(|p| p.decision == Initialize));
    let mut target = source.clone();
    target.pages.remove(0);
    let baselines = history(&source);
    let result = plan(&source, &target, &baselines[..2]);
    // Existing unchanged page 2 becomes unavailable, but page 3 keeps its reason.
    assert_eq!(
        result
            .iter()
            .map(|p| p.decision.clone())
            .collect::<Vec<_>>(),
        vec![SkipUserDeleted, SkipUserDeleted, SkipUnknownBaseline]
    );

    let mut target = source.clone();
    target.pages[0].title = Some("local".into());
    target.pages.truncate(1);
    let result = plan(&source, &target, &baselines[..1]);
    assert_eq!(
        result
            .iter()
            .map(|p| p.decision.clone())
            .collect::<Vec<_>>(),
        vec![SkipUserModified, Initialize, Initialize]
    );
    assert!(plan(&source, &source, &baselines)
        .iter()
        .all(|p| p.decision == Unchanged));
}

#[test]
fn native_plan_shared_tab_source_propagates_pending_document_without_overwriting_tab() {
    let mut source = snapshot();
    source.pages = vec![group(1, None)];
    source.pages[0].tabs.push(tab());
    let mut baselines = history(&source);
    let document = baselines
        .iter_mut()
        .find(|b| b.key.kind == "tab_document")
        .unwrap();
    document.pending = Some(TemplateWriteIntent {
        operation_id: Uuid::from_u128(100),
        target_id: document.target_id.clone(),
        expected_fingerprint: None,
        desired_fingerprint: document.applied_fingerprint.clone().unwrap(),
    });
    // Mapping only the document's baseline cannot remove a document independently
    // from its tab DTO, so make both absent and leave only the document pending.
    let result = plan(&source, &snapshot(), &baselines[2..3]);
    assert_eq!(
        result
            .iter()
            .map(|p| p.decision.clone())
            .collect::<Vec<_>>(),
        vec![Initialize, Initialize, SkipPendingWrite, SkipUserDeleted]
    );
}

#[test]
fn native_plan_fields_and_visibility_follow_sources_but_not_role_codes() {
    let mut source = snapshot();
    source.pages = vec![group(1, None), group(2, None)];
    source.pages[1].tabs.push(tab());
    source.pages[1].visibility_rules = vec![
        PortablePageVisibilityRule {
            role_code: Uuid::from_u128(1).to_string(),
            tab_id: None,
            visibility: domain::frontstage::FrontstagePageVisibility::Visible,
        },
        PortablePageVisibilityRule {
            role_code: "reader".into(),
            tab_id: Some(Uuid::from_u128(20)),
            visibility: domain::frontstage::FrontstagePageVisibility::Visible,
        },
    ];
    let mut model = model(1, "model");
    model.fields.push(
        serde_json::from_value(json!({
            "id":Uuid::from_u128(30), "code":"field", "title":"Field", "field_kind":"string",
            "is_system":false, "is_required":false, "api_required":false, "is_unique":false,
            "display_options":{}, "relation_options":{}
        }))
        .unwrap(),
    );
    source.data_models.push(model);
    let baselines = history(&source);
    let seeds: Vec<_> = baselines
        .into_iter()
        .filter(|b| {
            (b.key.kind == "page" && b.key.source_id == Uuid::from_u128(1).to_string())
                || b.key.kind == "tab_document"
        })
        .collect();
    let result = plan(&source, &snapshot(), &seeds);
    let decisions: Vec<_> = result
        .iter()
        .map(|p| (p.desired.key.kind.as_str(), p.decision.clone()))
        .collect();
    assert_eq!(
        decisions,
        vec![
            ("page", SkipUserDeleted),
            ("page", Initialize),
            ("page_visibility", Initialize),
            ("page_visibility", SkipUserDeleted),
            ("tab", Initialize),
            ("tab_document", SkipUserDeleted),
            ("block", SkipUserDeleted),
            ("data_model", Initialize),
            ("model_field", SkipUserDeleted),
        ]
    );
}
