use super::*;

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/installed/{installation_id}/contribution-authorizations",
    operation_id = "extension_center_contribution_authorizations_grant",
    summary = "Grant installed contribution authorizations",
    request_body = GrantContributionPermissionRequest,
    responses((status = 200, body = ContributionAuthorizationResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub(super) async fn grant_contribution_authorizations(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<GrantContributionPermissionRequest>,
) -> Result<Json<ApiSuccess<ContributionAuthorizationResponse>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.contribution-authorizations.grant.v1",
        interface::ExtensionCenterInput::GrantContributionPermission(installation_id, request),
        true,
    )
    .await?;
    let interface::ExtensionCenterOutput::ContributionAuthorizations(snapshot) = output else {
        unreachable!("contribution authorization binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(snapshot)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/installed/{installation_id}/contribution-authorizations/revoke",
    operation_id = "extension_center_contribution_authorizations_revoke",
    summary = "Revoke installed contribution authorizations",
    request_body = RevokeContributionPermissionRequest,
    responses((status = 200, body = ContributionAuthorizationResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub(super) async fn revoke_contribution_authorizations(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<RevokeContributionPermissionRequest>,
) -> Result<Json<ApiSuccess<ContributionAuthorizationResponse>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.contribution-authorizations.revoke.v1",
        interface::ExtensionCenterInput::RevokeContributionPermission(installation_id, request),
        true,
    )
    .await?;
    let interface::ExtensionCenterOutput::ContributionAuthorizations(snapshot) = output else {
        unreachable!("contribution authorization binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(snapshot)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/extension-center/installed/{installation_id}/contribution-authorizations",
    operation_id = "extension_center_contribution_authorizations_view",
    summary = "View installed contribution authorizations",
    responses((status = 200, body = ContributionAuthorizationResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub(super) async fn view_contribution_authorizations(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ContributionAuthorizationResponse>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.contribution-authorizations.view.v1",
        interface::ExtensionCenterInput::QueryContributionAuthorizations(installation_id),
        false,
    )
    .await?;
    let interface::ExtensionCenterOutput::ContributionAuthorizations(snapshot) = output else {
        unreachable!("contribution authorization binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(snapshot)))
}
