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

struct ObservedBody {
    inner: Body,
    capture: CaptureGuard,
    kind: ClientTrajectoryFrameKind,
}
impl HttpBody for ObservedBody {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(bytes) = frame.data_ref() {
                    this.capture.recorder.record(this.kind, bytes);
                }
                // Hyper need not poll again when the wrapped body reports EOF.
                if this.inner.is_end_stream() {
                    this.capture.finish();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(error))) => {
                this.capture.fail();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                this.capture.finish();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
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
    let mut capture = capture;
    if body.is_end_stream() {
        capture.finish();
    }
    Response::from_parts(
        parts,
        Body::new(ObservedBody {
            inner: body,
            capture,
            kind,
        }),
    )
}

#[cfg(test)]
pub(crate) mod _tests;
