use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};

async fn protocol(app: &Router, token: &str, method: &str, params: Value) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mcp/terminal_media")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    assert_ne!(response["result"]["isError"], true, "{response}");
    response["result"].clone()
}

#[tokio::test]
async fn terminal_mcp_range_image_audio_and_resource_preserve_binary_data_through_envelopes() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status,created) = request(&app,"POST","/api/console/mcp/instances",&cookie,&csrf,json!({"instance_id":"terminal_media","name":"Terminal media","status":"enabled","default_entry_path":"/"})).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, key) = request(
        &app,
        "POST",
        "/api/console/user-api-keys",
        &cookie,
        &csrf,
        json!({"name":"terminal media client","expiration_policy":"never"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let token = key["data"]["token"].as_str().unwrap();
    let temp = Temp::new();
    let image = temp.0.join("image.png");
    let image_bytes = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jr1sAAAAASUVORK5CYII=").unwrap();
    std::fs::write(&image, &image_bytes).unwrap();
    let audio = temp.0.join("audio.wav");
    let audio_bytes = b"RIFF\x24\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\0\0\0\0";
    std::fs::write(&audio, audio_bytes).unwrap();
    let resource = temp.0.join("resource.bin");
    let resource_bytes = b"\0binary resource\xff";
    std::fs::write(&resource, resource_bytes).unwrap();
    let cases = [
        (
            "range",
            "terminal.files.read_range",
            "read-range",
            json!({"path":resource,"timeout_ms":5000,"offset":1,"length":6}),
            json!({}),
            STANDARD.encode(&resource_bytes[1..7]),
            "application/octet-stream",
        ),
        (
            "image",
            "terminal.files.read_image",
            "read-image",
            json!({"path":image,"timeout_ms":5000,"max_file_bytes":1024}),
            json!({"structured_content_path":"/metadata","mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}]}),
            STANDARD.encode(&image_bytes),
            "image/png",
        ),
        (
            "audio",
            "terminal.files.read_audio",
            "read-audio",
            json!({"path":audio,"timeout_ms":5000,"max_file_bytes":1024}),
            json!({"structured_content_path":"/metadata","mcp_content":[{"type":"audio","data_path":"/data","mime_type_path":"/mime_type"}]}),
            STANDARD.encode(audio_bytes),
            "audio/wav",
        ),
        (
            "resource",
            "terminal.files.read_resource",
            "read-resource",
            json!({"path":resource,"timeout_ms":5000,"max_file_bytes":1024,"link_only":false}),
            json!({"resource_read":true,"resource_read_arguments":{"link_only":false},"structured_content_path":"/metadata","mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name","mime_type_path":"/mime_type"},{"type":"resource","uri_path":"/uri","blob_path":"/data","mime_type_path":"/mime_type","skip_if_null":true}]}),
            STANDARD.encode(resource_bytes),
            "application/octet-stream",
        ),
    ];
    for (kind, interface_id, suffix, arguments, output_mapping, encoded, mime_type) in cases {
        let tool_id = format!("terminal_media_{kind}");
        let mappings = arguments.as_object().unwrap().keys().map(|name|json!({"interface_param":name,"source":{"kind":"mcp_argument","path":name},"required":true})).collect::<Vec<_>>();
        let (status,created) = request(&app,"POST","/api/console/mcp/tools",&cookie,&csrf,json!({"tool_id":tool_id,"des_id":"terminalmediav1","name":format!("Terminal {kind}"),"short_description":"Read a local binary fixture.","full_description":"Read a local binary fixture through the canonical terminal interface.","execution_target":{"kind":"interface_wrapper","interface_id":interface_id},"parameter_schema":{},"result_schema":{},"input_mapping":{"mappings":mappings},"output_mapping":output_mapping,"permission_code":null,"risk_level":"low","status":"enabled"})).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let (status, binding) = request(
            &app,
            "POST",
            "/api/console/mcp/instances/terminal_media/tool-bindings",
            &cookie,
            &csrf,
            json!({"group_path":"/","tool_id":tool_id,"visible":true,"sort_order":0}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{binding}");
        let (status, http) = request(
            &app,
            "POST",
            &format!("/api/console/terminal/files/{suffix}"),
            &cookie,
            &csrf,
            arguments.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{http}");
        assert!(
            http["data"].is_object(),
            "HTTP data is the business DTO, not its base64 field"
        );
        assert_eq!(http["meta"], Value::Null);
        assert_eq!(http["data"]["data"], encoded);
        assert_eq!(http["data"]["mime_type"], mime_type);
        let result = protocol(
            &app,
            token,
            "tools/call",
            json!({"name":"mcp_call","arguments":{"tool_id":tool_id,"arguments":arguments}}),
        )
        .await;
        match kind {
            "range" => assert_eq!(result["structuredContent"], http["data"]),
            "image" | "audio" => {
                let native = result["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|block| block["type"] == kind)
                    .unwrap();
                assert_eq!(native["data"], encoded);
                assert_eq!(native["mimeType"], mime_type);
                assert_eq!(result["structuredContent"], http["data"]["metadata"]);
            }
            "resource" => {
                let content = result["content"].as_array().unwrap();
                let native = content
                    .iter()
                    .find(|block| block["type"] == "resource")
                    .unwrap();
                assert_eq!(native["resource"]["blob"], encoded);
                assert_eq!(native["resource"]["mimeType"], mime_type);
                let mut link_arguments = arguments;
                link_arguments["link_only"] = json!(true);
                let linked = protocol(&app,token,"tools/call",json!({"name":"mcp_call","arguments":{"tool_id":tool_id,"arguments":link_arguments}})).await;
                let blocks = linked["content"].as_array().unwrap();
                assert!(blocks.iter().all(|block| block["type"] != "resource"));
                let uri = blocks
                    .iter()
                    .find(|block| block["type"] == "resource_link")
                    .unwrap()["uri"]
                    .as_str()
                    .unwrap();
                assert!(uri.starts_with("flowbase-resource://"));
                let replay = protocol(&app, token, "resources/read", json!({"uri":uri})).await;
                assert_eq!(replay["contents"][0]["uri"], uri);
                assert_eq!(replay["contents"][0]["blob"], encoded);
                assert_eq!(replay["contents"][0]["mimeType"], mime_type);
            }
            _ => unreachable!(),
        }
    }
}
