use super::ManagedServiceRegistration;
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    extension_bus::InterfaceRegistryContribution,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane_contracts::ports::ManagedServiceMutationRepository;
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct ManagedServiceInput {
    pub interface_id: String,
    pub payload: serde_json::Value,
    pub stream: Option<super::stream::StreamInput>,
}
pub(crate) struct ManagedServiceOutput(pub serde_json::Value);
// Generic business bodies can contain outbound credentials. Hooks only receive safe metadata.
impl InterfaceContract for ManagedServiceInput {
    const CONTRACT_ID: &'static str = "managed-service-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","properties":{"body_bytes":{"type":"integer","minimum":0,"maximum":33554432}},"required":["body_bytes"],"additionalProperties":false}),
        )
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"body_bytes":serde_json::to_vec(&self.payload).ok()?.len()}))
    }
}
impl InterfaceContract for ManagedServiceOutput {
    const CONTRACT_ID: &'static str = "managed-service-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        ManagedServiceInput::managed_projection_schema()
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"body_bytes":serde_json::to_vec(&self.0).ok()?.len()}))
    }
}
pub(super) struct Operation {
    pub(super) declaration: plugin_framework::ManagedServiceOperation,
    pub(super) input: jsonschema::Validator,
    pub(super) output: jsonschema::Validator,
}
pub(super) struct Adapter {
    pub(super) state: std::sync::Weak<ApiState>,
    pub(super) registration: ManagedServiceRegistration,
    pub(super) operations: BTreeMap<String, Operation>,
}
impl ConsoleInterfacePort<ManagedServiceInput, ManagedServiceOutput> for Adapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ManagedServiceInput,
    ) -> ConsoleInterfaceFuture<'a, ManagedServiceOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
impl Adapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ManagedServiceInput,
    ) -> Result<ManagedServiceOutput, ApiError> {
        use control_plane::errors::ControlPlaneError as Error;
        let state = self
            .state
            .upgrade()
            .ok_or(Error::UpstreamUnavailable("managed_service_host"))?;
        let op = self
            .operations
            .get(&input.interface_id)
            .ok_or(Error::NotFound("managed_service_operation"))?;
        if !op.input.is_valid(&input.payload) {
            return Err(Error::InvalidInput("managed_service_input").into());
        }
        let timeout_ms = input
            .payload
            .pointer("/body/timeout_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(30000)
            .saturating_add(10000);
        let deadline = ((time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
            as i64)
            .checked_add(i64::try_from(timeout_ms).map_err(|_| Error::InvalidInput("timeout_ms"))?)
            .ok_or(Error::InvalidInput("timeout_ms"))?;
        let execution_deadline = tokio::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(timeout_ms))
            .ok_or(Error::InvalidInput("timeout_ms"))?;
        let result = tokio::time::timeout_at(execution_deadline, async {
            let _mutation = if op.declaration.requires_mutation_lock() {
                Some(
                    state
                        .store
                        .lock_managed_service_mutation(
                            self.registration.installation_id,
                            domain::SYSTEM_SCOPE_ID,
                            deadline,
                        )
                        .await?,
                )
            } else {
                None
            };
            let lifetime = _mutation.as_ref().map(|mutation| {
                runtime_core::runtime_backend::RuntimeManagedLifetime {
                    owner: mutation.owner.clone(),
                    lost: mutation.lost.clone(),
                }
            });
            let mut loss = lifetime.as_ref().map(|lifetime| lifetime.lost.clone());
            let execution = async {
                let composition = state.provider_runtime.managed_composition()?;
                let snapshot = match state
                    .extension_boot_snapshot
                    .as_ref()
                    .filter(|boot| boot.has_pinned_managed_snapshots())
                {
                    Some(boot) => boot.managed_snapshot(domain::SYSTEM_SCOPE_ID),
                    None => composition.snapshot(domain::SYSTEM_SCOPE_ID).await,
                }
                .ok_or(Error::Conflict("managed_service_inactive"))?;
                let contribution_id = plugin_framework::extension_bus::ContributionId::new(
                    &op.declaration.contribution_id,
                )?;
                let binding = snapshot
                    .bindings
                    .get(&contribution_id)
                    .ok_or(Error::Conflict("managed_service_inactive"))?;
                if binding.handle.identity().installation_id().as_str()
                    != self.registration.installation_id.to_string()
                {
                    return Err(Error::Conflict("managed_service_generation_mismatch").into());
                }
                let needs_call_context = self.registration.declaration.worker_lifetime
                    == plugin_framework::ManagedServiceWorkerLifetime::ActorWorkspaceScope;
                let caller = if needs_call_context
                    || binding
                        .descriptor
                        .required_permissions
                        .iter()
                        .any(|permission| {
                            matches!(
                                permission.as_str(),
                                "file.import.resource"
                                    | "file.read.content"
                                    | "application.logs.read"
                                    | "application.logs.import"
                                    | "network.http.request"
                            )
                        }) {
                    crate::resource_import::capture_caller(
                        &state,
                        principal,
                        &op.declaration.interface_id,
                    )
                    .await
                    .map_err(|error| error.0)?
                } else {
                    None
                };
                if needs_call_context && caller.is_none() {
                    return Err(Error::NotAuthenticated.into());
                }
                let output = composition
                    .execute_snapshot_with_lifetime(
                        snapshot.clone(),
                        domain::SYSTEM_SCOPE_ID,
                        &contribution_id,
                        runtime_core::runtime_backend::RuntimeExecutionPrincipal {
                            caller,
                            workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
                            actor_id: Some(principal.actor().user_id.to_string()),
                            deadline_unix_ms: deadline,
                        },
                        serde_json::json!({}),
                        input.payload,
                        lifetime.clone(),
                    )
                    .await?;
                if !op.output.is_valid(&output) {
                    return Err(Error::UpstreamUnavailable("managed_service_output_contract").into());
                }
                if op.declaration.invalidates_streams {
                    super::stream::revoke_installation(self.registration.installation_id);
                }
                if op.declaration.invalidates_workers {
                    composition
                        .reset_service_workers_with_lifetime(&binding.handle, lifetime.clone())
                        .await?;
                }
                Ok::<_, anyhow::Error>(ManagedServiceOutput(output))
            };
            tokio::select! {
                biased;
                _ = mutation_loss(&mut loss) => Err(Error::UpstreamUnavailable("managed_service_ownership_lost").into()),
                result = execution => result,
            }
        })
        .await
        .map_err(|_| Error::UpstreamUnavailable("managed_service_deadline_exceeded"))?;
        result.map_err(|error| {
            // Worker and transport error strings are not an outbound-credential-safe response.
            if let Some(failure) = super::ManagedServiceFailure::from_runtime(&error) {
                return failure.into();
            }
            if error.is::<Error>() {
                ApiError::from(error)
            } else {
                Error::UpstreamUnavailable("managed_service_execution").into()
            }
        })
    }
}

async fn mutation_loss(loss: &mut Option<tokio::sync::watch::Receiver<bool>>) {
    let Some(loss) = loss else {
        return std::future::pending().await;
    };
    while !*loss.borrow_and_update() {
        if loss.changed().await.is_err() {
            return;
        }
    }
}
pub(crate) fn registry_contribution(
    state: &Arc<ApiState>,
    registration: &ManagedServiceRegistration,
) -> anyhow::Result<InterfaceRegistryContribution> {
    let mut operations = BTreeMap::new();
    for op in &registration.declaration.operations {
        let compile = |_suffix: &str, schema: serde_json::Value| -> anyhow::Result<_> {
            Ok(jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .build(&schema)?)
        };
        operations.insert(
            op.interface_id.clone(),
            Operation {
                declaration: op.clone(),
                input: compile("input", op.input_schema.clone())?,
                output: compile("output", op.output_schema.clone())?,
            },
        );
    }
    let bindings = registration
        .declaration
        .operations
        .iter()
        .map(|op| format!("http.{}.v1", op.interface_id))
        .collect::<Vec<_>>();
    let declarations = registration
        .declaration
        .operations
        .iter()
        .zip(&bindings)
        .map(|(op, binding)| ConsoleInterfaceDeclaration {
            interface_id: &op.interface_id,
            binding_id: binding,
            method: &op.method,
            path: &op.path,
            mutating: op.method != "GET",
        })
        .collect::<Vec<_>>();
    let adapter = Arc::new(Adapter {
        state: Arc::downgrade(state),
        registration: registration.clone(),
        operations,
    });
    let (stream_declarations, unary_declarations): (Vec<_>, Vec<_>) =
        declarations.into_iter().partition(|declaration| {
            adapter.operations[declaration.interface_id]
                .declaration
                .stream
                .is_some()
        });
    let owner = interface_runtime::InterfaceOwner::new(&registration.plugin_code)?;
    let operations = registration
        .declaration
        .operations
        .iter()
        .map(|op| interface_runtime::AuthorizationOperation::new(&op.interface_id))
        .collect::<Result<Vec<_>, _>>()?;
    let graph = format!(
        "managed-service:{}:{}",
        registration.installation_id, registration.plugin_version
    );
    let mut compiler = interface_runtime::RegistryCompiler::new(
        interface_runtime::GraphFingerprint::new(&graph)?,
        operations,
        [owner],
    );
    if !unary_declarations.is_empty() {
        let registry = console_interface::compile_registry_with_scope(
            &registration.plugin_code,
            &graph,
            &unary_declarations,
            adapter.clone(),
            interface_runtime::InterfaceScope::System,
        )?;
        compiler.absorb_snapshot(&registry)?;
    }
    if !stream_declarations.is_empty() {
        let registry = console_interface::compile_server_stream_registry_with_scope(
            &registration.plugin_code,
            &graph,
            &stream_declarations,
            adapter,
            interface_runtime::InterfaceScope::System,
        )?;
        compiler.absorb_snapshot(&registry)?;
    }
    let registry = compiler.compile()?;
    Ok(InterfaceRegistryContribution::managed(
        registration.plugin_code.clone(),
        registration
            .declaration
            .operations
            .iter()
            .map(|op| op.interface_id.clone())
            .collect(),
        registry,
    ))
}
