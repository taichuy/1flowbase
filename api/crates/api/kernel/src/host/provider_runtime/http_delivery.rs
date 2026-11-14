//! HTTP delivery owns its scope independently of execution and trajectory persistence.
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::provider_runtime::TransportConnectionScopeGuard;

/// The guard is created before dispatch and moved here only when headers are ready.
/// In particular, dropping a silent or never-polled body must still detach delivery.
pub(crate) fn bind_response(response: Response, scope: TransportConnectionScopeGuard) -> Response {
    let (parts, inner) = response.into_parts();
    Response::from_parts(parts, Body::new(ScopedBody { inner, scope }))
}

struct ScopedBody {
    inner: Body,
    scope: TransportConnectionScopeGuard,
}

impl HttpBody for ScopedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        let frame = Pin::new(&mut this.inner).poll_frame(cx);
        if matches!(&frame, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            this.scope.close();
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for ScopedBody {
    fn drop(&mut self) {
        // Begin detachment before inner-body Drop can hand off archive completion.
        self.scope.close();
    }
}
