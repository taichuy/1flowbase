use std::io::Read;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    middleware::from_fn,
    response::IntoResponse,
    routing::get,
    Router,
};
use flate2::read::GzDecoder;
use futures_util::stream;
use tower::ServiceExt;

use super::compress_bounded_http_response;

fn test_router() -> Router {
    let large_json = serde_json::json!({
        "blocks": (0..40)
            .map(|idx| format!("export default function Block{idx}() {{ return <div>metric-{idx}</div>; }}"))
            .collect::<Vec<_>>()
    })
    .to_string();

    Router::new()
        .route(
            "/json-large",
            get(move || {
                let payload = large_json.clone();
                async move {
                    (
                        [(header::CONTENT_TYPE, "application/json")],
                        payload,
                    )
                }
            }),
        )
        .route(
            "/json-tiny",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    r#"{"ok":true}"#,
                )
            }),
        )
        .route(
            "/sse",
            get(|| async {
                let s = stream::iter(vec![Ok::<_, std::convert::Infallible>("data: ping\n\n")]);
                (
                    [(header::CONTENT_TYPE, "text/event-stream")],
                    Body::from_stream(s),
                )
                    .into_response()
            }),
        )
        .route(
            "/stream-json",
            get(|| async {
                let s = stream::iter(vec![Ok::<_, std::convert::Infallible>("{\"chunk\":1}")]);
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    Body::from_stream(s),
                )
                    .into_response()
            }),
        )
        .layer(from_fn(compress_bounded_http_response))
}

#[tokio::test]
async fn compresses_bounded_large_json_when_gzip_accepted_and_preserves_identity_otherwise() {
    let app = test_router();

    let res_gz = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/json-large")
                .header(header::ACCEPT_ENCODING, "br, gzip;q=0.9")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_gz.status(), StatusCode::OK);
    assert_eq!(res_gz.headers().get(header::CONTENT_ENCODING).unwrap(), "gzip");
    assert!(
        res_gz
            .headers()
            .get(header::VARY)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Accept-Encoding")
    );
    let gz_bytes = to_bytes(res_gz.into_body(), 1024 * 1024).await.unwrap();
    let mut decoded = String::new();
    GzDecoder::new(&gz_bytes[..])
        .read_to_string(&mut decoded)
        .unwrap();
    assert!(gz_bytes.len() < decoded.len() / 2);

    let res_identity = app
        .oneshot(
            Request::builder()
                .uri("/json-large")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(res_identity.headers().get(header::CONTENT_ENCODING).is_none());
    let raw_bytes = to_bytes(res_identity.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(std::str::from_utf8(&raw_bytes).unwrap(), decoded);
}

#[tokio::test]
async fn bypasses_tiny_bodies_sse_and_unbounded_streams() {
    let app = test_router();

    for path in ["/json-tiny", "/sse", "/stream-json"] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::ACCEPT_ENCODING, "gzip")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "failed on {path}");
        assert!(
            res.headers().get(header::CONTENT_ENCODING).is_none(),
            "must not compress {path}"
        );
    }
}
