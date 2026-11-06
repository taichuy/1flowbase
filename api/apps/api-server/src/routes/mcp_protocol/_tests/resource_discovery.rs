use super::*;

fn fixture() -> (
    domain::ActorContext,
    domain::McpCatalogSnapshot,
    virtual_ui::VirtualMcpScope,
) {
    let mut catalog = super::super::tests::catalog_with_server_bound_workspace();
    catalog.tools[0].output_mapping = json!({"structured_content_path":"/metadata","resource_read":true,
        "resources":[{"name":"a","uri":"urn:file:a","arguments":{"path":"a"},"mime_type":"application/octet-stream"}],
        "mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name"},{"type":"resource","uri_path":"/uri","blob_path":"/blob","skip_if_null":true}]});
    let actor = domain::ActorContext::root(
        uuid::Uuid::from_u128(1),
        catalog.instances[0].workspace_id,
        "root",
    );
    (
        actor,
        catalog,
        virtual_ui::VirtualMcpScope::single("selected".into()),
    )
}

#[test]
fn discovery_projects_current_visible_read_catalog_and_rechecks_authority() {
    let (actor, mut catalog, scope) = fixture();
    let listed = entries(&actor, &catalog, &scope, false).unwrap();
    assert_eq!(listed.len(), 1);
    let uri = listed.values().next().unwrap()["uri"].as_str().unwrap();
    let reference = ResourceReference::decode(uri).unwrap();
    assert_eq!(reference.resource_uri, "urn:file:a");
    assert_eq!(reference.arguments, json!({"path":"a"}));
    assert!(authorize_reference(&reference, &actor, &catalog, &scope).is_ok());
    catalog.bindings[0].visible = false;
    assert!(entries(&actor, &catalog, &scope, false).unwrap().is_empty());
    assert!(entries(&actor, &catalog, &scope, true).unwrap().is_empty());
    assert!(authorize_reference(&reference, &actor, &catalog, &scope).is_err());
    catalog.bindings[0].visible = true;
    catalog.tools[0].output_mapping["resource_read"] = json!(false);
    assert!(entries(&actor, &catalog, &scope, false).unwrap().is_empty());
}

#[test]
fn template_expansion_uses_request_identity_original_uri_and_current_tool_revision() {
    let (actor, mut catalog, scope) = fixture();
    let entries = entries(&actor, &catalog, &scope, true).unwrap();
    let template = entries.values().next().unwrap()["uriTemplate"]
        .as_str()
        .unwrap();
    let encode = |value: &str| {
        percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
    };
    let expand = |original: &str| {
        template
            .replace(
                "{arguments}",
                &encode(r#"{"path":"a + b/中文","link_only":true}"#),
            )
            .replace("{resource_uri}", &encode(original))
    };
    let reference = decode_read_reference(&expand("urn:file:a"), &actor).unwrap();
    assert_eq!(reference.arguments["path"], "a + b/中文");
    assert_eq!(reference.user_id, actor.user_id);
    assert!(authorize_reference(&reference, &actor, &catalog, &scope).is_ok());
    let mut other = actor.clone();
    other.user_id = uuid::Uuid::from_u128(99);
    assert_eq!(
        decode_read_reference(&expand("urn:file:a"), &other)
            .unwrap()
            .user_id,
        other.user_id
    );
    catalog.tools[0].des_id = "new-version".into();
    assert!(authorize_reference(&reference, &actor, &catalog, &scope).is_err());
    for invalid in [
        "flowbase-resource://read/anything",
        "flowbase-resource://template/anything",
        "relative",
        "urn:bad:%Q0",
    ] {
        assert!(decode_read_reference(&expand(invalid), &actor).is_err());
    }
    assert!(decode_read_reference(
        &format!("{}&arguments=%7B%7D", expand("urn:file:a")),
        &actor
    )
    .is_err());
    assert!(decode_read_reference(
        &expand("urn:file:a").replace(&encode(r#"{"path":"a + b/中文","link_only":true}"#), "[]"),
        &actor
    )
    .is_err());
}

#[test]
fn discovery_cursor_paginates_stably_under_json_budget_and_rejects_wrong_kind() {
    let catalog = (0..20).map(|n| (format!("urn:{n:03}"), json!({"name":format!("item-{n}"),"uri":format!("urn:{n:03}"),"description":"a".repeat(80)}))).collect::<BTreeMap<_,_>>();
    let mut args = json!({});
    let mut seen = Vec::new();
    loop {
        let result = page(catalog.clone(), &args, false, 500).unwrap();
        assert!(serde_json::to_vec(&result).unwrap().len() <= 500);
        seen.extend(
            result["resources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["uri"].as_str().unwrap().to_owned()),
        );
        match result.get("nextCursor") {
            Some(cursor) => {
                args["cursor"] = cursor.clone();
                assert!(page(catalog.clone(), &args, true, 500).is_err());
            }
            None => break,
        }
    }
    assert_eq!(seen, catalog.keys().cloned().collect::<Vec<_>>());
    assert!(page(catalog, &json!({"cursor":"bad!"}), false, 500).is_err());
    assert_eq!(
        page(BTreeMap::new(), &json!({}), true, 500).unwrap(),
        json!({"resourceTemplates":[]})
    );
}
