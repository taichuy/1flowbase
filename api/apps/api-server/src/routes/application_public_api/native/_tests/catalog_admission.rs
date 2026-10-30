use serde_json::json;

use super::compiled_plan_has_mcp_bindings;

#[test]
fn compiled_plan_without_mcp_bindings_skips_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"model": "test"}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), Some(false));
}

#[test]
fn compiled_plan_with_mcp_bindings_keeps_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"mcp_instance_ids": ["instance"]}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), Some(true));
}

#[test]
fn unknown_compiled_plan_keeps_catalog() {
    let plan = json!({"nodes": {"llm": {"config": {"mcp_instance_ids": 1}}}});
    assert_eq!(compiled_plan_has_mcp_bindings(&plan), None);
}

#[tokio::test]
async fn no_mcp_factory_leases_g2_without_catalog_and_keeps_detached_task_tail() {
    use crate::{
        _tests::support::{default_test_config, test_api_state_with_database_url},
        generation_dispatch::GenerationDispatch,
        routes::{
            mcp_management::interface_catalog::{
                McpInterfaceCatalogDependencies, McpInterfaceRegistrySnapshotPort,
            },
            mcp_protocol::virtual_ui::RuntimeInternalToolInvokerDependencies,
        },
    };
    use std::{collections::BTreeMap, sync::Arc, time::Duration};
    struct ForbiddenCatalog;
    impl McpInterfaceRegistrySnapshotPort for ForbiddenCatalog {
        fn snapshot(
            &self,
        ) -> Result<
            Arc<interface_runtime::CompiledInterfaceRegistry>,
            crate::error_response::ApiError,
        > {
            panic!("no-MCP lease must never query the MCP catalog")
        }
    }
    let (state, _) = test_api_state_with_database_url().await;
    let initial = crate::app_with_state_and_config(state.clone(), &default_test_config());
    let publisher = crate::managed_publication::ManagedGenerationPublisher::new(
        &state,
        default_test_config(),
        vec![],
        initial,
    )
    .unwrap();
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    let selected = publisher.current_generation().unwrap();
    let g2_id = selected.owner.generation_id();
    let probe = Arc::downgrade(&selected.owner);
    let retired = selected.owner.retirement_notification();
    let factory =
        super::native_runtime_invoker_factory(super::NativeRuntimeInvokerFactoryDependencies {
            generation: probe.clone(),
            runtime_dependencies: RuntimeInternalToolInvokerDependencies::new(
                selected.state.store.clone(),
                selected.state.infrastructure.cache_store(),
                selected.state.provider_secret_master_key.clone(),
            ),
            interface_catalog: McpInterfaceCatalogDependencies {
                store: selected.state.store.clone(),
                openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                    store: selected.state.store.clone(),
                    console_operations: selected
                        .state
                        .console_operation_registry
                        .inventory()
                        .clone(),
                    interface_registry: None,
                    api_docs: selected.state.api_docs.clone(),
                    template_catalog: selected.state.runtime_engine.template_catalog().clone(),
                },
            },
            interface_registry: Arc::new(ForbiddenCatalog),
        });
    let invoker = factory.without_mcp().unwrap();
    let body = crate::generation_dispatch::bind_response(
        axum::response::Response::new(axum::body::Body::empty()),
        selected.owner.clone(),
    );
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let task = tokio::spawn({
        let entered = entered.clone();
        let release = release.clone();
        let state = Arc::downgrade(&selected.state);
        async move {
            let _invoker = invoker;
            entered.notify_one();
            // Models task recovery/persistence after a business terminal, not a status poll.
            release.notified().await;
            let owner = GenerationDispatch::acquire(&state.upgrade().unwrap()).unwrap();
            assert_eq!(owner.generation_id(), g2_id);
        }
    });
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    drop(selected);
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    drop(body);
    assert!(
        probe.upgrade().is_some(),
        "only the detached no-MCP invoker now owns G2"
    );
    release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), retired.notified())
        .await
        .unwrap();
    assert!(
        probe.upgrade().is_none(),
        "retained static factory must stay Weak"
    );
    assert!(
        factory.without_mcp().is_err(),
        "expired generation cannot fall back to latest"
    );
}
