//! Root #2014 AC-001–004. Real compiled inventory and formally installed external SDK worker.
use super::{create_pair, managed_create_pair::Fixture};
use axum::{body::Body, http::Request};
use control_plane::plugin_management::{
    GrantContributionPermission, HostContributionGrantPolicy, PluginContributionAuthorityService,
    RevokeContributionPermission,
};
use extension_contracts::{ManagedInterfaceHostFrame, ManagedProjectionContract};
use interface_runtime::*;
use serde_json::json;
use std::{collections::BTreeSet, sync::Arc};
use tower::ServiceExt;

const INTERFACES: [&str; 3] = [
    "model_definitions.create",
    "host_infrastructure.providers.view",
    "application.native.runs.execute-stream",
];
const PHASES: [&str; 6] = [
    "authorization",
    "admission",
    "before",
    "after",
    "failure",
    "completion",
];

fn package() -> Vec<u8> {
    let raw = std::fs::read(
        crate::api_workspace_root()
            .unwrap()
            .join("plugins/fixtures/northwind.lifecycle-auditor/manifest.yaml"),
    )
    .unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(raw.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", raw.as_slice())
        .unwrap();
    let executable = std::env::var_os("MANAGED_HOOK_WORKER_FIXTURE")
        .expect("Root Test Batch builds the real SDK worker once");
    archive
        .append_path_with_name(executable, "bin/worker.py")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}
async fn fixture() -> Fixture {
    let grants = INTERFACES
        .iter()
        .enumerate()
        .flat_map(|(i, _)| {
            PHASES.map(move |phase| GrantContributionPermission {
                contribution_id: format!("northwind.lifecycle-auditor.i{i}.{phase}"),
                permission: format!("hook.interface.{phase}"),
                resource_scope: domain::ContributionResourceScope::Workspace,
                permission_contract_id: "managed-interface".into(),
                permission_contract_version: "1".into(),
            })
        })
        .collect();
    Fixture::new_with_package(package(), grants).await
}
fn trace(f: &Fixture) -> Vec<ManagedInterfaceHostFrame> {
    std::fs::read_to_string(f.worker.with_extension("trace"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn assert_trace(f: &Fixture, interface: &str, phases: &[&str]) {
    let frames = trace(f);
    assert_eq!(
        frames
            .iter()
            .map(|f| f.handler.strip_prefix("trace.").unwrap())
            .collect::<Vec<_>>(),
        phases
    );
    let first = frames
        .first()
        .expect("installed worker must actually execute");
    for frame in &frames {
        frame.validate().unwrap();
        assert_eq!(
            frame.protocol,
            extension_contracts::MANAGED_INTERFACE_PROTOCOL_V1
        );
        assert_eq!(frame.interface_id, interface);
        assert_eq!(frame.context.invocation, first.context.invocation);
        assert_eq!(frame.context.actor_id, Some(f.actor.user_id.to_string()));
        assert_eq!(
            frame.context.execution_identity.workspace_id().as_str(),
            f.actor.current_workspace_id.to_string()
        );
        assert_eq!(
            frame.context.execution_identity.installation_id().as_str(),
            f.installation_id.to_string()
        );
    }
}
async fn providers(f: &Fixture) -> u16 {
    f.app.clone().oneshot(Request::builder()
        .uri(crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH)
        .header("cookie", &f.cookie).body(Body::empty()).unwrap()).await.unwrap().status().as_u16()
}

#[tokio::test]
async fn root_2014_ac_001_complete_compiled_profiles() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let _app = crate::app_with_state(state.clone());
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let inventory: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../scripts/node/plugin-composition-test-batch/root-2014-inventory.json"
    )))
    .unwrap();
    let expected = |baseline: &str, added: &str| -> BTreeSet<String> {
        inventory[baseline]
            .as_array()
            .unwrap()
            .iter()
            .chain(inventory[added].as_array().unwrap())
            .map(|id| id.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        registry
            .definitions()
            .map(|definition| definition.interface_id().as_str().to_string())
            .collect::<BTreeSet<_>>(),
        expected("definitions", "approvedAddedDefinitions")
    );
    assert_eq!(
        registry
            .bindings()
            .map(|binding| binding.binding_id().as_str().to_string())
            .collect::<BTreeSet<_>>(),
        expected("bindings", "approvedAddedBindings")
    );
    assert_eq!(registry.definitions().len(), 458);
    assert_eq!(registry.bindings().len(), 482);
    assert_eq!(registry.managed_contracts().count(), 188);
    let mut ids = BTreeSet::new();
    for descriptor in registry.managed_contracts() {
        assert!(ids.insert(descriptor.contract.clone()));
        ManagedProjectionContract {
            contract_id: descriptor.contract.contract_id().into(),
            contract_version: descriptor.contract.version().into(),
            schema: descriptor.schema.clone().unwrap(),
        }
        .compile()
        .unwrap();
    }
    let mut profiles = BTreeSet::new();
    for binding in registry.bindings() {
        let plan = registry.plan(binding.binding_id()).unwrap();
        assert!(plan.has_managed_invocation_bridge());
        profiles.insert(plan.definition().principal_profile());
        assert!(ids.contains(plan.definition().input_contract()));
        assert!(ids.contains(plan.definition().output_contract()));
        assert!(ids.contains(plan.definition().target_error_contract()));
        if let Some(event) = plan.definition().stream_event_contract() {
            assert!(ids.contains(event));
        }
    }
    assert_eq!(profiles.len(), 3);
    assert_compiler_negatives(&registry);
}

#[tokio::test]
async fn root_2014_ac_002_installed_two_interface_plugin() {
    let f = fixture().await;
    f.mode("");
    let mut body = create_pair::input();
    body["code"] = json!("root_2014_generic_create");
    assert_eq!(f.call(false, body).await.0, 201);
    assert_trace(
        &f,
        INTERFACES[0],
        &[
            "authorization",
            "admission",
            "before",
            "after",
            "completion",
        ],
    );
    f.mode("");
    assert_eq!(providers(&f).await, 200);
    assert_trace(
        &f,
        INTERFACES[1],
        &[
            "authorization",
            "admission",
            "before",
            "after",
            "completion",
        ],
    );
    f.mode("fail.after");
    assert_eq!(
        providers(&f).await,
        200,
        "observer failure cannot replace the host result"
    );
    assert_trace(
        &f,
        INTERFACES[1],
        &[
            "authorization",
            "admission",
            "before",
            "after",
            "completion",
        ],
    );
    f.mode("deny.authorization");
    assert!(providers(&f).await >= 400);
    assert_trace(
        &f,
        INTERFACES[1],
        &["authorization", "failure", "completion"],
    );

    // Current authority is rechecked despite an already published frozen executable graph.
    let authority = PluginContributionAuthorityService::new(
        f.state.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let snapshot = authority.query(&f.actor, f.installation_id).await.unwrap();
    let grant = snapshot
        .authorizations
        .iter()
        .find(|g| g.contribution_id == "northwind.lifecycle-auditor.i1.authorization")
        .unwrap();
    authority
        .revoke(
            &f.actor,
            f.installation_id,
            RevokeContributionPermission {
                authorization_id: grant.id,
                expected_revision: snapshot.revision,
            },
        )
        .await
        .unwrap();
    f.mode("");
    assert!(providers(&f).await >= 400);
    assert!(!trace(&f)
        .iter()
        .any(|frame| frame.handler == "trace.before"));
}

// Negative handlers are never invoked: they exercise real activation rejection, not projection helpers.
struct MissingCodec;
impl InterfaceContract for MissingCodec {
    const CONTRACT_ID: &'static str = "root2014-missing-codec";
    const CONTRACT_VERSION: &'static str = "1";
}
struct MissingCodecHandler;
impl InterfaceHandler<MissingCodec, MissingCodec, MissingCodec> for MissingCodecHandler {
    fn invoke(
        &self,
        _: InterfaceHandlerContext,
        _: MissingCodec,
    ) -> InterfaceHandlerFuture<MissingCodec, MissingCodec> {
        panic!("activation must reject this handler before invocation")
    }
}
struct UnusedFactory;
impl ManagedInterfaceInvocationFactory for UnusedFactory {
    fn freeze(&self, _: ManagedInterfaceFreezeRequest) -> ManagedInterfaceFreezeFuture<'_> {
        panic!("compiler negative does not invoke")
    }
}
fn assert_compiler_negatives(registry: &CompiledInterfaceRegistry) {
    let definition = registry.definitions().next().unwrap();
    let mut duplicate = RegistryCompiler::new(
        registry.graph_fingerprint().clone(),
        [definition.authorization_operation().clone()],
        [definition.owner().clone()],
    );
    duplicate.register_definition(definition.clone()).unwrap();
    assert!(matches!(
        duplicate.register_definition(definition.clone()),
        Err(RegistryCompilationError::DuplicateInterface(_))
    ));

    let binding = registry.bindings().next().unwrap();
    let mut orphan = RegistryCompiler::new(registry.graph_fingerprint().clone(), [], []);
    orphan
        .register_binding(
            binding.clone(),
            registry
                .plan(binding.binding_id())
                .unwrap()
                .adapter_plan()
                .clone(),
        )
        .unwrap();
    assert!(matches!(
        orphan.compile(),
        Err(RegistryCompilationError::BindingUnknownInterface(_))
    ));

    let id = InterfaceId::new("root2014.missing-codec").unwrap();
    let identity = InterfaceIdentity::new(id.clone(), InterfaceVersion::new("1").unwrap());
    let contract = ContractIdentity::new(MissingCodec::CONTRACT_ID, "1").unwrap();
    let contracts = InterfaceContracts::unary(contract.clone(), contract.clone(), contract);
    let operation = AuthorizationOperation::new("root2014.codec.read").unwrap();
    let owner = InterfaceOwner::new("root2014.codec-owner").unwrap();
    let handler = HandlerReference::new("root2014.codec-handler").unwrap();
    let auth = AuthenticationAdapterReference::new("root2014.codec-auth").unwrap();
    let plugin = PluginIdentity::new("root2014.codec-auth").unwrap();
    let mut missing = RegistryCompiler::new(
        registry.graph_fingerprint().clone(),
        [operation.clone()],
        [owner.clone()],
    )
    .with_managed_invocations(Arc::new(UnusedFactory));
    missing
        .register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                PrincipalProfile::User,
                InterfaceAuthenticationPolicy::Authenticated,
                operation,
                InterfaceScope::System,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new("root2014.codec-target").unwrap(),
            ),
            InterfaceAuditPolicy::ReadOnly,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner,
        ))
        .unwrap();
    missing
        .register_authentication_adapter(
            &id,
            1,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                InterfaceScope::System,
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .unwrap(),
            ActivatedAuthenticationAdapter::new(
                plugin,
                InterfaceExtensionTier::BuiltIn,
                auth.clone(),
                AuthenticationActivationIdentity::new("root2014.codec-auth.v1").unwrap(),
                PrincipalProfile::User,
            ),
        )
        .unwrap();
    missing
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.root2014.codec.v1").unwrap(),
                identity,
                contracts,
                ProtocolProjection::http(RouteIdentity::new("GET", "/root2014/codec").unwrap()),
            ),
            InvocationAdapterPlan::new(
                auth,
                AuthorizationAdapterReference::new("root2014.codec-authz").unwrap(),
                None,
            ),
        )
        .unwrap();
    missing
        .bind_handler::<MissingCodec, MissingCodec, MissingCodec, UserPrincipal>(
            &id,
            handler,
            Arc::new(MissingCodecHandler),
        )
        .unwrap();
    assert!(matches!(
        missing.compile(),
        Err(RegistryCompilationError::ManagedProjectionMismatch(_))
    ));
}

async fn start_native_stream(
    f: &Fixture,
    token: &str,
    controls: InvocationControls,
    invalid: bool,
) -> Result<
    InterfaceStreamInvocation<
        crate::routes::application_public_api::native_interface::ApplicationNativeRunStreamEvent,
        crate::routes::application_public_api::native_interface::ApplicationNativeRunOutput,
        crate::routes::application_public_api::native_interface::ApplicationNativeRunTargetError,
    >,
    InterfaceInvocationFailure,
> {
    use crate::routes::application_public_api::{native, native_interface::*};
    let boot = f.state.extension_boot_snapshot.as_ref().unwrap();
    let registry = boot.interface_registry().unwrap().snapshot();
    let binding = BindingId::new(STREAM_BINDING_ID).unwrap();
    let authenticated = boot
        .authenticate_invocation::<_, ApplicationPrincipal>(
            registry.clone(),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: f.state.clone(),
                bearer_token: token.into(),
            },
        )
        .await
        .unwrap();
    let principal = authenticated.principal().clone();
    let lineage = authenticated.lineage().clone();
    let target = native::application_runtime_target(
        &f.state,
        &registry,
        &binding,
        principal.application_id(),
    );
    let activation = registry.authentication(&binding).unwrap();
    let input = ApplicationNativeRunInput {
        protocol:
            control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
        request: serde_json::from_value(
            json!({"query":"Root 2014 native stream", "response_mode":"streaming",
            "stream_options": {"include_workflow_events": if invalid { "debug" } else { "none" }}}),
        )
        .unwrap(),
    };
    let envelope = InvocationEnvelope::with_principal_and_controls(
        lineage,
        binding.clone(),
        InterfaceProtocol::Http,
        activation.adapter().clone(),
        activation.activation().clone(),
        principal,
        controls,
        input,
    );
    InterfaceInvocationKernel::new(Arc::new(ApplicationNativeRunAuthorization))
        .invoke_server_stream_with_dispatch_target::<ApplicationNativeRunInput, ApplicationNativeRunStreamEvent,
            ApplicationNativeRunOutput, ApplicationNativeRunTargetError>(registry, envelope, target).await
}

#[tokio::test]
async fn root_2014_ac_003_unary_stream_terminals() {
    use extension_contracts::{ManagedHookTerminal, ManagedInterfaceInput};
    use std::time::{Duration, SystemTime};
    let f = fixture().await;
    let token = crate::_tests::application_public_api::setup_published_native_app(
        &f.app,
        &f.state,
        "Root 2014 lifecycle stream",
    )
    .await;
    // The same compiled bridge still preserves unary result ownership.
    f.mode("fail.completion");
    assert_eq!(providers(&f).await, 200);
    assert_trace(
        &f,
        INTERFACES[1],
        &[
            "authorization",
            "admission",
            "before",
            "after",
            "completion",
        ],
    );

    for mode in ["", "fail.after", "fail.completion"] {
        f.mode(mode);
        let stream = start_native_stream(
            &f,
            &token,
            InvocationControls::new(None, InvocationCancellation::new(), None),
            false,
        )
        .await
        .unwrap_or_else(|_| panic!("real native stream must establish"));
        assert_trace(&f, INTERFACES[2], &["authorization", "admission", "before"]);
        let (mut events, completion) = stream.into_parts();
        let drain = async {
            let mut count = 0;
            while events.recv().await.is_some() {
                count += 1;
            }
            count
        };
        let (count, terminal) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(drain, completion.complete())
        })
        .await
        .unwrap();
        assert!(count > 0, "actual stream events must flow");
        let terminal =
            terminal.unwrap_or_else(|_| panic!("observer errors cannot replace native terminal"));
        assert_eq!(
            terminal.receipt().terminal(),
            InterfaceInvocationTerminal::Completed
        );
        assert_trace(
            &f,
            INTERFACES[2],
            &[
                "authorization",
                "admission",
                "before",
                "after",
                "completion",
            ],
        );
        assert!(matches!(
            trace(&f).last().unwrap().input,
            ManagedInterfaceInput::Completion {
                terminal: ManagedHookTerminal::Succeeded
            }
        ));
    }
    for (mode, invalid, expected, phases) in [
        (
            "",
            true,
            InterfaceInvocationTerminal::Failed,
            vec![
                "authorization",
                "admission",
                "before",
                "failure",
                "completion",
            ],
        ),
        (
            "deny.admission",
            false,
            InterfaceInvocationTerminal::Rejected,
            vec!["authorization", "admission", "failure", "completion"],
        ),
    ] {
        f.mode(mode);
        let error = match start_native_stream(
            &f,
            &token,
            InvocationControls::new(None, InvocationCancellation::new(), None),
            invalid,
        )
        .await
        {
            Ok(_) => panic!("real target validation or plugin veto must reject"),
            Err(error) => error,
        };
        assert_eq!(error.receipt().terminal(), expected);
        assert_trace(&f, INTERFACES[2], &phases);
    }
    // Controls terminate an established stream, so Completion cannot be emitted at setup.
    for deadline_case in [false, true] {
        f.mode("");
        let cancellation = InvocationCancellation::new();
        let deadline = deadline_case.then(|| SystemTime::now() + Duration::from_secs(3));
        let stream = start_native_stream(
            &f,
            &token,
            InvocationControls::new(deadline, cancellation.clone(), None),
            false,
        )
        .await
        .unwrap_or_else(|_| panic!("controlled stream must establish"));
        assert_trace(&f, INTERFACES[2], &["authorization", "admission", "before"]);
        let (events, completion) = stream.into_parts();
        if let Some(deadline) = deadline {
            if let Ok(remaining) = deadline.duration_since(SystemTime::now()) {
                tokio::time::sleep(remaining + Duration::from_millis(5)).await;
            }
        } else {
            cancellation.cancel();
        }
        let error = match completion.complete().await {
            Ok(_) => panic!("stream control must terminate"),
            Err(error) => error,
        };
        drop(events);
        assert_eq!(
            error.receipt().terminal(),
            InterfaceInvocationTerminal::Cancelled
        );
        assert_trace(
            &f,
            INTERFACES[2],
            &["authorization", "admission", "before", "completion"],
        );
        assert!(matches!(
            trace(&f).last().unwrap().input,
            ManagedInterfaceInput::Completion {
                terminal: ManagedHookTerminal::Cancelled
            }
        ));
    }
}
