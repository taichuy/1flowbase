use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

fn response(result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: Some(json!("image-call")),
        result: Some(result),
        error: None,
    }
}

#[test]
fn final_jsonrpc_envelope_delivers_complete_native_images_above_text_limit() {
    let bytes = vec![91; JSON_RPC_RESPONSE_MAX_BYTES * 2];
    let data = STANDARD.encode(&bytes);
    let (_, delivered) = bounded_jsonrpc_response(
        StatusCode::OK,
        response(json!({
            "content":[{"type":"text","text":"metadata"},{"type":"image","data":data,"mimeType":"image/png"}],
            "structuredContent":{"path":"/tmp/image.png","byte_count":bytes.len()}
        })),
    );
    assert!(delivered.0.error.is_none());
    let result = delivered.0.result.unwrap();
    assert_eq!(result["content"][1]["data"], data);
    assert_eq!(
        STANDARD
            .decode(result["content"][1]["data"].as_str().unwrap())
            .unwrap(),
        bytes
    );
}

#[test]
fn image_exemption_never_exempts_metadata_text_unknown_fields_or_invalid_media() {
    let data = STANDARD.encode(vec![91; JSON_RPC_RESPONSE_MAX_BYTES * 2]);
    let image = json!({"type":"image","data":data,"mimeType":"image/jpeg"});
    let oversized = "x".repeat(JSON_RPC_RESPONSE_MAX_BYTES);
    for result in [
        json!({"content":[image],"structuredContent":{"description":oversized}}),
        json!({"content":[image,{"type":"text","text":oversized}]}),
        json!({"content":[{"type":"image","data":data,"mimeType":"image/png","unknown":oversized}]}),
        json!({"content":[image],"unknown":oversized}),
        json!({"content":[{"type":"image","data":data,"mimeType":"text/plain"}]}),
        json!({"content":[{"type":"image","data":"!".repeat(JSON_RPC_RESPONSE_MAX_BYTES),"mimeType":"image/png"}]}),
        json!({"content":[{"type":"resource","data":data,"mimeType":"image/png"}]}),
        json!({"content":[{"type":"text","text":data}]}),
    ] {
        let (_, delivered) = bounded_jsonrpc_response(StatusCode::OK, response(result));
        assert!(delivered.0.result.is_none());
        assert_eq!(
            delivered.0.error.unwrap()["data"]["category"],
            "response_size_limit"
        );
    }
}

#[test]
fn image_budget_validation_requires_canonical_complete_base64_across_chunks() {
    for count in [1, 2, 3, 3071, 3072, 3073, 6143, 6144, 6145] {
        assert!(valid_image_base64(&STANDARD.encode(vec![1; count])));
    }
    for invalid in ["", "A", "YQ=", "YR==", "YQ==YQ==", "YWJj\n", "____"] {
        assert!(!valid_image_base64(invalid), "{invalid}");
    }
    let invalid = format!("{}YQ==", "YQ==".repeat(1024));
    assert!(!valid_image_base64(&invalid));
}

#[test]
fn standard_audio_and_resource_blobs_bypass_only_binary_budget() {
    let data = STANDARD.encode(vec![33; JSON_RPC_RESPONSE_MAX_BYTES * 2]);
    for result in [
        json!({"content":[{"type":"audio","data":data,"mimeType":"audio/ogg"}]}),
        json!({"content":[{"type":"resource","resource":{"uri":"urn:example:blob","mimeType":"application/octet-stream","blob":data}}]}),
        json!({"contents":[{"uri":"file:///tmp/blob","blob":data}]}),
        json!({"contents":[{"uri":"urn:example:empty","blob":""}]}),
    ] {
        let original = result.clone();
        let (_, delivered) = bounded_jsonrpc_response(StatusCode::OK, response(result));
        assert!(delivered.0.error.is_none());
        assert_eq!(delivered.0.result, Some(original));
    }
}

#[test]
fn resource_text_metadata_unknown_fields_and_invalid_binary_stay_budgeted() {
    let data = STANDARD.encode(vec![33; JSON_RPC_RESPONSE_MAX_BYTES * 2]);
    let oversized = "x".repeat(JSON_RPC_RESPONSE_MAX_BYTES);
    for result in [
        json!({"content":[{"type":"resource","resource":{"uri":"urn:test:a","text":oversized}}]}),
        json!({"contents":[{"uri":"urn:test:a","text":oversized}]}),
        json!({"contents":[{"uri":"relative","blob":data}]}),
        json!({"contents":[{"uri":"urn:test:a","blob":data,"text":"also text"}]}),
        json!({"contents":[{"uri":"urn:test:a","blob":data,"mimeType":"invalid"}]}),
        json!({"contents":[{"uri":"urn:test:a","blob":data,"unknown":oversized}]}),
        json!({"content":[{"type":"audio","data":data,"mimeType":"image/png"}]}),
        json!({"content":[{"type":"audio","data":data,"mimeType":"audio/ogg","annotations":{"description":oversized}}]}),
        json!({"content":[{"type":"resource_link","uri":"urn:test:a","name":oversized}]}),
        json!({"contents":[{"uri":"urn:test:a","blob":data}],"structuredContent":{"data":oversized}}),
    ] {
        let (_, delivered) = bounded_jsonrpc_response(StatusCode::OK, response(result));
        assert!(delivered.0.result.is_none());
        assert_eq!(
            delivered.0.error.unwrap()["data"]["category"],
            "response_size_limit"
        );
    }
}
