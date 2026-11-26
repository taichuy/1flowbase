//! Connection helper.
use std::{collections::VecDeque, future::Future, io, net::SocketAddr, time::Duration};

use futures_util::{stream::FuturesUnordered, StreamExt};
use tokio::net::TcpStream;
use tokio::time::{sleep_until, Instant};

use tungstenite::{
    error::{Error, UrlError},
    handshake::client::{Request, Response},
    protocol::WebSocketConfig,
};

use crate::{domain, stream::MaybeTlsStream, Connector, IntoClientRequest, WebSocketStream};

const HAPPY_EYEBALLS_DELAY: Duration = Duration::from_millis(250);

/// Connect to a given URL.
///
/// Accepts any request that implements [`IntoClientRequest`], which is often just `&str`, but can
/// be a variety of types such as `httparse::Request` or [`tungstenite::http::Request`] for more
/// complex uses.
///
/// ```no_run
/// # use tungstenite::client::IntoClientRequest;
///
/// # async fn test() {
/// use tungstenite::http::{Method, Request};
/// use tokio_tungstenite::connect_async;
///
/// let mut request = "wss://api.example.com".into_client_request().unwrap();
/// request.headers_mut().insert("api-key", "42".parse().unwrap());
///
/// let (stream, response) = connect_async(request).await.unwrap();
/// # }
/// ```
pub async fn connect_async<R>(
    request: R,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
where
    R: IntoClientRequest + Unpin,
{
    connect_async_with_config(request, None, false).await
}

/// The same as `connect_async()` but the one can specify a websocket configuration.
/// Please refer to `connect_async()` for more details. `disable_nagle` specifies if
/// the Nagle's algorithm must be disabled, i.e. `set_nodelay(true)`. If you don't know
/// what the Nagle's algorithm is, better leave it set to `false`.
///
/// When the `proxy` feature is enabled, this function honors `HTTP_PROXY`, `HTTPS_PROXY`,
/// `ALL_PROXY`, and `NO_PROXY` (case-insensitive) for client connections.
pub async fn connect_async_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
where
    R: IntoClientRequest + Unpin,
{
    connect(request.into_client_request()?, config, disable_nagle, None).await
}

/// The same as `connect_async()` but the one can specify a websocket configuration,
/// and a TLS connector to use. Please refer to `connect_async()` for more details.
/// `disable_nagle` specifies if the Nagle's algorithm must be disabled, i.e.
/// `set_nodelay(true)`. If you don't know what the Nagle's algorithm is, better
/// leave it to `false`.
#[cfg(any(feature = "native-tls", feature = "__rustls-tls"))]
pub async fn connect_async_tls_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    connector: Option<Connector>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
where
    R: IntoClientRequest + Unpin,
{
    connect(request.into_client_request()?, config, disable_nagle, connector).await
}

async fn connect(
    request: Request,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    connector: Option<Connector>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error> {
    let domain = domain(&request)?;
    let port = request
        .uri()
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or(Error::Url(UrlError::UnsupportedUrlScheme))?;

    let socket = connect_socket(&request, &domain, port).await?;

    if disable_nagle {
        socket.set_nodelay(true)?;
    }

    crate::tls::client_async_tls_with_config(request, socket, config, connector).await
}

async fn connect_socket(request: &Request, domain: &str, port: u16) -> Result<TcpStream, Error> {
    #[cfg(feature = "proxy")]
    if let Some(proxy) = tungstenite::proxy::ProxyConfig::from_env(request.uri())? {
        let addr = proxy.authority();
        let socket = connect_happy_eyeballs(addr).await.map_err(Error::Io)?;
        return crate::proxy::connect_via_proxy(socket, &proxy, domain, port).await;
    }

    let addr = format!("{domain}:{port}");
    connect_happy_eyeballs(addr).await.map_err(Error::Io)
}

async fn connect_happy_eyeballs(addr: impl tokio::net::ToSocketAddrs) -> io::Result<TcpStream> {
    let addrs = tokio::net::lookup_host(addr).await?.collect::<Vec<_>>();
    happy_eyeballs_connect(addrs, TcpStream::connect).await
}

async fn happy_eyeballs_connect<T, F, Fut>(addrs: Vec<SocketAddr>, mut connect: F) -> io::Result<T>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = io::Result<T>>,
{
    let mut addrs = interleave_addresses(addrs);
    let Some(first_addr) = addrs.pop_front() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any address",
        ));
    };

    let mut attempts = FuturesUnordered::new();
    attempts.push(connect(first_addr));
    let mut next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
    let mut last_err = None;

    loop {
        if addrs.is_empty() {
            match attempts.next().await {
                Some(Ok(stream)) => return Ok(stream),
                Some(Err(err)) => {
                    last_err = Some(err);
                    if attempts.is_empty() {
                        return Err(last_err.expect("an attempt just failed"));
                    }
                }
                None => return Err(last_err.expect("at least one attempt was started")),
            }
            continue;
        }

        tokio::select! {
            result = attempts.next() => {
                match result {
                    Some(Ok(stream)) => return Ok(stream),
                    Some(Err(err)) => {
                        last_err = Some(err);
                        let next_addr = addrs.pop_front().expect("addresses checked above");
                        attempts.push(connect(next_addr));
                        next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
                    }
                    None => {
                        let next_addr = addrs.pop_front().expect("addresses checked above");
                        attempts.push(connect(next_addr));
                        next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
                    }
                }
            }
            _ = sleep_until(next_attempt_at) => {
                let next_addr = addrs.pop_front().expect("addresses checked above");
                attempts.push(connect(next_addr));
                next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
            }
        }
    }
}

fn interleave_addresses(addrs: Vec<SocketAddr>) -> VecDeque<SocketAddr> {
    let mut addrs = addrs.into_iter();
    let Some(first) = addrs.next() else {
        return VecDeque::new();
    };

    let first_is_ipv4 = first.is_ipv4();
    let mut preferred = VecDeque::new();
    preferred.push_back(first);
    let mut alternate = VecDeque::new();
    for addr in addrs {
        if addr.is_ipv4() == first_is_ipv4 {
            preferred.push_back(addr);
        } else {
            alternate.push_back(addr);
        }
    }

    let mut interleaved = VecDeque::new();
    while !preferred.is_empty() || !alternate.is_empty() {
        if let Some(addr) = preferred.pop_front() {
            interleaved.push_back(addr);
        }
        if let Some(addr) = alternate.pop_front() {
            interleaved.push_back(addr);
        }
    }
    interleaved
}

#[cfg(test)]
mod tests {
    use super::{happy_eyeballs_connect, interleave_addresses, HAPPY_EYEBALLS_DELAY};
    use std::{future::pending, io, net::SocketAddr, time::Duration};
    use tokio::net::{TcpListener, TcpStream};

    #[test]
    fn interleaves_address_families_without_changing_family_order() {
        let v6_a = "[::1]:1".parse::<SocketAddr>().unwrap();
        let v6_b = "[::1]:2".parse::<SocketAddr>().unwrap();
        let v4_a = "127.0.0.1:1".parse::<SocketAddr>().unwrap();
        let v4_b = "127.0.0.1:2".parse::<SocketAddr>().unwrap();

        assert_eq!(
            interleave_addresses(vec![v6_a, v6_b, v4_a, v4_b]).into_iter().collect::<Vec<_>>(),
            vec![v6_a, v4_a, v6_b, v4_b]
        );
    }

    #[tokio::test]
    async fn starts_alternate_family_when_first_address_stalls() {
        let stalled_v6 = "[::1]:1".parse::<SocketAddr>().unwrap();
        let reachable_v4 = "127.0.0.1:1".parse::<SocketAddr>().unwrap();
        let started = tokio::time::Instant::now();

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            happy_eyeballs_connect(vec![stalled_v6, reachable_v4], |addr| async move {
                if addr == stalled_v6 {
                    pending::<io::Result<&'static str>>().await
                } else {
                    Ok("ipv4")
                }
            }),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(result, "ipv4");
        assert!(started.elapsed() >= HAPPY_EYEBALLS_DELAY);
    }

    #[tokio::test(start_paused = true)]
    async fn starts_next_address_immediately_when_attempt_fails() {
        let failed_v6 = "[::1]:1".parse::<SocketAddr>().unwrap();
        let reachable_v4 = "127.0.0.1:1".parse::<SocketAddr>().unwrap();
        let started = tokio::time::Instant::now();

        let result = happy_eyeballs_connect(vec![failed_v6, reachable_v4], |addr| async move {
            if addr == failed_v6 {
                Err(io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused"))
            } else {
                Ok("ipv4")
            }
        })
        .await
        .unwrap();

        assert_eq!(result, "ipv4");
        assert!(started.elapsed() < HAPPY_EYEBALLS_DELAY);
    }

    #[tokio::test]
    async fn connects_to_reachable_ipv4_after_unreachable_ipv6() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let reachable_v4 = listener.local_addr().unwrap();
        let unreachable_v6 = "[2001:db8::1]:443".parse::<SocketAddr>().unwrap();
        let accepted = tokio::spawn(async move { listener.accept().await.unwrap() });

        let stream = tokio::time::timeout(
            Duration::from_secs(1),
            happy_eyeballs_connect(vec![unreachable_v6, reachable_v4], TcpStream::connect),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(stream.peer_addr().unwrap(), reachable_v4);
        accepted.await.unwrap();
    }
}
