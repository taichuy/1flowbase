use axum::{
    http::{header::CACHE_CONTROL, HeaderValue},
    response::Response,
};

/// Console data is actor-scoped and mutable. A proxy must not infer a shared
/// cache lifetime when an endpoint has not declared its own cache policy.
/// Preserve explicit policies such as the private, revalidated block-tree ETag.
pub async fn prevent_implicit_console_caching(mut response: Response) -> Response {
    response
        .headers_mut()
        .entry(CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("private, no-store"));
    response
}

#[cfg(test)]
#[path = "_tests/console_response_cache.rs"]
mod tests;
