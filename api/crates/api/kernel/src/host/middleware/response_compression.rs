use std::io::Write;

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use flate2::{write::GzEncoder, Compression};
use http_body::Body as HttpBody;

const MIN_COMPRESSIBLE_BYTES: u64 = 512;
const MAX_COMPRESSIBLE_BYTES: usize = 16 * 1024 * 1024;

/// Compresses bounded, non-streaming HTTP responses (`application/json`, `text/*` excluding
/// `text/event-stream`, `application/javascript`, `image/svg+xml`) using gzip when negotiated by
/// `Accept-Encoding`. Never buffers SSE, WebSocket upgrades, or unbounded stream bodies.
pub async fn compress_bounded_http_response(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let client_accepts_gzip = request
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .is_some_and(accepts_gzip_encoding);

    let mut response = next.run(request).await;
    let status = response.status();
    if method == Method::HEAD
        || status == StatusCode::SWITCHING_PROTOCOLS
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
        || status.is_informational()
        || response.headers().contains_key(header::UPGRADE)
    {
        return response;
    }

    if !is_compressible_content_type(response.headers()) {
        return response;
    }

    ensure_vary_accept_encoding(response.headers_mut());

    if !client_accepts_gzip || response.headers().contains_key(header::CONTENT_ENCODING) {
        return response;
    }

    // Streaming responses (SSE, chunked archive/file streams) report `upper() == None`.
    // Only compress bodies with a known, bounded size hint in `[512 B, 16 MiB]`.
    let Some(upper_bound) = response.body().size_hint().upper() else {
        return response;
    };
    if upper_bound < MIN_COMPRESSIBLE_BYTES || upper_bound > MAX_COMPRESSIBLE_BYTES as u64 {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let Ok(raw_bytes) = to_bytes(body, MAX_COMPRESSIBLE_BYTES).await else {
        return Response::from_parts(parts, Body::empty());
    };
    if (raw_bytes.len() as u64) < MIN_COMPRESSIBLE_BYTES {
        return Response::from_parts(parts, Body::from(raw_bytes));
    }

    let mut encoder = GzEncoder::new(Vec::with_capacity(raw_bytes.len() / 2), Compression::fast());
    let compressed = match encoder.write_all(&raw_bytes).and_then(|()| encoder.finish()) {
        Ok(buf) if buf.len() < raw_bytes.len() => buf,
        _ => return Response::from_parts(parts, Body::from(raw_bytes)),
    };

    parts.headers.insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("gzip"),
    );
    parts.headers.remove(header::TRANSFER_ENCODING);
    if let Ok(len_val) = HeaderValue::from_str(&compressed.len().to_string()) {
        parts.headers.insert(header::CONTENT_LENGTH, len_val);
    }

    Response::from_parts(parts, Body::from(compressed))
}

fn is_compressible_content_type(headers: &HeaderMap) -> bool {
    let Some(raw) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let media_type = raw
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if media_type == "text/event-stream" {
        return false;
    }
    media_type == "application/json"
        || media_type.ends_with("+json")
        || media_type.starts_with("text/")
        || media_type == "application/javascript"
        || media_type == "text/javascript"
        || media_type == "image/svg+xml"
        || media_type == "application/xml"
}

fn accepts_gzip_encoding(accept_encoding: &str) -> bool {
    for part in accept_encoding.split(',') {
        let mut segments = part.trim().split(';');
        let Some(coding) = segments.next().map(str::trim) else {
            continue;
        };
        if !coding.eq_ignore_ascii_case("gzip") && !coding.eq_ignore_ascii_case("x-gzip") {
            continue;
        }
        let mut rejected_by_qvalue = false;
        for param in segments {
            if let Some((key, val)) = param.trim().split_once('=') {
                if key.trim().eq_ignore_ascii_case("q")
                    && val
                        .trim()
                        .parse::<f32>()
                        .is_ok_and(|q| q <= 0.0)
                {
                    rejected_by_qvalue = true;
                }
            }
        }
        if !rejected_by_qvalue {
            return true;
        }
    }
    false
}

fn ensure_vary_accept_encoding(headers: &mut HeaderMap) {
    let already_present = headers
        .get_all(header::VARY)
        .iter()
        .filter_map(|val| val.to_str().ok())
        .flat_map(|val| val.split(','))
        .any(|token| {
            let trimmed = token.trim();
            trimmed == "*" || trimmed.eq_ignore_ascii_case("accept-encoding")
        });
    if already_present {
        return;
    }
    if let Some(existing) = headers
        .get(header::VARY)
        .and_then(|val| val.to_str().ok())
        .map(str::trim)
        .filter(|val| !val.is_empty())
        .map(ToOwned::to_owned)
    {
        if let Ok(merged) = HeaderValue::from_str(&format!("{existing}, Accept-Encoding")) {
            headers.insert(header::VARY, merged);
            return;
        }
    }
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
}

#[cfg(test)]
#[path = "_tests/response_compression.rs"]
mod tests;
