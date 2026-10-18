use crate::_tests::support::{test_api_state_with_database_url, test_app, test_config};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn gateway_http_large_bodies_preserve_authentication_rejection() {
    let app = test_app().await;
    for path in [
        "/chat/completions",
        "/v1/chat/completions",
        "/responses",
        "/v1/responses",
        "/v1/responses/compact",
        "/v1/messages",
        "/v1/messages/count_tokens",
        "/api/agent/v1/runs",
        "/api/agent/v1/runs/00000000-0000-0000-0000-000000000000/resume",
    ] {
        let body = json!({"model":"fixture", "callback_task_id":"00000000-0000-0000-0000-000000000000", "response_payload":{"text":"x".repeat(16 * 1024 * 1024 + 1)}});
        let response = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).expect("protocol error must remain JSON");
        assert!(value.get("error").is_some(), "{path}: {value}");
    }
}

#[tokio::test]
async fn gateway_multipart_large_file_reaches_file_table_validation() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let token = super::setup_published_native_app(&app, &state, "Large multipart").await;
    let body = format!("--gateway-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"large.txt\"\r\nContent-Type: text/plain\r\n\r\n{}\r\n--gateway-boundary--\r\n", "x".repeat(2 * 1024 * 1024 + 1));
    let response = app
        .oneshot(
            Request::post("/api/agent/v1/files")
                .header("authorization", format!("Bearer {token}"))
                .header(
                    "content-type",
                    "multipart/form-data; boundary=gateway-boundary",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(value["error"]["code"], "file_table_id", "{value}");
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn gateway_native_websocket_preserves_a_message_above_old_frame_and_message_limits() {
    use tokio_tungstenite::tungstenite::{
        client::IntoClientRequest, protocol::WebSocketConfig, Message,
    };
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let token = super::setup_published_native_app(&app, &state, "Large Native WebSocket").await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let mut request = format!("ws://{address}/api/agent/v1/runs/websocket")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    request.headers_mut().insert(
        "sec-websocket-protocol",
        "1flowbase.native.v1".parse().unwrap(),
    );
    let config = WebSocketConfig::default()
        .max_message_size(None)
        .max_frame_size(None);
    let (mut socket, _) =
        tokio_tungstenite::connect_async_with_config(request, Some(config), false)
            .await
            .unwrap();
    // Echoed request_id proves the complete UTF-8 payload survives both transport directions.
    let id = format!("{}完整🙂", "x".repeat(64 * 1024 * 1024 + 1));
    let body = json!({"type":"run.cancel", "request_id":id, "run_id":"00000000-0000-0000-0000-000000000000"});
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        socket
            .send(Message::Text(body.to_string().into()))
            .await
            .unwrap();
        let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let response: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(response["request_id"].as_str(), Some(id.as_str()));
        assert_eq!(response["type"], "error");
    })
    .await
    .unwrap();
}
