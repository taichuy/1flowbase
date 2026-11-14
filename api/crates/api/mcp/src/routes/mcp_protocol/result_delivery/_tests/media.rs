use super::*;

fn declaration() -> Value {
    json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"})
}

#[test]
fn image_projection_retains_complete_base64_and_only_structured_metadata() {
    let data = STANDARD.encode(vec![42; 64_000]);
    let metadata = json!({"path":"/tmp/image.jpg","byte_count":64000});
    let mapped = project(
        &mapping(&declaration()).unwrap().unwrap(),
        &json!({"data":data,"mime_type":"image/jpeg","metadata":metadata}),
    )
    .unwrap();
    assert_eq!(mapped.content[0]["data"], data);
    assert_eq!(mapped.content[0]["mimeType"], "image/jpeg");
    assert_eq!(mapped.structured_content, Some(metadata.clone()));
    assert_eq!(
        STANDARD
            .decode(mapped.content[0]["data"].as_str().unwrap())
            .unwrap()
            .len(),
        64000
    );
    let delivered = retain_content(super::super::tool_result(metadata), &mapped.content);
    assert_eq!(delivered["content"][1]["data"], data);
    assert!(delivered["structuredContent"].get("data").is_none());
}

#[test]
fn media_projection_rejects_invalid_data_paths_mime_and_mapping() {
    let parsed = mapping(&declaration()).unwrap().unwrap();
    for source in [
        json!({"data":"%%%","mime_type":"image/jpeg","metadata":{}}),
        json!({"data":"","mime_type":"image/jpeg","metadata":{}}),
        json!({"data":"YQ==","mime_type":"text/plain","metadata":{}}),
        json!({"mime_type":"image/jpeg","metadata":{}}),
        json!({"data":"YQ==","mime_type":"image/jpeg","metadata":[]}),
    ] {
        assert!(project(&parsed, &source).is_err());
    }
    assert!(mapping(&json!({"mcp_content":[]})).is_err());
    assert!(mapping(&json!({"properties":{"stdout":{"type":"string"}}}))
        .unwrap()
        .is_none());
    for mime in ["image/jpeg", "image/png", "image/gif", "image/webp"] {
        assert!(project(
            &parsed,
            &json!({"data":"YQ==","mime_type":mime,"metadata":{}})
        )
        .is_ok());
    }
}

#[test]
fn proxy_media_survives_metadata_paging_without_duplicating_original_text() {
    let upstream =
        json!([{"type":"image","data":"YQ==","mimeType":"image/png"},{"type":"text","text":"old"}]);
    let delivered = retain_content(
        super::super::tool_result(json!({"next_cursor":"cursor"})),
        &upstream,
    );
    assert_eq!(delivered["content"].as_array().unwrap().len(), 2);
    assert_eq!(delivered["content"][1], upstream[0]);
    assert_ne!(delivered["content"][0]["text"], "old");
}

#[test]
fn standard_projection_and_local_text_survive_metadata_continuation() {
    let declaration = json!({"structured_content_path":"/metadata","mcp_content":[
        {"type":"text","text_path":"/text","annotations_path":"/annotations"},
        {"type":"audio","data_path":"/data","mime_type_path":"/audio_mime"},
        {"type":"resource_link","uri_path":"/uri","name_path":"/name","description_path":"/description","mime_type_path":"/mime"},
        {"type":"resource","uri_path":"/uri","text_path":"/text","mime_type_path":"/mime"},
        {"type":"resource","uri_path":"/uri","blob_path":"/blob"}
    ]});
    let source = json!({"text":"native text","annotations":{"audience":["assistant"],"priority":0.5,"lastModified":"2026-10-13T00:00:00Z"},"data":"YQ==","audio_mime":"audio/ogg","uri":"file:///tmp/content","name":"content","description":"linked file","mime":"text/plain","blob":"","metadata":{"count":1}});
    let projected = project(&mapping(&declaration).unwrap().unwrap(), &source).unwrap();
    assert_eq!(projected.content[0]["annotations"], source["annotations"]);
    assert_eq!(
        projected.content[1],
        json!({"type":"audio","data":"YQ==","mimeType":"audio/ogg"})
    );
    assert_eq!(projected.content[2]["uri"], source["uri"]);
    assert_eq!(projected.content[3]["resource"]["text"], "native text");
    assert_eq!(projected.content[4]["resource"]["blob"], "");
    let delivered = retain_native_content(
        super::super::tool_result(json!({"next_cursor":"cursor"})),
        &projected.content,
    );
    assert_eq!(delivered["content"][1]["text"], "native text");
    assert_eq!(delivered["content"].as_array().unwrap().len(), 6);
    assert_eq!(
        delivered["structuredContent"],
        json!({"next_cursor":"cursor"})
    );
}

#[test]
fn resource_projection_and_standard_validation_reject_invalid_shapes() {
    for mime in ["image/svg+xml", "image/avif", "image/tiff"] {
        assert!(valid_mime(mime, Some("image")));
    }
    for mime in ["audio/wav", "audio/mpeg", "audio/opus"] {
        assert!(valid_mime(mime, Some("audio")));
    }
    for mime in [
        "image/",
        "image/png; charset=utf8",
        "image/png/extra",
        " image/png",
    ] {
        assert!(!valid_mime(mime, Some("image")));
    }
    for uri in [
        "file:///tmp/a",
        "urn:example:a",
        "custom://host/a",
        "https://example.com/a%20b",
    ] {
        assert!(valid_uri(uri));
    }
    for uri in [
        "relative/a",
        "https://example.com/a b",
        "https://example.com/%zz",
        "1bad:x",
        "https://[invalid",
    ] {
        assert!(!valid_uri(uri));
    }
    let declaration = json!({"structured_content_path":"/metadata","mcp_content":[{"type":"resource","uri_path":"/uri","blob_path":"/blob"}]});
    let parsed = mapping(&declaration).unwrap().unwrap();
    for source in [
        json!({"metadata":{},"uri":"relative","blob":"YQ=="}),
        json!({"metadata":{},"uri":"urn:test:a","blob":"YR=="}),
        json!({"metadata":{},"uri":"urn:test:a","blob":1}),
    ] {
        assert!(project(&parsed, &source).is_err());
    }
    assert!(!valid_annotations(&json!({"priority":2})));
    assert!(!valid_annotations(&json!({"audience":["system"]})));
    assert!(!valid_annotations(&json!([])));
}

#[test]
fn nullable_resource_skips_only_explicit_null_and_preserves_link() {
    let declaration = json!({"structured_content_path":"/metadata","resource_read":true,
        "mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name"},
            {"type":"resource","uri_path":"/uri","blob_path":"/blob","skip_if_null":true}]});
    let parsed = mapping(&declaration).unwrap().unwrap();
    let source = json!({"uri":"urn:file:a","name":"a","blob":null,"metadata":{}});
    let result = project(&parsed, &source).unwrap();
    assert_eq!(result.content.as_array().unwrap().len(), 1);
    assert_eq!(result.content[0]["type"], "resource_link");
    let mut missing = source.clone();
    missing.as_object_mut().unwrap().remove("blob");
    assert!(project(&parsed, &missing).is_err());
    for invalid in [json!(false), json!({}), json!("%%%")] {
        let mut source = source.clone();
        source["blob"] = invalid;
        assert!(project(&parsed, &source).is_err());
    }
    let mut required = declaration;
    required["mcp_content"][1]
        .as_object_mut()
        .unwrap()
        .remove("skip_if_null");
    assert!(project(&mapping(&required).unwrap().unwrap(), &source).is_err());
    let mut complete = source;
    complete["blob"] = json!("YQ==");
    assert_eq!(
        project(&parsed, &complete).unwrap().content[1]["resource"]["blob"],
        "YQ=="
    );
}
