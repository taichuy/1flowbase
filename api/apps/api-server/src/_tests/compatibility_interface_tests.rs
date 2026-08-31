use interface_runtime::{BindingId, InterfaceExecutionMode, InterfaceProtocol, ProtocolProjection};

use crate::routes::application_public_api::compatibility_interface::{
    self, ANTHROPIC_MESSAGES_BINDING_ID, ANTHROPIC_MESSAGES_STREAM_BINDING_ID,
    NATIVE_WEBSOCKET_STREAM_BINDING_ID, OPENAI_CHAT_BINDING_ID, OPENAI_CHAT_ROOT_BINDING_ID,
    OPENAI_CHAT_ROOT_STREAM_BINDING_ID, OPENAI_CHAT_STREAM_BINDING_ID, OPENAI_RESPONSES_BINDING_ID,
    OPENAI_RESPONSES_COMPACT_BINDING_ID, OPENAI_RESPONSES_ROOT_BINDING_ID,
    OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID, OPENAI_RESPONSES_STREAM_BINDING_ID,
    OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
};

#[test]
fn blocking_compatibility_bindings_publish_as_typed_http_plans() {
    let registry = compatibility_interface::compile_registry_for_test()
        .expect("blocking compatibility catalog must publish");
    let expected = [
        OPENAI_CHAT_BINDING_ID,
        OPENAI_CHAT_ROOT_BINDING_ID,
        OPENAI_RESPONSES_BINDING_ID,
        OPENAI_RESPONSES_ROOT_BINDING_ID,
        OPENAI_RESPONSES_COMPACT_BINDING_ID,
        ANTHROPIC_MESSAGES_BINDING_ID,
        OPENAI_CHAT_STREAM_BINDING_ID,
        OPENAI_CHAT_ROOT_STREAM_BINDING_ID,
        OPENAI_RESPONSES_STREAM_BINDING_ID,
        OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID,
        ANTHROPIC_MESSAGES_STREAM_BINDING_ID,
        NATIVE_WEBSOCKET_STREAM_BINDING_ID,
        OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
    ];

    assert_eq!(registry.bindings().len(), expected.len());
    for raw_binding_id in expected {
        let binding_id = BindingId::new(raw_binding_id).expect("fixture binding id is valid");
        let plan = registry
            .plan(&binding_id)
            .expect("frozen binding must resolve an invocation plan");
        assert_eq!(
            plan.binding().projection().protocol(),
            InterfaceProtocol::Http
        );
        assert!(matches!(
            plan.binding().projection(),
            ProtocolProjection::Http { .. } | ProtocolProjection::HttpVariant { .. }
        ));
        assert_eq!(
            plan.authentication().adapter().as_str(),
            "api-server.application-api-key"
        );
        assert_eq!(
            plan.authentication().activation().as_str(),
            "api-server.application-api-key.activation.v1"
        );
        assert_eq!(
            plan.effective_handler().handler(),
            plan.definition().handler_reference()
        );
        let expected_mode = if raw_binding_id.contains(".stream.") {
            InterfaceExecutionMode::ServerStream
        } else {
            InterfaceExecutionMode::Unary
        };
        assert_eq!(plan.definition().execution_mode(), expected_mode);
    }
}

#[test]
fn blocking_compatibility_routes_select_frozen_binding_constants() {
    let source = include_str!("../routes/application_public_api/openai.rs");
    let anthropic = include_str!("../routes/application_public_api/anthropic.rs");

    for binding in [
        "OPENAI_CHAT_BINDING_ID",
        "OPENAI_CHAT_ROOT_BINDING_ID",
        "OPENAI_RESPONSES_BINDING_ID",
        "OPENAI_RESPONSES_ROOT_BINDING_ID",
        "OPENAI_RESPONSES_COMPACT_BINDING_ID",
        "OPENAI_CHAT_STREAM_BINDING_ID",
        "OPENAI_RESPONSES_STREAM_BINDING_ID",
    ] {
        assert!(source.contains(binding), "route must select {binding}");
    }
    assert!(anthropic.contains("ANTHROPIC_MESSAGES_BINDING_ID"));
    assert!(anthropic.contains("ANTHROPIC_MESSAGES_STREAM_BINDING_ID"));
    assert!(source.contains("compatibility_interface::invoke_blocking"));
    assert!(source.contains("compatibility_interface::invoke_stream"));
    assert!(anthropic.contains("compatibility_interface::invoke_blocking"));
}
