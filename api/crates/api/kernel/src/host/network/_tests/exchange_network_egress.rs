use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[tokio::test]
async fn exchange_host_services_network_preserves_redirect_status_and_body() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = BufReader::new(socket);
        let mut request = String::new();
        while !request.ends_with("\r\n\r\n") {
            assert_ne!(
                socket.read_line(&mut request).await.unwrap(),
                0,
                "HTTP request ended before its complete headers"
            );
        }
        assert!(request.starts_with("GET / HTTP/1.1\r\n"));
        socket.write_all(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:1/should-not-follow\r\nContent-Length: 5\r\nConnection: close\r\n\r\nretry").await.unwrap();
    });
    let scope = NetworkEgressHttpClientResolver::direct_for_tests()
        .resolve_managed_plugin_request(
            Uuid::now_v7(),
            Uuid::now_v7(),
            Some(std::time::Duration::from_secs(5)),
        )
        .await
        .unwrap();
    let result: anyhow::Result<_> = async {
        let response = scope
            .http_client()
            .get(format!("http://{address}/"))
            .send()
            .await?;
        assert_eq!(response.status().as_u16(), 307);
        assert_eq!(response.text().await?, "retry");
        Ok(())
    }
    .await;
    scope.finish(result).await.unwrap();
    server.await.unwrap();
}
