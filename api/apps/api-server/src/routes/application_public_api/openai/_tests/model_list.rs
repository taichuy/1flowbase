use super::*;

fn model(id: &str, multimodal: bool) -> OpenAiCompatibleModel {
    serde_json::from_value(json!({
        "id": id,
        "capabilities": {
            "reasoning": false,
            "tool_call": false,
            "multimodal": multimodal,
            "structured_output": false
        }
    }))
    .expect("valid published model descriptor")
}

#[test]
fn codex_catalog_preserves_each_models_image_input_capability() {
    let response =
        to_codex_model_list_response(vec![model("vision", true), model("text-only", false)]);

    assert_eq!(response["models"][0]["slug"], "vision");
    assert_eq!(
        response["models"][0]["input_modalities"],
        json!(["text", "image"])
    );
    assert_eq!(response["models"][1]["slug"], "text-only");
    assert_eq!(response["models"][1]["input_modalities"], json!(["text"]));
}

#[test]
fn ordinary_model_catalog_retains_the_same_multimodal_capability() {
    let response = to_openai_model_list_response(vec![model("vision", true)], 42);
    let response = serde_json::to_value(response).expect("serializable model list");

    assert_eq!(response["object"], "list");
    assert_eq!(response["data"][0]["id"], "vision");
    assert_eq!(response["data"][0]["capabilities"]["multimodal"], true);
    assert_eq!(response["data"][0]["created"], 42);
}
