use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use tower::ServiceExt;
use tower_http::services::ServeFile;

pub fn mount(router: Router, root: &Path) -> anyhow::Result<Router> {
    let root = root.canonicalize()?;
    let index_file = root.join("index.html");
    anyhow::ensure!(index_file.is_file(), "web/index.html is missing");
    let index_target = index_file.canonicalize()?;
    anyhow::ensure!(
        index_target.starts_with(&root),
        "web/index.html escapes distribution root"
    );

    // The host's existing fallback owns canonical/dynamic API dispatch. Never replace it.
    // Static bundle fast-paths (/assets/*, /external-npm/*, /icon.svg, etc.) serve directly
    // from precompressed disk assets without traversing the API router at all (including fast
    // 404 on missing hashed assets), while non-static paths keep backend dispatch first.
    Ok(Router::new().fallback(move |request: Request| {
        let backend = router.clone();
        let root = root.clone();
        let index_target = index_target.clone();
        async move {
            if is_static_fast_path_candidate(&request) {
                return serve(&root, &index_target, request).await;
            }

            let static_request = clone_empty_request(&request);
            let response = match backend.oneshot(request).await {
                Ok(response) => response,
                Err(error) => match error {},
            };
            if response.status() != StatusCode::NOT_FOUND {
                return response;
            }
            let file = serve(&root, &index_target, static_request).await;
            if file.status() == StatusCode::NOT_FOUND {
                response
            } else {
                file
            }
        }
    }))
}

fn clone_empty_request(request: &Request) -> Request {
    let mut cloned = Request::builder()
        .method(request.method().clone())
        .uri(request.uri().clone())
        .body(Body::empty())
        .expect("existing HTTP request is valid");
    *cloned.headers_mut() = request.headers().clone();
    cloned
}

fn is_static_fast_path_candidate(request: &Request) -> bool {
    let method = request.method();
    if method != Method::GET && method != Method::HEAD {
        return false;
    }
    let path = request.uri().path().trim_start_matches('/');
    path.starts_with("assets/")
        || path.starts_with("external-npm/")
        || path.starts_with("vditor/")
        || path.starts_with("icons/")
        || matches!(
            path,
            "icon.svg" | "authenticated-prewarm.json" | "scenario-chunk-manifest.json"
        )
}

async fn serve(root: &Path, index_target: &Path, request: Request) -> Response {
    let method = request.method();
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(path) = percent_encoding::percent_decode_str(request.uri().path()).decode_utf8() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let relative = path.trim_start_matches('/');
    // Never turn an unknown protocol or probe endpoint into an HTML success response.
    if [
        "api",
        "v1",
        ".well-known",
        "health",
        "healthz",
        "readyz",
        "metrics",
        "openapi.json",
    ]
    .iter()
    .any(|prefix| relative == *prefix || relative.starts_with(&format!("{prefix}/")))
        || relative.contains(['\\', ':'])
        || relative.chars().any(char::is_control)
        || relative
            .split('/')
            .any(|part| part == ".." || part.starts_with('.'))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let candidate = root.join(relative);
    let candidate = if candidate.is_dir() {
        candidate.join("index.html")
    } else {
        candidate
    };
    let (target, served_relative): (PathBuf, &str) = match candidate.canonicalize() {
        Ok(target) if target.starts_with(root) && target.is_file() => (target, relative),
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => {
            let navigation = request
                .headers()
                .get(header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("text/html"));
            if !navigation
                || Path::new(relative).extension().is_some()
                || relative.starts_with("external-npm/")
                || relative.starts_with("assets/")
            {
                return StatusCode::NOT_FOUND.into_response();
            }
            (index_target.to_path_buf(), "index.html")
        }
    };

    let cache_control = classify_cache_control(&target, served_relative);
    let service = ServeFile::new(target)
        .precompressed_br()
        .precompressed_gzip();
    match service.oneshot(request).await {
        Ok(response) => {
            let mut response = response.map(Body::new);
            apply_default_cache_control(&mut response, cache_control);
            response
        }
        Err(error) => match error {},
    }
}

pub(crate) fn apply_default_cache_control(response: &mut Response, cache_control: &'static str) {
    if !response.headers().contains_key(header::CACHE_CONTROL) {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(cache_control),
        );
    }
}

fn classify_cache_control(target: &Path, relative: &str) -> &'static str {
    if target
        .extension()
        .is_some_and(|extension| extension == "html")
    {
        // Content-hashed HTML fragments under /assets/ remain immutable; SPA documents are no-cache.
        if is_vite_hashed_asset(relative) {
            return "public, max-age=31536000, immutable";
        }
        return "no-cache";
    }
    if is_immutable_static_asset(relative) {
        return "public, max-age=31536000, immutable";
    }
    if relative == "icon.svg" {
        return "public, max-age=86400";
    }
    if matches!(
        relative,
        "external-npm/manifest.json"
            | "authenticated-prewarm.json"
            | "scenario-chunk-manifest.json"
    ) {
        return "no-cache";
    }
    "public, max-age=0, must-revalidate"
}

fn is_immutable_static_asset(relative: &str) -> bool {
    if relative.starts_with("external-npm/assets/") {
        return true;
    }
    if let Some(stem) = relative
        .strip_prefix("assets/native-declarations/")
        .and_then(|rest| rest.strip_suffix(".json"))
    {
        return stem.len() == 64 && stem.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    is_vite_hashed_asset(relative)
}

fn is_vite_hashed_asset(relative: &str) -> bool {
    let Some(file_name) = relative.strip_prefix("assets/") else {
        return false;
    };
    if file_name.contains('/') {
        return false;
    }
    let Some((stem, ext)) = file_name.rsplit_once('.') else {
        return false;
    };
    if !matches!(
        ext,
        "js" | "mjs" | "css" | "html" | "svg" | "json" | "woff" | "woff2" | "ttf" | "wasm"
    ) {
        return false;
    }
    if stem.len() <= 9 || stem.as_bytes()[stem.len() - 9] != b'-' {
        return false;
    }
    stem.as_bytes()[stem.len() - 8..]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
