use axum::http::{header::HOST, uri::Authority, HeaderMap};
use control_plane::mcp_oauth::{validate_origin, OAuthError};

/// The ingress preserves Host and overwrites X-Forwarded-Proto. Never use a
/// caller-supplied Origin or X-Forwarded-Host to choose the public origin.
pub(crate) fn request_origin(headers: &HeaderMap) -> Result<String, OAuthError> {
    if headers.get_all(HOST).iter().count() != 1
        || headers.get_all("x-forwarded-proto").iter().count() > 1
    {
        return Err(OAuthError::invalid());
    }
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(OAuthError::invalid)?;
    if host.contains(',') {
        return Err(OAuthError::invalid());
    }
    host.parse::<Authority>()
        .map_err(|_| OAuthError::invalid())?;
    let local_http = validate_origin(&format!("http://{host}"), false).is_ok();
    let scheme = match headers.get("x-forwarded-proto") {
        Some(value) => match value.to_str().map_err(|_| OAuthError::invalid())? {
            "https" => "https",
            "http" if local_http => "http",
            _ => return Err(OAuthError::invalid()),
        },
        None if local_http => "http",
        None => "https",
    };
    validate_origin(&format!("{scheme}://{host}"), false).map_err(|_| OAuthError::invalid())
}

#[cfg(test)]
#[path = "_tests/origin.rs"]
mod tests;

/// The explicit origin is request-scoped, never a global deployment setting.
#[derive(Clone, Debug)]
pub(crate) struct PublicOrigin {
    pub origin: String,
    pub explicit: bool,
}
impl PublicOrigin {
    pub fn issuer(&self) -> String {
        if self.explicit {
            control_plane::mcp_oauth::public_authorization_server_url(&self.origin)
        } else {
            control_plane::mcp_oauth::authorization_server_url(&self.origin)
        }
    }
    pub fn resource(&self, instance: &str) -> Result<String, OAuthError> {
        if self.explicit {
            control_plane::mcp_oauth::public_resource_url(&self.origin, instance)
        } else {
            control_plane::mcp_oauth::resource_url(&self.origin, instance)
        }
    }
    pub fn metadata(&self, instance: &str) -> Result<String, OAuthError> {
        if self.explicit {
            control_plane::mcp_oauth::public_resource_metadata_url(&self.origin, instance)
        } else {
            control_plane::mcp_oauth::resource_metadata_url(&self.origin, instance)
        }
    }
}
pub(crate) fn request_public_origin(
    headers: &HeaderMap,
    uri: &axum::http::Uri,
) -> Result<PublicOrigin, OAuthError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let values = url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .filter(|(name, _)| name == "origin")
        .map(|(_, value)| value.into_owned())
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(OAuthError::invalid());
    }
    let path_origin = uri
        .path()
        .split("/mcp-oauth/origins/")
        .nth(1)
        .map(|rest| rest.split('/').next().unwrap_or(""))
        .map(|encoded| {
            if encoded.len() > 4096 {
                return Err(OAuthError::invalid());
            }
            let decoded = URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| OAuthError::invalid())?;
            let value = String::from_utf8(decoded).map_err(|_| OAuthError::invalid())?;
            if URL_SAFE_NO_PAD.encode(value.as_bytes()) != encoded {
                return Err(OAuthError::invalid());
            }
            Ok(value)
        })
        .transpose()?;
    if let Some(value) = path_origin.as_ref().or(values.first()) {
        let origin = validate_origin(value, false).map_err(|_| OAuthError::invalid())?;
        if path_origin.as_ref().is_some_and(|p| p != &origin)
            || values
                .first()
                .is_some_and(|q| validate_origin(q, false).ok().as_ref() != Some(&origin))
        {
            return Err(OAuthError::invalid());
        }
        return Ok(PublicOrigin {
            origin,
            explicit: true,
        });
    }
    Ok(PublicOrigin {
        origin: request_origin(headers)?,
        explicit: false,
    })
}
