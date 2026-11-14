//! External HTTP construction boundary. A mount cannot discard its route inventory.
use axum::{
    handler::Handler,
    routing::{MethodFilter, MethodRouter},
    Router,
};

#[path = "erased_route_handler.rs"]
mod erased_route_handler;
#[doc(hidden)]
pub use erased_route_handler::ErasedRouteHandler;

use crate::external_endpoint_catalog::ExternalEndpointContribution;

/// Concrete HTTP states own shared router code generation at the Kernel boundary.
pub trait HttpRouteState: Clone + Send + Sync + 'static {
    fn method_router(filter: MethodFilter, handler: ErasedRouteHandler<Self>)
        -> MethodRouter<Self>;
    fn method_router_on(
        router: MethodRouter<Self>,
        filter: MethodFilter,
        handler: ErasedRouteHandler<Self>,
    ) -> MethodRouter<Self>;
    fn any_method_router(handler: ErasedRouteHandler<Self>) -> MethodRouter<Self>;
    fn new_router() -> Router<Self>;
    fn route_router(router: Router<Self>, path: &str, methods: MethodRouter<Self>) -> Router<Self>;
    fn merge_routers(router: Router<Self>, other: Router<Self>) -> Router<Self>;
    fn nest_router(router: Router<Self>, prefix: &str, other: Router<Self>) -> Router<Self>;
    fn limit_router_body(
        router: Router<Self>,
        limit: axum::extract::DefaultBodyLimit,
    ) -> Router<Self>;
    fn docs_router() -> Router<Self>;
}

// Expand only concrete, non-generic implementations; no default/blanket operation body.
macro_rules! http_route_state {
    ($state:ty) => {
        impl HttpRouteState for $state {
            fn method_router(
                filter: MethodFilter,
                handler: ErasedRouteHandler<Self>,
            ) -> MethodRouter<Self> {
                MethodRouter::new().on(filter, handler)
            }

            fn method_router_on(
                router: MethodRouter<Self>,
                filter: MethodFilter,
                handler: ErasedRouteHandler<Self>,
            ) -> MethodRouter<Self> {
                router.on(filter, handler)
            }

            fn any_method_router(handler: ErasedRouteHandler<Self>) -> MethodRouter<Self> {
                axum::routing::any(handler)
            }

            fn new_router() -> Router<Self> {
                Router::new()
            }

            fn route_router(
                router: Router<Self>,
                path: &str,
                methods: MethodRouter<Self>,
            ) -> Router<Self> {
                router.route(path, methods)
            }

            fn merge_routers(router: Router<Self>, other: Router<Self>) -> Router<Self> {
                router.merge(other)
            }

            fn nest_router(
                router: Router<Self>,
                prefix: &str,
                other: Router<Self>,
            ) -> Router<Self> {
                router.nest(prefix, other)
            }

            fn limit_router_body(
                router: Router<Self>,
                limit: axum::extract::DefaultBodyLimit,
            ) -> Router<Self> {
                router.layer(limit)
            }

            fn docs_router() -> Router<Self> {
                utoipa_swagger_ui::SwaggerUi::new("/docs")
                    .config(utoipa_swagger_ui::Config::from("/openapi.json"))
                    .into()
            }
        }
    };
}

http_route_state!(std::sync::Arc<crate::app_state::ApiState>);
http_route_state!(());

/// Canonical endpoint and authorization templates retain `:name` / `*rest`.
/// Only the protocol router consumes Axum's brace syntax.
pub(crate) fn axum_route_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if let Some(parameter) = segment.strip_prefix(':') {
                format!("{{{parameter}}}")
            } else if let Some(catch_all) = segment.strip_prefix('*') {
                format!("{{*{catch_all}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub struct ExternalMethodRouter<S> {
    router: MethodRouter<S>,
    methods: Vec<(&'static str, Option<&'static str>)>,
}

macro_rules! method {
    ($name:ident, $verb:literal, $filter:ident) => {
        pub fn $name<H, T, S>(handler: H) -> ExternalMethodRouter<S>
        where
            H: Handler<T, S>,
            T: 'static,
            S: HttpRouteState,
        {
            ExternalMethodRouter {
                router: S::method_router(MethodFilter::$filter, ErasedRouteHandler::new(handler)),
                methods: vec![($verb, None)],
            }
        }
    };
}
method!(get, "GET", GET);
method!(post, "POST", POST);

pub fn any<H, T, S>(handler: H) -> ExternalMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: HttpRouteState,
{
    ExternalMethodRouter {
        router: S::any_method_router(ErasedRouteHandler::new(handler)),
        methods: vec![("ANY", None)],
    }
}

/// A JSON-RPC POST carrier retains the canonical MCP invocation binding as it is nested.
pub fn mcp_post<H, T, S>(handler: H, binding_id: &'static str) -> ExternalMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: HttpRouteState,
{
    ExternalMethodRouter {
        router: S::method_router(MethodFilter::POST, ErasedRouteHandler::new(handler)),
        methods: vec![("POST", Some(binding_id))],
    }
}

impl<S: Clone + Send + Sync + 'static> ExternalMethodRouter<S> {
    pub fn post<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
        S: HttpRouteState,
    {
        self.router = S::method_router_on(
            self.router,
            MethodFilter::POST,
            ErasedRouteHandler::new(handler),
        );
        self.methods.push(("POST", None));
        self
    }
}

pub struct ExternalRouteAssembly<S> {
    router: Router<S>,
    routes: Vec<(String, String, Option<&'static str>)>,
}

impl<S: Clone + Send + Sync + 'static> ExternalRouteAssembly<S> {
    pub fn new() -> Self
    where
        S: HttpRouteState,
    {
        Self {
            router: S::new_router(),
            routes: Vec::new(),
        }
    }

    pub fn route(mut self, path: &str, methods: ExternalMethodRouter<S>) -> Self
    where
        S: HttpRouteState,
    {
        self.routes.extend(
            methods
                .methods
                .into_iter()
                .map(|(method, binding)| (method.to_string(), path.to_string(), binding)),
        );
        self.router = S::route_router(self.router, &axum_route_path(path), methods.router);
        self
    }

    pub fn merge(mut self, other: Self) -> Self
    where
        S: HttpRouteState,
    {
        self.routes.extend(other.routes);
        self.router = S::merge_routers(self.router, other.router);
        self
    }

    /// Apply an extractor policy without losing the mounted endpoint inventory.
    pub fn default_body_limit(mut self, limit: axum::extract::DefaultBodyLimit) -> Self
    where
        S: HttpRouteState,
    {
        self.router = S::limit_router_body(self.router, limit);
        self
    }

    pub fn nest(mut self, prefix: &str, other: Self) -> Self
    where
        S: HttpRouteState,
    {
        self.routes
            .extend(other.routes.into_iter().map(|(method, path, binding)| {
                (
                    method,
                    format!("{}{path}", prefix.trim_end_matches('/')),
                    binding,
                )
            }));
        self.router = S::nest_router(self.router, &axum_route_path(prefix), other.router);
        self
    }

    // The sole adapter for the existing Console construction boundary; no raw Router input.
    pub fn console(
        assembly: crate::routes::console_route_assembly::ConsoleRouteAssembly<S>,
    ) -> Self {
        let routes = assembly
            .bindings()
            .iter()
            .map(|binding| {
                (
                    binding.route.method.clone(),
                    binding
                        .route
                        .path
                        .strip_prefix("/api/console")
                        .expect("console assembly owns its prefix")
                        .to_string(),
                    None,
                )
            })
            .collect();
        Self {
            router: assembly.into_router(),
            routes,
        }
    }

    // Swagger owns these three routes in utoipa-swagger-ui; arbitrary Router conversion is absent.
    pub fn docs() -> Self
    where
        S: HttpRouteState,
    {
        Self {
            router: S::docs_router(),
            routes: ["/docs", "/docs/*rest"]
                .into_iter()
                .map(|path| ("GET".to_string(), path.to_string(), None))
                .collect(),
        }
    }

    pub fn contributions(&self) -> Vec<ExternalEndpointContribution> {
        self.routes
            .iter()
            .map(|(method, path, binding)| {
                if let Some(binding) = binding {
                    return ExternalEndpointContribution::mcp_http_carrier(
                        "external-route-assembly",
                        method,
                        path,
                        binding,
                    );
                }
                ExternalEndpointContribution::unclassified_http(
                    "external-route-assembly",
                    method,
                    path,
                )
            })
            .collect()
    }

    pub fn into_router(self) -> Router<S> {
        self.router
    }
}
