use crate::{app_state::ApiState, error_response::ApiError};
pub use api_host_kernel::routes::notifications::events::*;
use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use std::{convert::Infallible, sync::Arc};

// Protocol control: this transport carries only an inbox-refresh signal and no business payload.
#[utoipa::path(
    get,
    path = "/api/console/notifications/events",
    operation_id = "notifications.events.subscribe",
    tag = "Notifications",
    summary = "Subscribe to notification refresh signals",
    description = "Open an authenticated recipient-scoped protocol stream carrying only inbox refresh signals; reconnect readers load database facts.",
    responses((status = 200, description = "SSE notification.changed events with data {} and keep-alive comments; no business completion payload.", body = String, content_type = "text/event-stream"))
)]
pub async fn events(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let context = crate::middleware::require_session::require_session(&state, &headers).await?;
    let receiver = state
        .notification_events
        .subscribe(context.actor.current_workspace_id, context.actor.user_id);
    let stream =
        futures_util::stream::unfold((receiver, true), |(mut receiver, initial)| async move {
            if !initial {
                match receiver.receiver.changed().await {
                    Ok(()) => {}
                    Err(_) => return None,
                }
            }
            Some((
                Ok::<_, Infallible>(Event::default().event("notification.changed").data("{}")),
                (receiver, false),
            ))
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
