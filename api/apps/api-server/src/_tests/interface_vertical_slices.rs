#[test]
fn issue_1944_four_approved_vertical_slices_enter_the_typed_kernel() {
    let slices = [
        (
            "Public",
            include_str!("../routes/identity/auth.rs"),
            "PublicPrincipal::new()",
        ),
        (
            "Console/User",
            include_str!("../routes/settings/host_infrastructure/interface_operation.rs"),
            "UserPrincipal",
        ),
        (
            "Application/SSE",
            include_str!("../routes/application_public_api/native.rs"),
            "interface_application_principal(&api_actor)",
        ),
        (
            "MCP/User API Key",
            include_str!("../routes/mcp_protocol.rs"),
            "context.interface_principal()",
        ),
    ];
    for (name, source, principal_probe) in slices {
        assert!(
            source.contains("InterfaceInvocationKernel::new"),
            "{name} must enter InterfaceInvocationKernel"
        );
        assert!(
            source.contains(principal_probe),
            "{name} must establish its frozen typed principal"
        );
        assert!(
            source.contains(".projected()"),
            "{name} protocol adapter must mark projection after terminal receipt"
        );
    }
}

#[test]
fn issue_1944_credential_material_stops_before_typed_handlers() {
    for (source, handler_start, handler_end) in [
        (
            include_str!("../routes/identity/login_instances_interface.rs"),
            "struct PublicLoginInstancesHandler",
            "pub(crate) struct PublicLoginInstancesAuthorization",
        ),
        (
            include_str!("../routes/application_public_api/native_interface.rs"),
            "struct ApplicationNativeRunHandler",
            "pub(crate) struct ApplicationNativeRunAuthorization",
        ),
        (
            include_str!("../routes/mcp_protocol/interface_operation.rs"),
            "struct McpInvocationHandler",
            "pub(super) struct McpInvocationAuthorization",
        ),
        (
            include_str!("../routes/settings/host_infrastructure/interface_operation.rs"),
            "struct HostInfrastructureProvidersViewHandler",
            "struct ConsoleInterfaceAuthorizationPort",
        ),
    ] {
        let start = source.find(handler_start).expect("handler start probe");
        let end = source[start..]
            .find(handler_end)
            .map(|offset| start + offset)
            .expect("handler end probe");
        let handler = &source[start..end];
        for forbidden in [
            "HeaderMap",
            "Cookie",
            "bearer_token",
            "ApiState",
            "RuntimeHost",
        ] {
            assert!(
                !handler.contains(forbidden),
                "typed handler module leaked forbidden request/runtime material: {forbidden}"
            );
        }
    }
}
