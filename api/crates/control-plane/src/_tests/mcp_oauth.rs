use super::*;
#[test]
fn issuer_is_an_explicit_origin_and_cannot_be_a_host_header_or_path() {
    assert_eq!(
        validate_issuer("https://example.com/", true).unwrap(),
        "https://example.com"
    );
    assert!(validate_issuer("http://example.com", false).is_err());
    assert!(validate_issuer("http://localhost:8080", true).is_err());
    assert!(validate_issuer("http://localhost:8080", false).is_ok());
    for url in [
        "https://user@example.com",
        "https://example.com/path",
        "https://example.com?x=1",
        "https://example.com#frag",
    ] {
        assert!(validate_issuer(url, true).is_err());
    }
}
#[test]
fn chatgpt_redirect_allowlist_is_exact_and_has_no_fetch_surface() {
    assert!(allowed_redirect(
        "https://chatgpt.com/connector_platform_oauth_redirect"
    ));
    assert!(allowed_redirect(
        "https://chatgpt.com/connector/oauth/abc-123"
    ));
    for url in [
        "https://chatgpt.com.evil.test/connector/oauth/abc",
        "https://evil.test",
        "http://chatgpt.com/connector/oauth/abc",
        "https://chatgpt.com/connector/oauth/abc/extra",
        "https://chatgpt.com/connector/oauth/abc?next=evil",
        "https://chatgpt.com/connector/oauth/%2e%2e",
        "https://user@chatgpt.com/connector/oauth/abc",
        "https://chatgpt.com/connector/oauth/",
    ] {
        assert!(!allowed_redirect(url), "{url}");
    }
}
#[test]
fn resource_cannot_change_origin_or_escape_instance() {
    assert_eq!(
        resource_instance("https://example.com", "https://example.com/api/mcp/team").unwrap(),
        "team"
    );
    for resource in [
        "https://evil.test/api/mcp/team",
        "https://example.com/api/mcp/team/other",
        "https://example.com/api/mcp/../console",
        "https://example.com/api/mcp/team?x=1",
    ] {
        assert!(resource_instance("https://example.com", resource).is_err());
    }
}

#[test]
fn metadata_uri_encodes_instance_without_query_fragment_or_header_injection() {
    let issuer = "https://example.com";
    for instance in [
        "team?x",
        "team#x",
        "team/name",
        "team\"name",
        "team%name",
        "a+b",
        "team&other",
        "space name",
        "工作区",
    ] {
        let resource = resource_url(issuer, instance).unwrap();
        let metadata = resource_metadata_url(issuer, instance).unwrap();
        assert_eq!(
            metadata,
            format!(
                "{issuer}/.well-known/oauth-protected-resource{}",
                &resource[issuer.len()..]
            )
        );
        let url = url::Url::parse(&metadata).unwrap();
        assert!(url.query().is_none() && url.fragment().is_none());
        assert!(!metadata.contains('"'));
        assert_eq!(resource_instance(issuer, &resource).unwrap(), instance);
    }
    for instance in ["", ".", ".."] {
        assert!(resource_metadata_url(issuer, instance).is_err());
    }
}
