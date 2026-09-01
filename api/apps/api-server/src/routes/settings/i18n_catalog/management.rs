use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use control_plane::{
    i18n_catalog::management::CatalogManagementAccess,
    ports::{CatalogManagementEntry, CatalogManagementOrigin},
};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogTranslation, WorkspaceCatalogRevision};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

use super::invalid_input;

pub(super) const DEFAULT_PAGE_LIMIT: u32 = 50;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CatalogManagementOriginDto {
    Official,
    OfficialOverride,
    Custom,
    English,
}

impl From<CatalogManagementOriginDto> for CatalogManagementOrigin {
    fn from(value: CatalogManagementOriginDto) -> Self {
        match value {
            CatalogManagementOriginDto::Official => Self::Official,
            CatalogManagementOriginDto::OfficialOverride => Self::OfficialOverride,
            CatalogManagementOriginDto::Custom => Self::Custom,
            CatalogManagementOriginDto::English => Self::English,
        }
    }
}

impl From<CatalogManagementOrigin> for CatalogManagementOriginDto {
    fn from(value: CatalogManagementOrigin) -> Self {
        match value {
            CatalogManagementOrigin::Official => Self::Official,
            CatalogManagementOrigin::OfficialOverride => Self::OfficialOverride,
            CatalogManagementOrigin::Custom => Self::Custom,
            CatalogManagementOrigin::English => Self::English,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListCatalogEntriesQuery {
    pub key: Option<String>,
    pub locale: Option<String>,
    pub search: Option<String>,
    pub origin: Option<CatalogManagementOriginDto>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetCatalogEntryQuery {
    pub key: String,
    pub locale: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertCatalogTranslationBody {
    pub key: String,
    pub locale: String,
    pub translation: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreCatalogOverrideBody {
    pub key: String,
    pub locale: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteCustomCatalogKeyBody {
    pub key: String,
    pub expected_revision: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreCatalogOverridesBody {
    pub expected_revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogManagementEntryResponse {
    pub key: String,
    pub locale: String,
    pub official_translation: Option<String>,
    pub override_translation: Option<String>,
    pub custom_translation: Option<String>,
    pub effective_value: String,
    pub origin: CatalogManagementOriginDto,
    pub missing: bool,
    pub obsolete: bool,
    pub revision: i64,
}

impl From<CatalogManagementEntry> for CatalogManagementEntryResponse {
    fn from(entry: CatalogManagementEntry) -> Self {
        Self {
            key: entry.key,
            locale: entry.locale.as_str().to_owned(),
            official_translation: entry.official_translation,
            override_translation: entry.override_translation,
            custom_translation: entry.custom_translation,
            effective_value: entry.effective_value,
            origin: entry.origin.into(),
            missing: entry.missing,
            obsolete: entry.obsolete,
            revision: entry.revision.value(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogManagementPageResponse {
    pub entries: Vec<CatalogManagementEntryResponse>,
    pub total: u64,
    pub revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogEntryMutationResponse {
    pub revision: i64,
    pub entry: CatalogManagementEntryResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogRevisionResponse {
    pub revision: i64,
}

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/i18n/entries",
            console_get(
                list_catalog_entries,
                ConsoleOperation("i18n_catalog.entries.list".to_string()),
            ),
        )
        .route(
            "/settings/i18n/entries/detail",
            console_get(
                get_catalog_entry,
                ConsoleOperation("i18n_catalog.entries.detail".to_string()),
            ),
        )
        .route(
            "/settings/i18n/overrides",
            console_put(
                upsert_catalog_override,
                ConsoleOperation("i18n_catalog.overrides.upsert".to_string()),
            )
            .delete(
                restore_catalog_override,
                ConsoleOperation("i18n_catalog.overrides.restore".to_string()),
            ),
        )
        .route(
            "/settings/i18n/custom-translations",
            console_put(
                upsert_custom_catalog_translation,
                ConsoleOperation("i18n_catalog.custom_translations.upsert".to_string()),
            ),
        )
        .route(
            "/settings/i18n/custom-keys",
            console_delete(
                delete_custom_catalog_key,
                ConsoleOperation("i18n_catalog.custom_keys.delete".to_string()),
            ),
        )
        .route(
            "/settings/i18n/restore-overrides",
            console_post(
                restore_all_catalog_overrides,
                ConsoleOperation("i18n_catalog.overrides.restore_all".to_string()),
            ),
        )
}

pub(super) fn access(actor: domain::ActorContext) -> CatalogManagementAccess {
    CatalogManagementAccess {
        current_workspace_id: actor.current_workspace_id,
        actor,
    }
}

pub(super) fn identity(key: String) -> Result<CatalogMessageIdentity, ApiError> {
    CatalogMessageIdentity::new(key).map_err(|_| invalid_input("i18n_catalog_key"))
}

pub(super) fn locale(value: String) -> Result<CatalogLocale, ApiError> {
    CatalogLocale::new(value).map_err(|_| invalid_input("i18n_catalog_locale"))
}

pub(super) fn revision(value: i64) -> Result<WorkspaceCatalogRevision, ApiError> {
    WorkspaceCatalogRevision::new(value).map_err(|_| invalid_input("expected_revision"))
}

pub(super) fn translation(
    body: &UpsertCatalogTranslationBody,
) -> Result<CatalogTranslation, ApiError> {
    CatalogTranslation::new(
        identity(body.key.clone())?,
        locale(body.locale.clone())?,
        body.translation.clone(),
    )
    .map_err(|_| invalid_input("i18n_catalog_translation"))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/entries",
    summary = "List i18n catalog management entries",
    description = "Lists the root i18n catalog management projection with key and locale filters, key or effective-translation search, and pagination.",
    params(ListCatalogEntriesQuery),
    responses((status = 200, body = CatalogManagementPageResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog_entries(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ListCatalogEntriesQuery>,
) -> Result<Json<ApiSuccess<CatalogManagementPageResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::Entries(response) = super::invoke(
        state,
        headers,
        "http.console.i18n.entries.list.get.v1",
        super::interface::I18nCatalogInput::ListEntries(query),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/entries/detail",
    summary = "Get an i18n catalog management entry",
    description = "Returns one root i18n catalog management projection identified by key and locale.",
    params(GetCatalogEntryQuery),
    responses((status = 200, body = CatalogManagementEntryResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn get_catalog_entry(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<GetCatalogEntryQuery>,
) -> Result<Json<ApiSuccess<CatalogManagementEntryResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::Entry(response) = super::invoke(
        state,
        headers,
        "http.console.i18n.entries.detail.get.v1",
        super::interface::I18nCatalogInput::GetEntry(query),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/i18n/overrides",
    summary = "Upsert an official i18n catalog override",
    description = "Creates or replaces one root override for an official catalog key and locale at the expected revision.",
    request_body = UpsertCatalogTranslationBody,
    responses((status = 200, body = CatalogEntryMutationResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn upsert_catalog_override(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<UpsertCatalogTranslationBody>,
) -> Result<Json<ApiSuccess<CatalogEntryMutationResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::EntryMutation(response) = super::invoke_mutating(
        state,
        headers,
        "http.console.i18n.overrides.put.v1",
        super::interface::I18nCatalogInput::UpsertOfficialOverride(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/i18n/overrides",
    summary = "Restore an official i18n catalog translation",
    description = "Removes one root override and restores the official key and locale translation at the expected revision.",
    request_body = RestoreCatalogOverrideBody,
    responses((status = 200, body = CatalogEntryMutationResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn restore_catalog_override(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<RestoreCatalogOverrideBody>,
) -> Result<Json<ApiSuccess<CatalogEntryMutationResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::EntryMutation(response) = super::invoke_mutating(
        state,
        headers,
        "http.console.i18n.overrides.delete.v1",
        super::interface::I18nCatalogInput::RestoreOfficialOverride(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/i18n/custom-translations",
    summary = "Upsert a custom i18n catalog translation",
    description = "Creates or replaces one custom root catalog key and locale translation at the expected revision.",
    request_body = UpsertCatalogTranslationBody,
    responses((status = 200, body = CatalogEntryMutationResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn upsert_custom_catalog_translation(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<UpsertCatalogTranslationBody>,
) -> Result<Json<ApiSuccess<CatalogEntryMutationResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::EntryMutation(response) = super::invoke_mutating(
        state,
        headers,
        "http.console.i18n.custom-translations.put.v1",
        super::interface::I18nCatalogInput::UpsertCustomTranslation(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/i18n/custom-keys",
    summary = "Delete a custom i18n catalog key",
    description = "Deletes one custom root catalog key and all of its translations at the expected revision.",
    request_body = DeleteCustomCatalogKeyBody,
    responses((status = 200, body = CatalogRevisionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn delete_custom_catalog_key(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<DeleteCustomCatalogKeyBody>,
) -> Result<Json<ApiSuccess<CatalogRevisionResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::Revision(response) = super::invoke_mutating(
        state,
        headers,
        "http.console.i18n.custom-keys.delete.v1",
        super::interface::I18nCatalogInput::DeleteCustomKey(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/i18n/restore-overrides",
    summary = "Restore all official i18n catalog overrides",
    description = "Removes all official root catalog overrides while retaining custom keys and translations.",
    request_body = RestoreCatalogOverridesBody,
    responses((status = 200, body = CatalogRevisionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn restore_all_catalog_overrides(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<RestoreCatalogOverridesBody>,
) -> Result<Json<ApiSuccess<CatalogRevisionResponse>>, ApiError> {
    let super::interface::I18nCatalogOutput::Revision(response) = super::invoke_mutating(
        state,
        headers,
        "http.console.i18n.restore-overrides.post.v1",
        super::interface::I18nCatalogInput::RestoreAllOfficialOverrides(body),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[cfg(test)]
mod tests {
    use super::identity;

    #[test]
    fn i18n_catalog_management_rejects_non_english_and_variable_keys() {
        for key in ["设置", "settings.title", "custom_key", "<b>Settings</b>"] {
            assert!(identity(key.to_owned()).is_err(), "{key:?}");
        }
        for key in ["Settings", "Save {name}", "API v2.0"] {
            assert!(identity(key.to_owned()).is_ok(), "{key:?}");
        }
    }
}
