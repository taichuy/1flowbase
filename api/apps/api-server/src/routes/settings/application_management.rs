use std::sync::Arc;

use axum::{extract::Query, extract::State, http::HeaderMap, Json, Router};
use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    ports::{
        ApplicationManagementPage, ApplicationManagementQuery, ApplicationManagementRecord,
        ApplicationManagementSortDirection, ApplicationManagementSortField,
    },
    resource_crud::{parse_resource_filter, parse_resource_filter_expr},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[derive(Debug, Deserialize)]
pub struct ApplicationManagementQueryParams {
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationManagementTagResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationManagementItemResponse {
    pub id: String,
    pub application_type: String,
    pub workflow_trigger_type: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: String,
    pub created_by_display_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<ApplicationManagementTagResponse>,
    pub publication_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationManagementPageResponse {
    pub items: Vec<ApplicationManagementItemResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

impl InterfaceContract for ApplicationManagementQueryParams {
    const CONTRACT_ID: &'static str = "console-application-management-input";
    const CONTRACT_VERSION: &'static str = "1";
}
impl InterfaceContract for ApplicationManagementPageResponse {
    const CONTRACT_ID: &'static str = "console-application-management-output";
    const CONTRACT_VERSION: &'static str = "1";
}
struct ApplicationManagementAdapter(storage_durable_postgres::MainDurableStore);
impl ConsoleInterfacePort<ApplicationManagementQueryParams, ApplicationManagementPageResponse>
    for ApplicationManagementAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        query: ApplicationManagementQueryParams,
    ) -> ConsoleInterfaceFuture<'a, ApplicationManagementPageResponse> {
        Box::pin(async move {
            let filter = match parse_resource_filter(query.filter.as_deref())
                .map_err(ApiError::from)
                .map_err(ConsoleInterfaceTargetError)?
            {
                Some(filter) => parse_resource_filter_expr(&filter)
                    .map_err(ApiError::from)
                    .map_err(ConsoleInterfaceTargetError)?,
                None => domain::ResourceFilterExpr::All(Vec::new()),
            };
            let (sort_field, sort_direction) =
                parse_application_management_sort(query.sort.as_deref())
                    .map_err(ConsoleInterfaceTargetError)?;
            let page = ApplicationService::new(self.0.for_actor(principal.actor().clone()))
                .list_application_management(
                    principal.actor().user_id,
                    ApplicationManagementQuery {
                        filter,
                        sort_field,
                        sort_direction,
                        page: query.page.unwrap_or(1).max(1),
                        page_size: query.page_size.unwrap_or(20).clamp(1, 100),
                    },
                )
                .await
                .map_err(ApiError::from)
                .map_err(ConsoleInterfaceTargetError)?;
            page.try_into()
                .map_err(ApiError::from)
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) fn compile_registry(
    store: storage_durable_postgres::MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
        interface_id: access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
        binding_id: "http.console.settings.applications.get.v1",
        method: "GET",
        path: "/api/console/settings/applications",
        mutating: false,
    }];
    console_interface::compile_registry(
        "api-server.console-application-management",
        "graph:console-application-management-v1",
        DECLARATIONS,
        Arc::new(ApplicationManagementAdapter(store)),
    )
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new().route(
        "/settings/applications",
        console_get(
            list_application_management,
            access_control::ConsoleRouteOwnership::ConsoleOperation(
                access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION.to_string(),
            ),
        ),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/applications",
    responses(
        (status = 200, body = ApplicationManagementPageResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_management(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ApplicationManagementQueryParams>,
) -> Result<Json<ApiSuccess<ApplicationManagementPageResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let value = console_interface::invoke(
        snapshot_state,
        "http.console.settings.applications.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        query,
    )
    .await?;
    Ok(Json(ApiSuccess::new(value)))
}

fn parse_application_management_sort(
    sort: Option<&str>,
) -> Result<
    (
        ApplicationManagementSortField,
        ApplicationManagementSortDirection,
    ),
    ApiError,
> {
    let Some(sort) = sort.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((
            ApplicationManagementSortField::UpdatedAt,
            ApplicationManagementSortDirection::Desc,
        ));
    };
    let (field, direction) = sort
        .split_once(':')
        .ok_or(ControlPlaneError::InvalidInput("sort"))?;
    let field = match field {
        "updated_at" => ApplicationManagementSortField::UpdatedAt,
        "created_at" => ApplicationManagementSortField::CreatedAt,
        "name" => ApplicationManagementSortField::Name,
        "application_type" => ApplicationManagementSortField::ApplicationType,
        _ => return Err(ControlPlaneError::InvalidInput("sort").into()),
    };
    let direction = match direction {
        "asc" => ApplicationManagementSortDirection::Asc,
        "desc" => ApplicationManagementSortDirection::Desc,
        _ => return Err(ControlPlaneError::InvalidInput("sort").into()),
    };

    Ok((field, direction))
}

impl TryFrom<ApplicationManagementRecord> for ApplicationManagementItemResponse {
    type Error = time::error::Format;

    fn try_from(record: ApplicationManagementRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: record.id.to_string(),
            application_type: record.application_type.as_str().to_string(),
            workflow_trigger_type: record
                .workflow_trigger_type
                .map(|value| value.as_str().to_string()),
            name: record.name,
            description: record.description,
            icon: record.icon,
            icon_type: record.icon_type,
            icon_background: record.icon_background,
            created_by: record.created_by.to_string(),
            created_by_display_name: record.created_by_display_name,
            created_at: record.created_at.format(&Rfc3339)?,
            updated_at: record.updated_at.format(&Rfc3339)?,
            tags: record
                .tags
                .into_iter()
                .map(|tag| ApplicationManagementTagResponse {
                    id: tag.id.to_string(),
                    name: tag.name,
                })
                .collect(),
            publication_status: record.publication_status.as_str().to_string(),
        })
    }
}

impl TryFrom<ApplicationManagementPage> for ApplicationManagementPageResponse {
    type Error = time::error::Format;

    fn try_from(page: ApplicationManagementPage) -> Result<Self, Self::Error> {
        Ok(Self {
            items: page
                .items
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            total: page.total,
            page: page.page,
            page_size: page.page_size,
        })
    }
}
