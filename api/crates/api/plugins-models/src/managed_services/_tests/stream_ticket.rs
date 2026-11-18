use super::super::{
    stream,
    stream_ticket::{self, Ticket, TicketBinding},
};
use serde_json::json;
fn ticket() -> Ticket {
    Ticket {
        session: "exact-session".into(),
        actor: uuid::Uuid::now_v7(),
        origin: "https://console.example".into(),
        installation: uuid::Uuid::now_v7(),
        generation: "g1".into(),
        epoch: 0,
        operation: "fixture.terminal".into(),
        payload: json!({"path":{},"body":{"host_id":"h"}}),
    }
}
#[tokio::test]
async fn stream_ticket_atomic_replay_origin_exact_session_and_generation_binding() {
    let cache = storage_ephemeral::MokaCacheStore::new("stream-test", 100);
    let ticket = ticket();
    let token = stream_ticket::store(&cache, &ticket).await.unwrap();
    assert_eq!(token.len(), 64);
    for (session, origin, generation, operation) in [
        (
            "another-session",
            ticket.origin.as_str(),
            "g1",
            ticket.operation.as_str(),
        ),
        (
            ticket.session.as_str(),
            "https://attacker.example",
            "g1",
            ticket.operation.as_str(),
        ),
        (
            ticket.session.as_str(),
            ticket.origin.as_str(),
            "g2",
            ticket.operation.as_str(),
        ),
        (
            ticket.session.as_str(),
            ticket.origin.as_str(),
            "g1",
            "fixture.other",
        ),
    ] {
        assert!(stream_ticket::claim(
            &cache,
            &token,
            TicketBinding {
                session,
                actor: ticket.actor,
                origin,
                installation: ticket.installation,
                generation,
                operation,
            }
        )
        .await
        .is_err());
    }
    let a = stream_ticket::claim(
        &cache,
        &token,
        TicketBinding {
            session: &ticket.session,
            actor: ticket.actor,
            origin: &ticket.origin,
            installation: ticket.installation,
            generation: &ticket.generation,
            operation: &ticket.operation,
        },
    );
    let b = stream_ticket::claim(
        &cache,
        &token,
        TicketBinding {
            session: &ticket.session,
            actor: ticket.actor,
            origin: &ticket.origin,
            installation: ticket.installation,
            generation: &ticket.generation,
            operation: &ticket.operation,
        },
    );
    let (a, b) = tokio::join!(a, b);
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(stream_ticket::claim(
        &cache,
        &token,
        TicketBinding {
            session: &ticket.session,
            actor: ticket.actor,
            origin: &ticket.origin,
            installation: ticket.installation,
            generation: &ticket.generation,
            operation: &ticket.operation,
        }
    )
    .await
    .is_err());
}
#[tokio::test]
async fn mutation_and_hot_generation_revoke_preissued_ticket() {
    let cache = storage_ephemeral::MokaCacheStore::new("stream-test", 100);
    let mut ticket = ticket();
    ticket.epoch = stream::epoch(ticket.installation);
    let token = stream_ticket::store(&cache, &ticket).await.unwrap();
    stream::revoke_installation(ticket.installation);
    assert!(stream_ticket::claim(
        &cache,
        &token,
        TicketBinding {
            session: &ticket.session,
            actor: ticket.actor,
            origin: &ticket.origin,
            installation: ticket.installation,
            generation: &ticket.generation,
            operation: &ticket.operation,
        }
    )
    .await
    .is_err());
    ticket.epoch = stream::epoch(ticket.installation);
    let token = stream_ticket::store(&cache, &ticket).await.unwrap();
    stream::revoke_all();
    assert!(stream_ticket::claim(
        &cache,
        &token,
        TicketBinding {
            session: &ticket.session,
            actor: ticket.actor,
            origin: &ticket.origin,
            installation: ticket.installation,
            generation: &ticket.generation,
            operation: &ticket.operation,
        }
    )
    .await
    .is_err());
}
#[test]
fn browser_origin_is_bound_independently_of_proxy_host() {
    for host in ["127.0.0.1:8080", "127.0.0.1:9000", "console.example"] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("host", host.parse().unwrap());
        headers.insert("origin", "https://console.example".parse().unwrap());
        headers.insert("x-forwarded-host", "unrelated.example".parse().unwrap());
        assert_eq!(
            stream_ticket::origin(&headers).unwrap(),
            "https://console.example"
        );
    }
}

#[test]
fn browser_origin_must_be_a_single_http_origin() {
    assert!(stream_ticket::origin(&axum::http::HeaderMap::new()).is_err());
    for origin in [
        "null",
        "file:///tmp/console",
        "https://console.example/path",
        "https://user@console.example",
        "https://console.example?query",
        "https://console.example#fragment",
        "https://console.example https://other.example",
    ] {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("origin", origin.parse().unwrap());
        assert!(stream_ticket::origin(&headers).is_err(), "{origin}");
    }
    let mut headers = axum::http::HeaderMap::new();
    headers.append("origin", "https://console.example".parse().unwrap());
    headers.append("origin", "https://other.example".parse().unwrap());
    assert!(stream_ticket::origin(&headers).is_err());
}
