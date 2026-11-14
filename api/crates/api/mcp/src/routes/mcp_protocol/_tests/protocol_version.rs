use super::*;

#[test]
fn negotiation_and_stateless_header_rules_are_version_aware() {
    let mut headers = HeaderMap::new();
    for version in [LEGACY, LATEST] {
        assert_eq!(
            negotiate("initialize", &json!({"protocolVersion":version}), &headers).unwrap(),
            version
        );
        headers.insert("mcp-protocol-version", version.parse().unwrap());
        assert_eq!(
            negotiate("tools/call", &json!({}), &headers).unwrap(),
            version
        );
    }
    assert_eq!(
        negotiate(
            "initialize",
            &json!({"protocolVersion":"2099-01-01"}),
            &headers
        )
        .unwrap(),
        LATEST
    );
    headers.remove("mcp-protocol-version");
    assert_eq!(
        negotiate("resources/read", &json!({}), &headers).unwrap(),
        LEGACY
    );
    headers.insert("mcp-protocol-version", "2099-01-01".parse().unwrap());
    assert!(negotiate("tools/list", &json!({}), &headers).is_err());
    assert!(negotiate("initialize", &json!({}), &headers).is_err());
}

#[test]
fn legacy_schema_preserves_information_without_new_content_fields() {
    let result = json!({"content":[
        {"type":"text","text":"hello","annotations":{"priority":0.5,"lastModified":"2026-11-06T00:00:00Z"}},
        {"type":"image","data":"YQ==","mimeType":"image/png"},
        {"type":"audio","data":"YQ==","mimeType":"audio/wav"},
        {"type":"resource_link","name":"file","uri":"urn:file:a"},
        {"type":"resource","resource":{"uri":"urn:file:a","blob":"YQ=="}}
    ],"structuredContent":{"only_here":42},"isError":false});
    assert_eq!(project(result.clone(), LATEST), result);
    let legacy = project(result, LEGACY);
    assert!(legacy.get("structuredContent").is_none());
    assert!(legacy["content"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["type"] != "resource_link"));
    assert_eq!(legacy["content"][4]["resource"]["blob"], "YQ==");
    assert!(legacy["content"][0]["annotations"]
        .get("lastModified")
        .is_none());
    let link: Value = serde_json::from_str(legacy["content"][3]["text"].as_str().unwrap()).unwrap();
    assert_eq!(link["uri"], "urn:file:a");
    assert_eq!(legacy["content"][5]["text"], "{\"only_here\":42}");
    let repeated = project(
        json!({"content":[{"type":"text","text":"{\"a\":1}"}],"structuredContent":{"a":1}}),
        LEGACY,
    );
    assert_eq!(repeated["content"].as_array().unwrap().len(), 1);
}
