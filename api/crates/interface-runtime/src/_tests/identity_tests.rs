use crate::{IdentityError, RouteIdentity};

#[test]
fn route_identity_accepts_static_external_protocol_paths() {
    for path in [
        "/v1/chat/completions",
        "/chat/completions",
        "/v1/responses",
        "/responses",
        "/v1/responses/compact",
        "/v1/messages",
        "/api/ex/*slug",
    ] {
        let route = RouteIdentity::new("POST", path)
            .unwrap_or_else(|error| panic!("static protocol path {path} was rejected: {error}"));
        assert_eq!(route.path(), path);
    }
}

#[test]
fn route_identity_rejects_non_absolute_or_request_specific_paths() {
    for path in [
        "v1/responses",
        "//upstream/responses",
        "/v1/responses?stream=true",
        "/v1/responses#fragment",
        "/v1/response path",
        "/v1/response\npath",
    ] {
        assert_eq!(
            RouteIdentity::new("POST", path),
            Err(IdentityError::InvalidRoutePath),
            "invalid route path was accepted: {path:?}"
        );
    }
}
