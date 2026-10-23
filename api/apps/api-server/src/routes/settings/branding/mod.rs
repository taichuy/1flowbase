use crate::routes::{
    console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    console_route_assembly::{console_get, ConsoleRouteAssembly},
};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{extract::State, http::HeaderMap, Json};
use control_plane::system_branding::SystemBrandingService;
use control_plane_contracts::ports::SystemBranding;
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::sync::Arc;

pub(crate) mod public_interface;

pub(crate) enum BrandingInput {
    Read,
    Save(SystemBranding),
}
pub(crate) struct BrandingOutput(pub SystemBranding);
impl InterfaceContract for BrandingInput {
    const CONTRACT_ID: &'static str = "system-branding-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","additionalProperties":false,"properties":{"action":{"type":"string","maxLength":4,"enum":["read","save"]}},"required":["action"]}),
        )
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"action": match self { Self::Read => "read", Self::Save(_) => "save" }}),
        )
    }
}
impl InterfaceContract for BrandingOutput {
    const CONTRACT_ID: &'static str = "system-branding-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "system_name",
            crate::extension_bus::managed_projection::text_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"system_name": crate::extension_bus::managed_projection::text(&self.0.system_name)?}),
        )
    }
}
struct BrandingAdapter(storage_durable_postgres::MainDurableStore);
impl ConsoleInterfacePort<BrandingInput, BrandingOutput> for BrandingAdapter {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        input: BrandingInput,
    ) -> ConsoleInterfaceFuture<'a, BrandingOutput> {
        Box::pin(async move {
            let service = SystemBrandingService::new(self.0.clone());
            let value = match input {
                BrandingInput::Read => service.read().await,
                BrandingInput::Save(value) => service.save(value).await,
            };
            value
                .map(BrandingOutput)
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
    console_interface::compile_registry(
        "api-server.system-branding",
        "graph:system-branding-v1",
        &[
            ConsoleInterfaceDeclaration {
                interface_id: "system.branding.get",
                binding_id: "http.console.system-branding.get.v1",
                method: "GET",
                path: "/api/console/settings/system",
                mutating: false,
            },
            ConsoleInterfaceDeclaration {
                interface_id: "system.branding.save",
                binding_id: "http.console.system-branding.put.v1",
                method: "PUT",
                path: "/api/console/settings/system",
                mutating: true,
            },
        ],
        Arc::new(BrandingAdapter(store)),
    )
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    ConsoleRouteAssembly::new().route(
        "/settings/system",
        console_get(
            get_system_settings,
            ConsoleOperation("system.branding.get".into()),
        )
        .put(
            save_system_settings,
            ConsoleOperation("system.branding.save".into()),
        ),
    )
}

/// Read installation-wide system branding.
/// Returns the persisted logo URL, system name, login tagline visibility, description and filing HTML.
#[utoipa::path(get, path = "/api/console/settings/system", responses((status = 200, body = serde_json::Value)))]
pub async fn get_system_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<SystemBranding>>, ApiError> {
    let output: BrandingOutput = console_interface::invoke(
        Arc::clone(&state),
        "http.console.system-branding.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        BrandingInput::Read,
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

/// Save installation-wide system branding.
/// Replaces all public display fields. Administrator-authored filing HTML is stored verbatim.
#[utoipa::path(put, path = "/api/console/settings/system", request_body = serde_json::Value, responses((status = 200, body = serde_json::Value)))]
pub async fn save_system_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(value): Json<SystemBranding>,
) -> Result<Json<ApiSuccess<SystemBranding>>, ApiError> {
    let output: BrandingOutput = console_interface::invoke(
        Arc::clone(&state),
        "http.console.system-branding.put.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        BrandingInput::Save(value),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}
