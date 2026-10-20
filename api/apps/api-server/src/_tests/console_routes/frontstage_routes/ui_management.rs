use super::*;

pub(super) async fn set_ui_policy(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    role: &str,
    enabled: bool,
    strategy: &str,
    operations: Value,
) {
    let (status, payload) = send_json(app, "PUT", &format!("/api/console/settings/roles/{role}/console-policy"), cookie, csrf, json!({
        "groups": [{ "kind": "settings_feature", "group_id": "system.ui-management", "enabled": enabled, "strategy": strategy, "operations": operations }]
    })).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{payload}");
}
