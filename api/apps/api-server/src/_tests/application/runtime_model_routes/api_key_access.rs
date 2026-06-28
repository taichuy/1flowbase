use super::*;

#[tokio::test]
async fn runtime_model_routes_reject_legacy_data_model_api_key() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_code = "legacy_dmk_rejected_orders";
    let model_id =
        create_model_with_status(&app, &cookie, &csrf, model_code, Some("published")).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;
    create_runtime_record(&app, &cookie, &csrf, model_code, "visible").await;

    let (status, payload) =
        list_records_with_api_key(&app, model_code, "dmk_legacy_runtime_token").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(payload["code"], json!("not_authenticated"));
}

#[tokio::test]
async fn runtime_model_routes_user_api_key_uses_bound_user_role_permissions() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_code = "pat_user_role_orders";
    let model_id =
        create_model_with_status(&app, &cookie, &csrf, model_code, Some("published")).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;
    create_runtime_record(&app, &cookie, &csrf, model_code, "pat-visible").await;

    let token = create_user_api_key(&app, &cookie, &csrf, "runtime pat").await;

    let (status, payload) = list_records_with_api_key(&app, model_code, &token).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["data"]["total"], json!(1));
    assert_eq!(payload["data"]["items"][0]["title"], json!("pat-visible"));
}
