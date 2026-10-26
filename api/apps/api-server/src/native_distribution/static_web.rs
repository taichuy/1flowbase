use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::Request,
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use tower::ServiceExt;
use tower_http::services::ServeFile;

pub fn mount(router: Router, root: &Path) -> anyhow::Result<Router> {
    let root = root.canonicalize()?;
    anyhow::ensure!(
        root.join("index.html").is_file(),
        "web/index.html is missing"
    );
    // The host's existing fallback owns canonical/dynamic API dispatch. Never replace it.
    Ok(Router::new().fallback(move |request: Request| {
        let backend = router.clone();
        let root = root.clone();
        async move {
            let mut static_request = Request::builder()
                .method(request.method().clone())
                .uri(request.uri().clone())
                .body(Body::empty())
                .expect("existing HTTP request is valid");
            *static_request.headers_mut() = request.headers().clone();
            let response = match backend.oneshot(request).await {
                Ok(response) => response,
                Err(error) => match error {},
            };
            if response.status() != StatusCode::NOT_FOUND {
                return response;
            }
            let file = serve(root, static_request).await;
            if file.status() == StatusCode::NOT_FOUND {
                response
            } else {
                file
            }
        }
    }))
}

async fn serve(root: PathBuf, request: Request) -> Response {
    let method = request.method();
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(path) = percent_encoding::percent_decode_str(request.uri().path()).decode_utf8() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let relative = path.trim_start_matches('/');
    // Never turn an unknown protocol endpoint into an HTML success response.
    if ["api", "v1", ".well-known", "health", "openapi.json"]
        .iter()
        .any(|prefix| relative == *prefix || relative.starts_with(&format!("{prefix}/")))
        || relative.contains(['\\', '\0', ':'])
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
    let target = match candidate.canonicalize() {
        Ok(target) if target.starts_with(&root) && target.is_file() => target,
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
            root.join("index.html")
        }
    };
    // The SPA index must also stay inside the distribution (including symlinks).
    let Ok(target) = target.canonicalize() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !target.starts_with(&root) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let html = target
        .extension()
        .is_some_and(|extension| extension == "html");
    match ServeFile::new(target).oneshot(request).await {
        Ok(response) => {
            let mut response = response.map(Body::new);
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                if html {
                    "no-cache"
                } else {
                    "public, max-age=0, must-revalidate"
                }
                .parse()
                .unwrap(),
            );
            response
        }
        Err(error) => match error {},
    }
}
