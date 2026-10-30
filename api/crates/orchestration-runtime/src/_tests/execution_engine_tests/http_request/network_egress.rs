use super::*;
use crate::execution_engine::execute_http_request_node_with_provider_invoker;

#[tokio::test]
async fn http_request_explicit_direct_bypasses_workspace_resolver() {
    let (url, _, server) =
        spawn_http_fixture(vec![response("/", 200, "text/plain", b"direct".to_vec())]).await;
    let plan = http_request_plan(
        json!({"url":url,"network_egress":{"mode":"direct"}}),
        BTreeMap::new(),
    );
    let result = execute_http_request_node_with_provider_invoker(
        &plan.nodes["node-http"],
        &Default::default(),
        &Default::default(),
        None,
        Some(&FailingNetworkEgressInvoker),
    )
    .await
    .unwrap();
    server.abort();
    assert!(result.error_payload.is_none());
    assert_eq!(result.output_payload["body"], "direct");
}

#[tokio::test]
async fn http_request_explicit_proxy_without_resolver_never_contacts_origin() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plan = http_request_plan(
        json!({"url":format!("http://{}/", listener.local_addr().unwrap()), "timeout_ms":200, "network_egress":{"mode":"proxy","pool_member_id":Uuid::now_v7()}}),
        BTreeMap::new(),
    );
    let result = crate::execution_engine::execute_http_request_node(
        &plan.nodes["node-http"],
        &Default::default(),
        &Default::default(),
        None,
    )
    .await
    .unwrap();
    assert!(result.error_payload.is_some());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn http_request_passes_exact_member_choice_and_old_default_to_host() {
    struct RecordChoice(Mutex<Vec<domain::HttpNodeNetworkEgress>>);
    #[async_trait::async_trait]
    impl ProviderInvoker for RecordChoice {
        async fn invoke_llm(
            &self,
            _: &CompiledLlmRuntime,
            _: ProviderInvocationInput,
        ) -> Result<ProviderInvocationOutput> {
            unreachable!()
        }
        async fn acquire_http_node_client(
            &self,
            choice: &domain::HttpNodeNetworkEgress,
            _: std::time::Duration,
            _: bool,
        ) -> Result<Option<crate::execution_engine::HttpRequestClientLease>> {
            self.0.lock().unwrap().push(choice.clone());
            anyhow::bail!("selected proxy unavailable")
        }
    }
    let invoker = RecordChoice(Mutex::new(Vec::new()));
    let member_id = Uuid::now_v7();
    for config in [
        json!({"url":"http://unused.invalid"}),
        json!({"url":"http://unused.invalid", "network_egress":{"mode":"proxy","pool_member_id":member_id}}),
    ] {
        let plan = http_request_plan(config, BTreeMap::new());
        let result = execute_http_request_node_with_provider_invoker(
            &plan.nodes["node-http"],
            &Default::default(),
            &Default::default(),
            None,
            Some(&invoker),
        )
        .await
        .unwrap();
        assert!(result.error_payload.is_some());
    }
    assert_eq!(
        *invoker.0.lock().unwrap(),
        vec![
            domain::HttpNodeNetworkEgress::Workspace,
            domain::HttpNodeNetworkEgress::Proxy {
                pool_member_id: member_id
            }
        ]
    );
}

#[tokio::test]
async fn http_request_invalid_egress_is_rejected_before_network_access() {
    for choice in [
        json!({"mode":"proxy"}),
        json!({"mode":"proxy","pool_member_id":"not-a-uuid"}),
        json!({"mode":"typo"}),
        json!({"mode":"direct","password":"secret"}),
    ] {
        let plan = http_request_plan(
            json!({"url":"http://unused.invalid","network_egress":choice}),
            BTreeMap::new(),
        );
        let result = crate::execution_engine::execute_http_request_node(
            &plan.nodes["node-http"],
            &Default::default(),
            &Default::default(),
            None,
        )
        .await
        .unwrap();
        assert!(result.error_payload.is_some());
    }
}
