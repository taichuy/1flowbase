use axum::{extract::Request, handler::Handler, response::Response};
use std::{future::Future, marker::PhantomData, pin::Pin};

type RouteFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// Erases the domain Handler before Kernel constructs the shared Axum route stack.
#[doc(hidden)]
pub struct ErasedRouteHandler<S> {
    handler: Box<dyn ErasedHandler<S>>,
}

trait ErasedHandler<S>: Send + Sync {
    fn clone_box(&self) -> Box<dyn ErasedHandler<S>>;
    fn call(self: Box<Self>, request: Request, state: S) -> RouteFuture;
}

struct TypedRouteHandler<H, T, S> {
    handler: H,
    marker: PhantomData<fn() -> (T, S)>,
}

impl<S: 'static> ErasedRouteHandler<S> {
    pub fn new<H, T>(handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        Self {
            handler: Box::new(TypedRouteHandler {
                handler,
                marker: PhantomData,
            }),
        }
    }
}

impl<H, T, S> ErasedHandler<S> for TypedRouteHandler<H, T, S>
where
    H: Handler<T, S>,
    T: 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandler<S>> {
        Box::new(Self {
            handler: self.handler.clone(),
            marker: PhantomData,
        })
    }

    fn call(self: Box<Self>, request: Request, state: S) -> RouteFuture {
        // Call immediately and retain the original future until its caller drops it,
        // including after Ready. An async/await adapter would change both lifetimes.
        Box::pin(<H as Handler<T, S>>::call(self.handler, request, state))
    }
}

impl<S> Clone for ErasedRouteHandler<S> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone_box(),
        }
    }
}

impl<S: 'static> Handler<(), S> for ErasedRouteHandler<S> {
    type Future = RouteFuture;

    fn call(self, request: Request, state: S) -> Self::Future {
        self.handler.call(request, state)
    }
}

#[cfg(test)]
#[path = "_tests/erased_route_handler.rs"]
mod tests;
