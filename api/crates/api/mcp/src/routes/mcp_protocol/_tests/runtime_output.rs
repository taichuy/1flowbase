use super::*;
#[test]
fn explicit_image_tool_result_reaches_llm_as_image_blocks_with_metadata_text() {
    let result = model_tool_content(json!({"content":[
        {"type":"text","text":"authorized file metadata"},
        {"type":"image","mimeType":"image/jpeg","data":"aW1hZ2U="}
    ],"structuredContent":{"file_record_id":"record"}}));
    assert_eq!(result[0]["text"], "authorized file metadata");
    assert_eq!(
        result[1],
        json!({"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"aW1hZ2U="}})
    );
    assert!(result.is_array());
    // Ordinary metadata and non-MCP business results retain their existing semantics.
    let plain = json!({"content":[{"type":"text","text":"file metadata only"}],"structuredContent":{"id":"a"}});
    assert_eq!(model_tool_content(plain.clone()), plain);
    let metadata =
        json!({"file_record_id":"record","url":"/authorized/content","media_type":"image/jpeg"});
    assert_eq!(model_tool_content(metadata.clone()), metadata);
}
