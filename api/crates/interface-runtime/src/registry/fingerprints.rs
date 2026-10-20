use super::*;

pub(super) fn registry_fingerprint(
    graph_fingerprint: &GraphFingerprint,
    definitions: &BTreeMap<InterfaceId, InterfaceDefinition>,
    bindings: &BTreeMap<BindingId, ProtocolBinding>,
    plans: &BTreeMap<BindingId, CompiledInvocationPlan>,
) -> RegistryFingerprint {
    let mut digest = Sha256::new();
    digest.update(graph_fingerprint.as_str().as_bytes());
    for definition in definitions.values() {
        for part in [
            definition.interface_id().as_str(),
            definition.version().as_str(),
            definition.input_contract().contract_id(),
            definition.input_contract().version(),
            definition.output_contract().contract_id(),
            definition.output_contract().version(),
            definition.target_error_contract().contract_id(),
            definition.target_error_contract().version(),
            definition.authorization_operation().as_str(),
            definition.handler_reference().as_str(),
            definition.target_reference().as_str(),
            definition.owner().as_str(),
            match definition.authentication() {
                InterfaceAuthenticationPolicy::Anonymous => "authn:anonymous",
                InterfaceAuthenticationPolicy::Authenticated => "authn:authenticated",
            },
            match definition.principal_profile() {
                PrincipalProfile::Public => "principal:public",
                PrincipalProfile::User => "principal:user",
                PrincipalProfile::Application => "principal:application",
            },
            match definition.audit() {
                InterfaceAuditPolicy::ReadOnly => "audit:read-only",
                InterfaceAuditPolicy::Mutating => "audit:mutating",
            },
            match definition.error() {
                InterfaceErrorPolicy::TypedTarget => "error:typed-target",
            },
            match definition.scope() {
                InterfaceScope::System => "scope:system",
                InterfaceScope::Workspace => "scope:workspace",
            },
            match definition.lifecycle() {
                InterfaceLifecycle::BootSnapshot => "lifecycle:boot-snapshot",
            },
            match definition.execution_mode() {
                InterfaceExecutionMode::Unary => "mode:unary",
                InterfaceExecutionMode::ServerStream => "mode:server-stream",
                InterfaceExecutionMode::AsyncAck => "mode:async-ack",
            },
        ] {
            digest.update([0]);
            digest.update(part.as_bytes());
        }
        if let Some(event) = definition.stream_event_contract() {
            digest.update([0]);
            digest.update(event.contract_id().as_bytes());
            digest.update([0]);
            digest.update(event.version().as_bytes());
        }
    }
    for binding in bindings.values() {
        digest.update([0]);
        digest.update(binding.binding_id().as_str().as_bytes());
        digest.update([0]);
        digest.update(binding_fingerprint(binding).as_str().as_bytes());
        digest.update([0]);
        digest.update(
            plans
                .get(binding.binding_id())
                .expect("every binding must have a compiled plan")
                .fingerprint()
                .as_str()
                .as_bytes(),
        );
    }
    RegistryFingerprint::new(format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&digest.finalize())
    ))
    .expect("SHA-256 registry fingerprint must be a valid identity")
}

pub(super) fn binding_fingerprint(binding: &ProtocolBinding) -> BindingFingerprint {
    let mut digest = Sha256::new();
    for part in [
        binding.binding_id().as_str(),
        binding.interface_identity().interface_id().as_str(),
        binding.interface_identity().version().as_str(),
        binding.input_contract().contract_id(),
        binding.input_contract().version(),
        binding.output_contract().contract_id(),
        binding.output_contract().version(),
        binding.contracts().target_error().contract_id(),
        binding.contracts().target_error().version(),
    ] {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    if let Some(event) = binding.contracts().stream_event() {
        digest.update([0]);
        digest.update(event.contract_id().as_bytes());
        digest.update([0]);
        digest.update(event.version().as_bytes());
    }
    digest.update([0]);
    digest.update(match binding.contracts().mode() {
        InterfaceExecutionMode::Unary => b"mode:unary".as_slice(),
        InterfaceExecutionMode::ServerStream => b"mode:server-stream".as_slice(),
        InterfaceExecutionMode::AsyncAck => b"mode:async-ack".as_slice(),
    });
    let (kind, first, second, third) = binding.projection().fingerprint_parts();
    for part in [Some(kind), Some(first), second, third]
        .into_iter()
        .flatten()
    {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    BindingFingerprint::new(format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&digest.finalize())
    ))
    .expect("SHA-256 binding fingerprint must be a valid identity")
}

pub(super) fn plan_fingerprint(
    graph_fingerprint: &GraphFingerprint,
    definition: &InterfaceDefinition,
    binding_fingerprint: &BindingFingerprint,
    adapter_plan: &InvocationAdapterPlan,
    authentication: &ActivatedAuthenticationAdapter,
    extension_plan: &crate::ExtensionPlanFingerprint,
    effective_handler: &InterfaceHandlerCandidate,
) -> PlanFingerprint {
    let mut digest = Sha256::new();
    for part in [
        graph_fingerprint.as_str(),
        definition.interface_id().as_str(),
        definition.version().as_str(),
        binding_fingerprint.as_str(),
        definition.authorization_operation().as_str(),
        definition.handler_reference().as_str(),
        definition.target_reference().as_str(),
        adapter_plan.authentication().as_str(),
        authentication.activation().as_str(),
        adapter_plan.authorization().as_str(),
        effective_handler.plugin().as_str(),
        effective_handler.handler().as_str(),
        effective_handler.target().as_str(),
    ] {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    if let Some(admission) = adapter_plan.admission() {
        digest.update([0]);
        digest.update(admission.as_str().as_bytes());
    }
    digest.update([0]);
    digest.update(extension_plan.as_str().as_bytes());
    PlanFingerprint::new(format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&digest.finalize())
    ))
    .expect("SHA-256 plan fingerprint must be a valid identity")
}
