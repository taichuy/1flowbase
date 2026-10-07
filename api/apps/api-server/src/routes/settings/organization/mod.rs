use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_put, ConsoleRouteAssembly,
    },
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
pub(crate) mod interface;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveDepartmentBody {
    pub name: String,
    pub parent_id: Option<String>,
    pub role_codes: Vec<String>,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceMemberDepartmentsBody {
    pub department_ids: Vec<String>,
    pub primary_department_id: Option<String>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct DepartmentResponse {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub role_codes: Vec<String>,
    pub member_count: i64,
}
impl From<domain::Department> for DepartmentResponse {
    fn from(d: domain::Department) -> Self {
        Self {
            id: d.id.to_string(),
            name: d.name,
            parent_id: d.parent_id.map(|id| id.to_string()),
            role_codes: d.role_codes,
            member_count: d.member_count,
        }
    }
}
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DepartmentListQuery {
    pub parent_id: Option<uuid::Uuid>,
    pub prefix: Option<String>,
    /// Comma-separated department IDs for resolving saved selections.
    pub ids: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}
impl DepartmentListQuery {
    fn into_input(self) -> Result<control_plane::ports::DepartmentListInput, ApiError> {
        let ids = self
            .ids
            .map(|ids| {
                ids.split(',')
                    .map(|id| {
                        uuid::Uuid::parse_str(id.trim()).map_err(|_| {
                            control_plane::errors::ControlPlaneError::InvalidInput("ids")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        Ok(control_plane::ports::DepartmentListInput {
            parent_id: self.parent_id,
            prefix: self.prefix,
            ids,
            limit: self.limit.unwrap_or(100),
            cursor: self.cursor,
        })
    }
}
#[derive(Debug, Serialize, ToSchema)]
pub struct DepartmentTreeItemResponse {
    #[serde(flatten)]
    pub department: DepartmentResponse,
    pub has_children: bool,
    pub is_match: bool,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct DepartmentPageResponse {
    pub items: Vec<DepartmentTreeItemResponse>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}
impl From<domain::DepartmentPage> for DepartmentPageResponse {
    fn from(page: domain::DepartmentPage) -> Self {
        Self {
            items: page
                .items
                .into_iter()
                .map(|item| DepartmentTreeItemResponse {
                    department: item.department.into(),
                    has_children: item.has_children,
                    is_match: item.is_match,
                })
                .collect(),
            has_more: page.has_more,
            next_cursor: page.next_cursor,
        }
    }
}
#[derive(Debug, Serialize, ToSchema)]
pub struct OrganizationAccessResponse {
    pub can_list: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub can_replace_member_departments: bool,
    pub can_assign_roles: bool,
}
impl From<control_plane::organization::OrganizationAccess> for OrganizationAccessResponse {
    fn from(a: control_plane::organization::OrganizationAccess) -> Self {
        Self {
            can_list: a.can_list,
            can_create: a.can_create,
            can_update: a.can_update,
            can_delete: a.can_delete,
            can_replace_member_departments: a.can_replace_member_departments,
            can_assign_roles: a.can_assign_roles,
        }
    }
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::{Authenticated, ConsoleOperation};
    ConsoleRouteAssembly::new()
        .route(
            "/settings/departments/access",
            console_get(access, Authenticated),
        )
        .route(
            "/settings/departments",
            console_get(list, ConsoleOperation("departments.list".into()))
                .post(create, ConsoleOperation("departments.create".into())),
        )
        .route(
            "/settings/departments/:id",
            console_patch(update, ConsoleOperation("departments.update".into()))
                .delete(delete, ConsoleOperation("departments.delete".into())),
        )
        .route(
            "/settings/members/:id/departments",
            console_put(
                replace,
                ConsoleOperation("members.departments.replace".into()),
            ),
        )
}
async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding: &'static str,
    input: interface::OrganizationInput,
    mutating: bool,
) -> Result<interface::OrganizationOutput, ApiError> {
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        }
    };
    crate::routes::console_interface::invoke(state, binding, credential, input).await
}
#[utoipa::path(get,operation_id="console.departments.access",path="/api/console/settings/departments/access",responses((status=200,body=OrganizationAccessResponse)))]
pub async fn access(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<OrganizationAccessResponse>>, ApiError> {
    let interface::OrganizationOutput::Access(data) = invoke(
        state,
        headers,
        "http.console.departments.access.v1",
        interface::OrganizationInput::Access,
        false,
    )
    .await?
    else {
        return Err(anyhow::anyhow!("organization access output mismatch").into());
    };
    Ok(Json(ApiSuccess::new(data)))
}
#[utoipa::path(get,path="/api/console/settings/departments",params(DepartmentListQuery),responses((status=200,body=DepartmentPageResponse)))]
pub async fn list(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<DepartmentListQuery>,
) -> Result<Json<ApiSuccess<DepartmentPageResponse>>, ApiError> {
    let interface::OrganizationOutput::Departments(data) = invoke(
        state,
        headers,
        "http.console.departments.list.v1",
        interface::OrganizationInput::List(query.into_input()?),
        false,
    )
    .await?
    else {
        return Err(anyhow::anyhow!("organization list output mismatch").into());
    };
    Ok(Json(ApiSuccess::new(data)))
}
#[utoipa::path(post,path="/api/console/settings/departments",request_body=SaveDepartmentBody,responses((status=201,body=DepartmentResponse)))]
pub async fn create(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SaveDepartmentBody>,
) -> Result<(StatusCode, Json<ApiSuccess<DepartmentResponse>>), ApiError> {
    let interface::OrganizationOutput::Department(data) = invoke(
        state,
        headers,
        "http.console.departments.create.v1",
        interface::OrganizationInput::Create(body),
        true,
    )
    .await?
    else {
        return Err(anyhow::anyhow!("organization create output mismatch").into());
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(data))))
}
#[utoipa::path(patch,path="/api/console/settings/departments/{id}",request_body=SaveDepartmentBody,params(("id"=String,Path)),responses((status=200,body=DepartmentResponse)))]
pub async fn update(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveDepartmentBody>,
) -> Result<Json<ApiSuccess<DepartmentResponse>>, ApiError> {
    let interface::OrganizationOutput::Department(data) = invoke(
        state,
        headers,
        "http.console.departments.update.v1",
        interface::OrganizationInput::Update { id, body },
        true,
    )
    .await?
    else {
        return Err(anyhow::anyhow!("organization update output mismatch").into());
    };
    Ok(Json(ApiSuccess::new(data)))
}
#[utoipa::path(delete,path="/api/console/settings/departments/{id}",params(("id"=String,Path)),responses((status=204)))]
pub async fn delete(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    invoke(
        state,
        headers,
        "http.console.departments.delete.v1",
        interface::OrganizationInput::Delete { id },
        true,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(put,path="/api/console/settings/members/{id}/departments",request_body=ReplaceMemberDepartmentsBody,params(("id"=String,Path)),responses((status=204)))]
pub async fn replace(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ReplaceMemberDepartmentsBody>,
) -> Result<StatusCode, ApiError> {
    invoke(
        state,
        headers,
        "http.console.members.departments.replace.v1",
        interface::OrganizationInput::Replace { id, body },
        true,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
