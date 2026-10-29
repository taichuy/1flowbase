use super::super::*;
use serde_json::json;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Only this fixture resolves the reserved target domain, proving SOCKS remote DNS.
pub(super) async fn socks_proxy(
    auth: bool,
    reject: bool,
    tls_port: Option<u16>,
) -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert_eq!(stream.read_u8().await.unwrap(), 5);
            let n = stream.read_u8().await.unwrap();
            let mut methods = vec![0; n as usize];
            stream.read_exact(&mut methods).await.unwrap();
            let method = if auth { 2 } else { 0 };
            assert!(methods.contains(&method));
            stream.write_all(&[5, method]).await.unwrap();
            if auth {
                assert_eq!(stream.read_u8().await.unwrap(), 1);
                let n = stream.read_u8().await.unwrap();
                let mut username = vec![0; n as usize];
                stream.read_exact(&mut username).await.unwrap();
                let n = stream.read_u8().await.unwrap();
                let mut password = vec![0; n as usize];
                stream.read_exact(&mut password).await.unwrap();
                assert_eq!(username, b"alice");
                assert_eq!(password, b"proxy-secret");
                stream.write_all(&[1, u8::from(reject)]).await.unwrap();
                if reject {
                    return;
                }
            }
            let mut header = [0; 4];
            stream.read_exact(&mut header).await.unwrap();
            assert_eq!(
                header,
                [5, 1, 0, 3],
                "the client must send a domain, not a locally resolved IP"
            );
            let n = stream.read_u8().await.unwrap();
            let mut host = vec![0; n as usize];
            stream.read_exact(&mut host).await.unwrap();
            assert_eq!(host, b"proxy-origin.invalid");
            let target_port = stream.read_u16().await.unwrap();
            assert_eq!(target_port, if tls_port.is_some() { 443 } else { 80 });
            stream
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                .await
                .unwrap();
            if let Some(port) = tls_port {
                let mut upstream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
            } else {
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(stream.read_u8().await.unwrap());
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with("GET / HTTP/1.1"));
                assert!(!request.contains("proxy-secret"));
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nproxied",
                    )
                    .await
                    .unwrap();
            }
        })
        .await
        .expect("SOCKS fixture timed out");
    });
    (port, task)
}

struct TlsOrigin {
    child: Child,
    directory: PathBuf,
    port: u16,
}
impl Drop for TlsOrigin {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl TlsOrigin {
    async fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("network-proxy-tls-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&directory).unwrap();
        let status = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                "key.pem",
                "-out",
                "cert.pem",
                "-days",
                "1",
                "-subj",
                "/CN=proxy-origin.invalid",
            ])
            .current_dir(&directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port();
        drop(socket);
        let child = Command::new("openssl")
            .args([
                "s_server",
                "-quiet",
                "-www",
                "-accept",
                &format!("127.0.0.1:{port}"),
                "-key",
                "key.pem",
                "-cert",
                "cert.pem",
            ])
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let origin = Self {
            child,
            directory,
            port,
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("TLS origin did not start");
        origin
    }
}

#[tokio::test]
async fn network_proxy_socks5_http_authentication_and_remote_dns() {
    for (auth, reject) in [(false, false), (true, false), (true, true)] {
        let (port, server) = socks_proxy(auth, reject, None).await;
        let scope = static_proxy_execution_scope(
            domain::StaticProxyProtocol::Socks5,
            json!({
                "host":"127.0.0.1", "port":port,
                "username":if auth { "alice" } else { "" },
                "password":if auth { "proxy-secret" } else { "" }
            }),
        )
        .unwrap()
        .unwrap();
        let result = scope
            .client
            .get("http://proxy-origin.invalid/")
            .timeout(Duration::from_secs(5))
            .send()
            .await;
        if reject {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().text().await.unwrap(), "proxied");
        }
        scope.release().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn network_proxy_socks5_https_tunnel_and_remote_dns() {
    let origin = TlsOrigin::start().await;
    let (port, server) = socks_proxy(true, false, Some(origin.port)).await;
    let scope = static_proxy_execution_scope(
        domain::StaticProxyProtocol::Socks5,
        json!({
            "host":"127.0.0.1", "port":port, "username":"alice", "password":"proxy-secret"
        }),
    )
    .unwrap()
    .unwrap();
    // The local test certificate is self-signed; all traffic still performs a real TLS handshake.
    let lease = scope
        .into_http_request_client_lease(Duration::from_secs(5), false)
        .await
        .unwrap();
    // Exercise the public node executor; it owns and releases the host client.
    let result = run_with_lease(lease, "https://proxy-origin.invalid/").await;
    assert!(result.error_payload.is_none(), "{:?}", result.error_payload);
    assert_eq!(result.output_payload["status_code"], 200);
    assert!(result.output_payload["body"]
        .as_str()
        .unwrap()
        .contains("s_server"));
    server.await.unwrap();
}

pub(super) async fn run_with_lease(
    lease: orchestration_runtime::execution_engine::HttpRequestClientLease,
    url: &str,
) -> orchestration_runtime::execution_engine::HttpRequestNodeExecution {
    struct Invoker(
        std::sync::Mutex<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>>,
    );
    #[async_trait::async_trait]
    impl orchestration_runtime::execution_engine::ProviderInvoker for Invoker {
        async fn invoke_llm(
            &self,
            _: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
            _: plugin_framework::provider_contract::ProviderInvocationInput,
        ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
            unreachable!()
        }
        async fn acquire_http_node_client(
            &self,
            _: &domain::HttpNodeNetworkEgress,
            _: Duration,
            _: bool,
        ) -> Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>>
        {
            Ok(self.0.lock().unwrap().take())
        }
    }
    let node: orchestration_runtime::compiled_plan::CompiledNode = serde_json::from_value(json!({
        "node_id":"http", "node_type":"http_request", "alias":"HTTP", "container_id":null,
        "dependency_node_ids":[], "downstream_node_ids":[], "bindings":{}, "outputs":[],
        "config":{"url":url,"verify_ssl":false,"network_egress":{"mode":"proxy","pool_member_id":Uuid::now_v7()}},
        "plugin_runtime":null,"llm_runtime":null,"code_runtime":null
    })).unwrap();
    orchestration_runtime::execution_engine::execute_http_request_node_with_provider_invoker(
        &node,
        &Default::default(),
        &Default::default(),
        None,
        Some(&Invoker(std::sync::Mutex::new(Some(lease)))),
    )
    .await
    .unwrap()
}
