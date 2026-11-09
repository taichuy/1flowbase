use super::*;
use axum::{
    body::{to_bytes, Body},
    extract::{DefaultBodyLimit, Query, Request},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use tower::ServiceExt;

async fn observe(router: Router, req: Request, expected: u16) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status();
    assert_eq!(status.as_u16(), expected);
    let (parts, body) = response.into_parts();
    let body = to_bytes(body, 16_384).await.unwrap().to_vec();
    (status, parts.headers, body)
}

async fn old_body(Json(body): Json<CreateModelDefinitionBody>) -> Json<CreateModelDefinitionBody> {
    assert_eq!(body.code, "items");
    Json(body)
}

async fn new_body(body: CreateModelDefinitionBody) -> Json<CreateModelDefinitionBody> {
    assert_eq!(body.code, "items");
    Json(body)
}

async fn old_query(Query(query): Query<ListModelsQuery>) -> Json<ListModelsQuery> {
    assert_eq!(query.data_source_id.as_deref(), Some("main"));
    Json(query)
}

async fn new_query(query: ListModelsQuery) -> Json<ListModelsQuery> {
    assert_eq!(query.data_source_id.as_deref(), Some("main"));
    Json(query)
}

#[tokio::test]
async fn body_boundary_preserves_original_axum_status_headers_bytes_and_error_paths() {
    let valid = serde_json::json!({
        "scope_kind": "workspace",
        "template_provider": "core",
        "template_code": "custom",
        "template_version": "v1",
        "code": "items",
        "title": "Items"
    });
    let mut unknown = valid.clone();
    unknown["unknown"] = true.into();
    let mut invalid = valid.clone();
    invalid["title"] = 42.into();
    let cases = [
        (Some("application/json"), valid.to_string(), 4096, 200),
        (
            Some("application/problem+json"),
            valid.to_string(),
            4096,
            200,
        ),
        (None, valid.to_string(), 4096, 415),
        (Some("text/plain"), valid.to_string(), 4096, 415),
        (Some("application/json"), "{".to_owned(), 4096, 400),
        (Some("application/json"), unknown.to_string(), 4096, 422),
        (Some("application/json"), invalid.to_string(), 4096, 422),
        (Some("application/json"), valid.to_string(), 4, 413),
    ];
    for (content_type, body, limit, expected) in cases {
        let observe = |router, request| observe(router, request, expected);
        let old = Router::new()
            .route("/", post(old_body))
            .layer(DefaultBodyLimit::max(limit));
        let new = Router::new()
            .route("/", post(new_body))
            .layer(DefaultBodyLimit::max(limit));
        let request = || {
            let mut builder = Request::builder().method("POST").uri("/");
            if let Some(content_type) = content_type {
                builder = builder.header("content-type", content_type);
            }
            builder.body(Body::from(body.clone())).unwrap()
        };
        assert_eq!(observe(old, request()).await, observe(new, request()).await);
    }
}

#[tokio::test]
async fn query_boundary_preserves_original_axum_status_headers_bytes_and_error_paths() {
    for (uri, expected) in [
        ("/?data_source_id=main", 200),
        ("/?data_source_id=main&filter=a+b%2Fc", 200),
        ("/?data_source_id=a&data_source_id=b", 400),
    ] {
        let observe = |router, request| observe(router, request, expected);
        let old = Router::new().route("/", get(old_query));
        let new = Router::new().route("/", get(new_query));
        let request = || Request::builder().uri(uri).body(Body::empty()).unwrap();
        assert_eq!(observe(old, request()).await, observe(new, request()).await);
    }
}
