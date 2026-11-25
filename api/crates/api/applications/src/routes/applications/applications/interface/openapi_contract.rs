use uuid::Uuid;

use super::{ApplicationsInput, ApplicationsOutput};
use crate::routes::console_interface::ConsoleInterfaceDeclaration;

const fn decl(
    interface_id: &'static str,
    binding_id: &'static str,
    method: &'static str,
    path: &'static str,
    mutating: bool,
) -> ConsoleInterfaceDeclaration<'static> {
    ConsoleInterfaceDeclaration {
        interface_id,
        binding_id,
        method,
        path,
        mutating,
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    decl(
        "applications.list",
        "http.console.applications.list.v1",
        "GET",
        "/api/console/applications",
        false,
    ),
    decl(
        "applications.create",
        "http.console.applications.create.v1",
        "POST",
        "/api/console/applications",
        true,
    ),
    decl(
        "applications.get",
        "http.console.applications.get.v1",
        "GET",
        "/api/console/applications/:id",
        false,
    ),
    decl(
        "applications.update",
        "http.console.applications.update.v1",
        "PATCH",
        "/api/console/applications/:id",
        true,
    ),
    decl(
        "applications.delete",
        "http.console.applications.delete.v1",
        "DELETE",
        "/api/console/applications/:id",
        true,
    ),
    decl(
        "applications.catalog.get",
        "http.console.applications.catalog.get.v1",
        "GET",
        "/api/console/applications/catalog",
        false,
    ),
    decl(
        "applications.tags.create",
        "http.console.applications.tags.create.v1",
        "POST",
        "/api/console/applications/tags",
        true,
    ),
    decl(
        "applications.environment-variables.list",
        "http.console.applications.environment-variables.list.v1",
        "GET",
        "/api/console/applications/:id/environment-variables",
        false,
    ),
    decl(
        "applications.environment-variables.replace",
        "http.console.applications.environment-variables.replace.v1",
        "PUT",
        "/api/console/applications/:id/environment-variables",
        true,
    ),
    decl(
        "applications.js-dependencies.list",
        "http.console.applications.js-dependencies.list.v1",
        "GET",
        "/api/console/applications/:id/js-dependencies",
        false,
    ),
    decl(
        "applications.js-dependencies.replace",
        "http.console.applications.js-dependencies.replace.v1",
        "PUT",
        "/api/console/applications/:id/js-dependencies",
        true,
    ),
];

impl crate::openapi_interface::contract::ApiInput for ApplicationsInput {
    fn document_input_schema(
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<serde_json::Value> {
        (operation.interface_id == "applications.environment-variables.replace").then(|| {
            crate::routes::applications_group::event_projection::environment_input_schema(operation)
        })
    }
    fn project_document_input(
        &self,
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        use crate::openapi_interface::contract::to_value;
        use crate::routes::applications_group::event_projection::{environment_body, request};
        let empty = || serde_json::json!({});
        let id_path = |id: &Uuid| serde_json::json!({"application_id": id});
        let null = serde_json::Value::Null;
        let (path_params, body) = match self {
            Self::List | Self::Catalog { .. } => (empty(), null),
            Self::Create(body) => (empty(), to_value(body)?),
            Self::CreateTag(body) => (empty(), to_value(body)?),
            Self::Get { application_id }
            | Self::Delete { application_id }
            | Self::ListEnvironmentVariables { application_id }
            | Self::ListJsDependencies { application_id } => (id_path(application_id), null),
            Self::Patch {
                application_id,
                body,
            } => (id_path(application_id), to_value(body)?),
            Self::ReplaceEnvironmentVariables {
                application_id,
                body,
            } => (id_path(application_id), to_value(&environment_body(body))?),
            Self::ReplaceJsDependency {
                application_id,
                body,
            } => (id_path(application_id), to_value(body)?),
        };
        Ok(request(operation, path_params, empty(), body))
    }
}

impl crate::openapi_interface::contract::ApiOutput for ApplicationsOutput {
    fn document_output_schema(
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<serde_json::Value> {
        matches!(operation.interface_id.as_str(), "applications.environment-variables.list" | "applications.environment-variables.replace").then(crate::openapi_interface::contract::schema::<Vec<crate::routes::applications_group::event_projection::EnvironmentVariableResponse>>)
    }
    fn project_document_output(
        &self,
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        match self {
            Self::Applications(value) => crate::openapi_interface::contract::to_value(value),
            Self::Catalog(value) => crate::openapi_interface::contract::to_value(value),
            Self::Application(value) => crate::openapi_interface::contract::to_value(value),
            Self::Tag(value) => crate::openapi_interface::contract::to_value(value),
            Self::EnvironmentVariables(values) => crate::openapi_interface::contract::to_value(
                &crate::routes::applications_group::event_projection::environment_response(values),
            ),
            Self::JsDependencies(value) => crate::openapi_interface::contract::to_value(value),
            Self::JsDependency(value) => crate::openapi_interface::contract::to_value(value),
            Self::NoContent => Ok(serde_json::Value::Null),
        }
    }
}
