//! Root #2014 AC-004: actual typed owners, both directions, with hostile content.
//! AC-001's full compiled-registry enumeration belongs to D1-04's frozen profile fixture.
use control_plane::auth::{LoginCommand, LoginResult};
use extension_contracts::extension_bus::{
    ContributionId, ManagedArtifactFingerprint, ManagedBindingFingerprint,
    ManagedExecutionIdentity, ManagedInstallationId, ManagedWorkspaceId,
};
use extension_contracts::{
    MANAGED_INTERFACE_PROTOCOL_V1, ManagedHookHostContext, ManagedHookInvocation,
    ManagedInterfaceHostFrame, ManagedInterfaceInput, ManagedInterfaceView,
    ManagedProjectionContract,
};
use interface_runtime::{InterfaceContract, ManagedInterfaceProjection};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::routes::sign_in_interface::{
    PublicSignInInput, PublicSignInOutput, PublicSignInTargetError,
};

const SECRET: &str = "ROOT_2014_CREDENTIAL_SENTINEL_never_export";

fn view<T: InterfaceContract>(value: &T) -> ManagedInterfaceView {
    let projected = ManagedInterfaceProjection::from_contract(value)
        .expect("every compiled owner has an explicit managed projection");
    assert_eq!(projected.contract().contract_id(), T::CONTRACT_ID);
    assert_eq!(projected.contract().version(), T::CONTRACT_VERSION);
    projected_view(projected)
}

fn projected_view(projected: ManagedInterfaceProjection) -> ManagedInterfaceView {
    let result = ManagedInterfaceView {
        contract: ManagedProjectionContract {
            contract_id: projected.contract().contract_id().to_owned(),
            contract_version: projected.contract().version().to_owned(),
            schema: projected.schema().clone(),
        },
        value: projected.value().clone(),
    };
    result
        .validate()
        .expect("typed value satisfies its declared closed schema");
    result
}

fn frame(input: ManagedInterfaceInput) -> ManagedInterfaceHostFrame {
    ManagedInterfaceHostFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V1.into(),
        call_id: "projection-fixture".into(),
        handler: "auditor".into(),
        interface_id: "root-2014.actual-owner-projection".into(),
        interface_version: "1".into(),
        input,
        context: ManagedHookHostContext {
            invocation: ManagedHookInvocation {
                invocation_id: "host-sealed-invocation".into(),
                registry_fingerprint: "registry".into(),
                graph_fingerprint: "graph".into(),
                authority_revision: 1,
            },
            execution_identity: ManagedExecutionIdentity::new(
                ManagedInstallationId::new("auditor-installation").unwrap(),
                ManagedWorkspaceId::new("host-workspace").unwrap(),
                ContributionId::new("auditor").unwrap(),
                ManagedArtifactFingerprint::from_bytes(b"artifact"),
                ManagedBindingFingerprint::from_bytes(b"binding"),
            ),
            generation: std::num::NonZeroU64::new(1).unwrap(),
            deadline_unix_ms: 1000,
            actor_id: Some("host-sealed-actor".into()),
        },
    }
}

fn assert_safe_frame(input: ManagedInterfaceInput) -> Value {
    let frame = frame(input);
    frame.validate().unwrap();
    let wire = serde_json::to_string(&frame).unwrap();
    assert!(
        !wire.contains(SECRET),
        "secret escaped in full worker frame"
    );
    assert!(wire.len() < extension_contracts::MANAGED_INTERFACE_MAX_FRAME_BYTES);
    serde_json::from_str(&wire).unwrap()
}

#[test]
fn root_2014_ac_004_schema_identity_and_secret_negatives() {
    let id = Uuid::new_v4();
    let input = PublicSignInInput(LoginCommand {
        login_entry_id: id,
        identifier: SECRET.into(),
        password: SECRET.into(),
    });
    let input_view = view(&input);
    assert_eq!(input_view.value["0"]["login_entry_id"], id.to_string());
    assert_safe_frame(ManagedInterfaceInput::Before {
        input: input_view.clone(),
    });
    assert_eq!(
        input.0.password, SECRET,
        "projection must not alter handler input"
    );
    let mut injected = input_view.clone();
    injected.value["0"]["password"] = json!(SECRET);
    assert!(
        injected.validate().is_err(),
        "unknown fields must fail closed"
    );
    let mut wrong_type = input_view.clone();
    wrong_type.value["0"]["login_entry_id"] = json!(1);
    assert!(wrong_type.validate().is_err());
    let mut oversized = input_view.clone();
    oversized.value["0"]["login_entry_id"] = json!("x".repeat(257));
    assert!(oversized.validate().is_err());
    let mut changed_version = input_view.contract.clone();
    changed_version.contract_version = "2".into();
    assert_ne!(
        input_view.contract.compile().unwrap().fingerprint(),
        changed_version.compile().unwrap().fingerprint()
    );

    let output = PublicSignInOutput(LoginResult {
        actor: domain::ActorContext {
            user_id: id,
            tenant_id: id,
            current_workspace_id: id,
            effective_display_role: "member".into(),
            is_root: false,
            permissions: Default::default(),
        },
        session: domain::SessionRecord {
            session_id: SECRET.into(),
            user_id: id,
            tenant_id: id,
            current_workspace_id: id,
            active_role_code: "member".into(),
            session_version: 1,
            csrf_token: SECRET.into(),
            expires_at_unix: 1000,
        },
    });
    let output_frame = assert_safe_frame(ManagedInterfaceInput::After {
        output: view(&output),
    });
    assert_eq!(output_frame["context"]["actor_id"], "host-sealed-actor");
    assert!(
        output_frame["input"]["output"]["value"]["0"]
            .get("session")
            .is_none()
    );
    assert_eq!(output.0.session.session_id, SECRET);
    assert_eq!(output.0.session.csrf_token, SECRET);

    let error = PublicSignInTargetError(crate::error_response::ApiError(anyhow::anyhow!(SECRET)));
    let error_view = view(&error);
    assert!(!serde_json::to_string(&error_view).unwrap().contains(SECRET));
    assert!(
        error.0.0.to_string().contains(SECRET),
        "host error chain remains intact"
    );

    let (storage_input, storage_output) =
        crate::routes::file_storages::root_2014_projection_fixture(SECRET);
    assert_safe_frame(ManagedInterfaceInput::Before {
        input: projected_view(storage_input),
    });
    assert_safe_frame(ManagedInterfaceInput::After {
        output: projected_view(storage_output),
    });
    forwarding_and_bytes();
    oversized_typed_values();
}

fn forwarding_and_bytes() {
    use crate::openapi_interface::{
        CallableDispatchForwarding, CallableDispatchHeader, CallableDispatchHttpResponse,
    };
    use crate::routes::mcp_management::{
        debug_execute::{McpDebugExecuteBody, McpDebugResponseMode},
        interface_debug::{McpDebugInput, McpDebugOutput},
    };
    let nested = json!({SECRET: {"innocent": SECRET, "url": format!("https://user:{SECRET}@example.invalid")}});
    let input = McpDebugInput {
        body: McpDebugExecuteBody {
            interface_id: "target.interface".into(),
            debug_response_mode: McpDebugResponseMode::ToolResult,
            mcp_arguments: nested.clone(),
            input_mapping: nested.clone(),
            output_mapping: nested,
        },
        forwarding: CallableDispatchForwarding {
            cookie: Some(SECRET.as_bytes().to_vec()),
            authorization: Some(SECRET.as_bytes().to_vec()),
            csrf_token: Some(SECRET.as_bytes().to_vec()),
            ..Default::default()
        },
    };
    let projected = view(&input);
    assert!(projected.value.get("forwarding").is_none());
    assert_eq!(projected.value["body"]["mcp_arguments"]["kind"], "object");
    assert_safe_frame(ManagedInterfaceInput::Before { input: projected });
    assert_eq!(
        input.forwarding.authorization.as_deref(),
        Some(SECRET.as_bytes())
    );
    let body = SECRET.repeat(100_000).into_bytes();
    let response = McpDebugOutput::Target(CallableDispatchHttpResponse {
        status: 200,
        headers: vec![CallableDispatchHeader {
            name: "set-cookie".into(),
            value: SECRET.as_bytes().to_vec(),
        }],
        body: body.clone(),
    });
    let projected = view(&response);
    assert_eq!(projected.value["0"]["body"]["byte_count"], body.len());
    assert_safe_frame(ManagedInterfaceInput::After { output: projected });
    match response {
        McpDebugOutput::Target(original) => {
            assert_eq!(original.body, body);
            assert_eq!(original.headers[0].value, SECRET.as_bytes());
        }
        _ => unreachable!(),
    }
}

fn oversized_typed_values() {
    use crate::routes::application_public_api::{
        native_interface::ApplicationNativeRunInput, native_read_interface::NativeUploadFileInput,
    };
    use crate::routes::console_identity_interface::ConsoleIdentityInput;
    let input = NativeUploadFileInput {
        file_table_id: Uuid::new_v4(),
        original_filename: SECRET.into(),
        content_type: Some("application/octet-stream".into()),
        bytes: SECRET.repeat(100_000).into_bytes(),
    };
    let projected = view(&input);
    assert_eq!(projected.value["bytes"]["byte_count"], input.bytes.len());
    assert_safe_frame(ManagedInterfaceInput::Before { input: projected });
    assert!(input.bytes.starts_with(SECRET.as_bytes()));
    assert!(
        ConsoleIdentityInput::SwitchWorkspace {
            workspace_id: "x".repeat(257)
        }
        .project_for_managed_hook()
        .is_none()
    );
    let run = ApplicationNativeRunInput {
        request: serde_json::from_value(json!({"query":"hello", "system":
            vec![json!({"type":"text","text":"hello"});33]}))
        .unwrap(),
        protocol:
            control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
    };
    assert!(
        run.project_for_managed_hook().is_none(),
        "typed lists have a hard item budget"
    );
    assert_eq!(run.request.system.len(), 33);
}
