use super::{
    DepartmentPageResponse, DepartmentResponse, OrganizationAccessResponse,
    ReplaceMemberDepartmentsBody, SaveDepartmentBody,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane::{navigation_cache::NavigationCache, organization::OrganizationService};
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::sync::Arc;
use storage_durable_postgres::MainDurableStore;
pub(crate) enum OrganizationInput {
    Access,
    List(control_plane::ports::DepartmentListInput),
    Create(SaveDepartmentBody),
    Update {
        id: String,
        body: SaveDepartmentBody,
    },
    Delete {
        id: String,
    },
    Replace {
        id: String,
        body: ReplaceMemberDepartmentsBody,
    },
}
pub(crate) enum OrganizationOutput {
    Access(OrganizationAccessResponse),
    Departments(DepartmentPageResponse),
    Department(DepartmentResponse),
    Empty,
}
impl InterfaceContract for OrganizationInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(
            ["Access", "List", "Create", "Update", "Delete", "Replace"]
                .into_iter()
                .map(|tag| mp::object_schema(&[("variant", mp::tag_schema(tag))]))
                .collect(),
        ))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"variant":match self{Self::Access=>"Access",Self::List(_)=>"List",Self::Create(_)=>"Create",Self::Update{..}=>"Update",Self::Delete{..}=>"Delete",Self::Replace{..}=>"Replace"}}),
        )
    }
    const CONTRACT_VERSION: &'static str = "2";
    const CONTRACT_ID: &'static str = "console-organization-input";
}
impl InterfaceContract for OrganizationOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(
            ["Access", "Departments", "Department", "Empty"]
                .into_iter()
                .map(|tag| mp::object_schema(&[("variant", mp::tag_schema(tag))]))
                .collect(),
        ))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"variant":match self{Self::Access(_)=>"Access",Self::Departments(_)=>"Departments",Self::Department(_)=>"Department",Self::Empty=>"Empty"}}),
        )
    }
    const CONTRACT_VERSION: &'static str = "2";
    const CONTRACT_ID: &'static str = "console-organization-output";
}
struct Adapter {
    store: MainDurableStore,
    cache: NavigationCache,
}
pub(crate) fn port(
    store: MainDurableStore,
    cache: NavigationCache,
) -> Arc<dyn ConsoleInterfacePort<OrganizationInput, OrganizationOutput>> {
    Arc::new(Adapter { store, cache })
}
fn id(value: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("department_id").into())
}
impl Adapter {
    async fn inner(
        &self,
        principal: &UserPrincipal,
        input: OrganizationInput,
    ) -> Result<OrganizationOutput, ApiError> {
        let actor = principal.actor();
        let service = OrganizationService::new(self.store.for_actor(actor.clone()))
            .with_navigation_cache(self.cache.clone());
        Ok(match input {
            OrganizationInput::Access => {
                OrganizationOutput::Access(service.access(actor).await?.into())
            }
            OrganizationInput::List(input) => {
                OrganizationOutput::Departments(service.list_page(actor, input).await?.into())
            }
            OrganizationInput::Create(body) => OrganizationOutput::Department(
                service
                    .save(
                        actor,
                        None,
                        body.name,
                        body.parent_id.as_deref().map(id).transpose()?,
                        body.role_codes,
                    )
                    .await?
                    .into(),
            ),
            OrganizationInput::Update {
                id: department_id,
                body,
            } => OrganizationOutput::Department(
                service
                    .save(
                        actor,
                        Some(id(&department_id)?),
                        body.name,
                        body.parent_id.as_deref().map(id).transpose()?,
                        body.role_codes,
                    )
                    .await?
                    .into(),
            ),
            OrganizationInput::Delete { id: department_id } => {
                service.delete(actor, id(&department_id)?).await?;
                OrganizationOutput::Empty
            }
            OrganizationInput::Replace {
                id: member_id,
                body,
            } => {
                service
                    .replace_member_departments(
                        actor,
                        id(&member_id)?,
                        domain::MemberDepartments {
                            department_ids: body
                                .department_ids
                                .iter()
                                .map(|v| id(v))
                                .collect::<Result<_, _>>()?,
                            primary_department_id: body
                                .primary_department_id
                                .as_deref()
                                .map(id)
                                .transpose()?,
                        },
                    )
                    .await?;
                OrganizationOutput::Empty
            }
        })
    }
}
impl ConsoleInterfacePort<OrganizationInput, OrganizationOutput> for Adapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: OrganizationInput,
    ) -> ConsoleInterfaceFuture<'a, OrganizationOutput> {
        Box::pin(async move {
            self.inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "console.departments.access",
        binding_id: "http.console.departments.access.v1",
        method: "GET",
        path: "/api/console/settings/departments/access",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "departments.list",
        binding_id: "http.console.departments.list.v1",
        method: "GET",
        path: "/api/console/settings/departments",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "departments.create",
        binding_id: "http.console.departments.create.v1",
        method: "POST",
        path: "/api/console/settings/departments",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "departments.update",
        binding_id: "http.console.departments.update.v1",
        method: "PATCH",
        path: "/api/console/settings/departments/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "departments.delete",
        binding_id: "http.console.departments.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/departments/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "members.departments.replace",
        binding_id: "http.console.members.departments.replace.v1",
        method: "PUT",
        path: "/api/console/settings/members/:id/departments",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<OrganizationInput, OrganizationOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-organization",
        "graph:console-organization-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
#[path = "_tests/registry.rs"]
mod tests;
