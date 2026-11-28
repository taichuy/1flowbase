use super::*;
use crate::_tests::support::{create_member, create_role, replace_member_roles};

// Use the real session, compiled grants and HTTP bindings, not a mocked root.
pub(super) async fn assert_page_and_detail_grants(app: &axum::Router, root: &str, csrf: &str) {
    let role = "mcp_point_reader";
    let member = create_member(app, root, csrf, role, "temp-pass").await;
    create_role(app, root, csrf, role).await;
    replace_member_roles(app, root, csrf, &member, &[role]).await;
    for (grant, allowed, denied) in [
        (
            "list_mcp_tools_page",
            "/api/console/mcp/tools/page",
            "/api/console/mcp/tools/by-id?tool_id=page",
        ),
        (
            "get_mcp_tool",
            "/api/console/mcp/tools/by-id?tool_id=page",
            "/api/console/mcp/tools/page",
        ),
    ] {
        let policy = json!({"groups":[{"kind":"settings_feature","group_id":"system.mcp-management","enabled":true,"strategy":"custom","operations":[{"kind":"simple","operation_id":grant,"enabled":true}]}]});
        let saved = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/console/settings/roles/{role}/console-policy"))
                    .header("cookie", root)
                    .header("x-csrf-token", csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(policy.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), StatusCode::NO_CONTENT);
        let (cookie, token) = login_and_capture_cookie(app, role, "temp-pass").await;
        assert_eq!(
            mcp_get(app, &cookie, allowed).await.status(),
            StatusCode::OK,
            "{grant}: {allowed}"
        );
        assert_eq!(
            mcp_get(app, &cookie, denied).await.status(),
            StatusCode::FORBIDDEN,
            "{grant}: {denied}"
        );
        let denied_delete = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/console/mcp/tools/page")
                    .header("cookie", &cookie)
                    .header("x-csrf-token", token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied_delete.status(), StatusCode::FORBIDDEN);
    }
}
