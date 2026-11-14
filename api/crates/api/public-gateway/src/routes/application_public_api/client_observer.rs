//! Client protocol evidence is captured after projection, independently of business completion.
use crate::app_state::ApiState;
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use control_plane::client_trajectory::{
    ClientTrajectoryFrameKind, ClientTrajectoryRecorder, ClientTrajectoryTransport,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub(super) fn recorder(
    state: &ApiState,
    transport: ClientTrajectoryTransport,
) -> ClientTrajectoryRecorder {
    ClientTrajectoryRecorder::new(Arc::new(state.store.clone()), transport)
}

/// One transport owner finishes the recorder. Clones used for run correlation do not own EOF.
pub(super) struct CaptureGuard {
    pub(super) recorder: ClientTrajectoryRecorder,
    finished: bool,
}
impl CaptureGuard {
    pub(super) fn new(recorder: ClientTrajectoryRecorder) -> Self {
        Self {
            recorder,
            finished: false,
        }
    }
    pub(super) fn finish(&mut self) {
        if !self.finished {
            self.recorder.finish();
            self.finished = true;
        }
    }
    pub(super) async fn complete(&mut self) {
        self.finish();
        if let Err(error) = self.recorder.complete().await {
            tracing::warn!(request_id=%self.recorder.capture_id(), %error, "client capture durable completion failed");
        }
    }
    pub(super) fn fail(&mut self) {
        if !self.finished {
            self.recorder.mark_incomplete();
            self.finish();
        }
    }
}
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        self.fail();
    }
}

type CaptureFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;
struct ObservedBody {
    inner: Body,
    capture: CaptureGuard,
    kind: ClientTrajectoryFrameKind,
    pending: Option<CaptureFuture>,
    frame: Option<Result<Frame<Bytes>, axum::Error>>,
    eof: bool,
    done: bool,
}
impl Drop for ObservedBody {
    fn drop(&mut self) {
        // A polled frame remains owned by its admission future during disconnect.
        // Detach that finite frame and await it before signaling recorder EOF.
        let pending = self.pending.take();
        let recorder = self.capture.recorder.clone();
        self.capture.finished = true;
        tokio::spawn(async move {
            if let Some(pending) = pending {
                if pending.await.is_err() {
                    recorder.mark_incomplete();
                }
            }
            if let Err(error) = recorder.complete().await {
                tracing::warn!(%error,"disconnected client archive completion failed");
            }
        });
    }
}
impl HttpBody for ObservedBody {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        loop {
            if let Some(pending) = this.pending.as_mut() {
                match pending.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(result) => {
                        this.pending = None;
                        if let Err(error) = result {
                            this.capture.recorder.mark_incomplete();
                            tracing::warn!(%error,"client archive persistence failed");
                        }
                        if this.eof {
                            this.done = true;
                            this.capture.finished = true;
                        }
                        if let Some(frame) = this.frame.take() {
                            return Poll::Ready(Some(frame));
                        }
                        if this.done {
                            return Poll::Ready(None);
                        }
                    }
                }
            }
            if this.done {
                return Poll::Ready(None);
            }
            match Pin::new(&mut this.inner).poll_frame(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(frame) => {
                    this.eof = frame.is_none() || this.inner.is_end_stream();
                    let bytes = frame
                        .as_ref()
                        .and_then(|f| f.as_ref().ok())
                        .and_then(|f| f.data_ref())
                        .cloned();
                    if frame.as_ref().is_some_and(|f| f.is_err()) {
                        this.capture.recorder.mark_incomplete();
                        this.eof = true;
                    }
                    this.frame = frame;
                    let recorder = this.capture.recorder.clone();
                    let kind = this.kind;
                    let eof = this.eof;
                    this.pending = Some(Box::pin(async move {
                        if let Some(bytes) = bytes {
                            recorder.record(kind, &bytes).await?;
                        }
                        if eof {
                            recorder.complete().await?;
                        }
                        Ok(())
                    }));
                }
            }
        }
    }
    fn is_end_stream(&self) -> bool {
        self.done && self.frame.is_none()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

pub(super) fn observe_response(response: Response, capture: CaptureGuard) -> Response {
    let kind = if response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .is_some_and(|value| value.as_bytes().starts_with(b"text/event-stream"))
    {
        ClientTrajectoryFrameKind::ResponseSse
    } else {
        ClientTrajectoryFrameKind::ResponseJson
    };
    let (parts, body) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(ObservedBody {
            inner: body,
            capture,
            kind,
            pending: None,
            frame: None,
            eof: false,
            done: false,
        }),
    )
}

#[cfg(test)]
pub(crate) mod _tests;

pub(super) async fn correlate_blocking_capture(
    state: &ApiState,
    recorder: &ClientTrajectoryRecorder,
    flow_run_id: uuid::Uuid,
) {
    use control_plane::ports::{ProviderTrajectoryRepository, TrajectorySelection};
    recorder.bind_run(flow_run_id, None);
    let mut cursor = None;
    loop {
        let page = match state
            .store
            .provider_trajectory_filtered_page(
                flow_run_id,
                None,
                cursor,
                100,
                TrajectorySelection {
                    request_id: Some(recorder.capture_id()),
                    target_id: None,
                },
            )
            .await
        {
            Ok(page) => page,
            Err(error) => {
                recorder.mark_incomplete();
                tracing::warn!(%flow_run_id, %error, "blocking client node correlation failed");
                return;
            }
        };
        for item in page.items {
            if let Some(node_run_id) = item.metadata["node_run_id"]
                .as_str()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
            {
                recorder.link_llm_node(flow_run_id, node_run_id);
            }
        }
        match page.next_cursor {
            Some(next) if cursor.is_none_or(|previous| next > previous) => cursor = Some(next),
            Some(_) => {
                recorder.mark_incomplete();
                return;
            }
            None => return,
        }
    }
}
