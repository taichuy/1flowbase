use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use crate::{
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    runtime_data_model_docs,
};

pub(crate) struct DataModelOpenApiInput {
    pub(crate) model_id: String,
}

impl InterfaceContract for DataModelOpenApiInput {
    const CONTRACT_ID: &'static str = "console-data-model-openapi-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct DataModelOpenApiOutput(pub(crate) serde_json::Value);

impl InterfaceContract for DataModelOpenApiOutput {
    const CONTRACT_ID: &'static str = "console-data-model-openapi-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct DataModelOpenApiAdapter {
    store: MainDurableStore,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
}

impl ConsoleInterfacePort<DataModelOpenApiInput, DataModelOpenApiOutput>
    for DataModelOpenApiAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: DataModelOpenApiInput,
    ) -> ConsoleInterfaceFuture<'a, DataModelOpenApiOutput> {
        Box::pin(async move {
            let model_id = uuid::Uuid::parse_str(&input.model_id).map_err(|_| {
                ConsoleInterfaceTargetError(
                    control_plane::errors::ControlPlaneError::InvalidInput("model_id").into(),
                )
            })?;
            let model =
                control_plane::model_definition::ModelDefinitionService::for_console_operation(
                    self.store.clone(),
                    domain::ConsolePolicyGroup::settings_feature("system.data-models")
                        .expect("compiled data-model settings group must be valid"),
                    access_control::MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID,
                )
                .get_model(principal.actor().user_id, model_id)
                .await
                .map_err(|error| ConsoleInterfaceTargetError(error.into()))?;
            if model.status != domain::DataModelStatus::Published {
                return Err(ConsoleInterfaceTargetError(
                    control_plane::errors::ControlPlaneError::NotFound("model_id").into(),
                ));
            }
            let spec = runtime_data_model_docs::build_model_openapi(
                &model,
                self.runtime_engine.template_catalog(),
            )
            .ok_or_else(|| {
                ConsoleInterfaceTargetError(
                    control_plane::errors::ControlPlaneError::NotFound("model_id").into(),
                )
            })?;
            Ok(DataModelOpenApiOutput(spec))
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "model-definitions.openapi.view",
    binding_id: "http.console.model-definitions.openapi.view.v1",
    method: "GET",
    path: "/api/console/settings/data-models/model-definitions/:model_id/openapi.json",
    mutating: false,
}];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-data-model-openapi",
        "graph:console-data-model-openapi-v1",
        DECLARATIONS,
        Arc::new(DataModelOpenApiAdapter {
            store,
            runtime_engine,
        }),
    )
}
