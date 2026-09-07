//! External HTTP construction boundary. A mount cannot discard its route inventory.
use axum::{handler::Handler, routing::MethodRouter, Router};

use crate::external_endpoint_catalog::ExternalEndpointContribution;

pub(crate) struct ExternalMethodRouter<S> {
    router: MethodRouter<S>,
    methods: Vec<(&'static str, Option<&'static str>)>,
}

macro_rules! method {
    ($name:ident, $verb:literal) => {
        pub(crate) fn $name<H, T, S>(handler: H) -> ExternalMethodRouter<S>
        where
            H: Handler<T, S>,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            ExternalMethodRouter {
                router: axum::routing::$name(handler),
                methods: vec![($verb, None)],
            }
        }
    };
}
method!(get, "GET");
method!(post, "POST");
method!(any, "ANY");

/// A JSON-RPC POST carrier retains the canonical MCP invocation binding as it is nested.
pub(crate) fn mcp_post<H, T, S>(handler: H, binding_id: &'static str) -> ExternalMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ExternalMethodRouter {
        router: axum::routing::post(handler),
        methods: vec![("POST", Some(binding_id))],
    }
}

impl<S: Clone + Send + Sync + 'static> ExternalMethodRouter<S> {
    pub(crate) fn post<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.post(handler);
        self.methods.push(("POST", None));
        self
    }
}

pub(crate) struct ExternalRouteAssembly<S> {
    router: Router<S>,
    routes: Vec<(String, String, Option<&'static str>)>,
}

impl<S: Clone + Send + Sync + 'static> ExternalRouteAssembly<S> {
    pub(crate) fn new() -> Self {
        Self {
            router: Router::new(),
            routes: Vec::new(),
        }
    }

    pub(crate) fn route(mut self, path: &str, methods: ExternalMethodRouter<S>) -> Self {
        self.routes.extend(
            methods
                .methods
                .into_iter()
                .map(|(method, binding)| (method.to_string(), path.to_string(), binding)),
        );
        self.router = self.router.route(path, methods.router);
        self
    }

    pub(crate) fn merge(mut self, other: Self) -> Self {
        self.routes.extend(other.routes);
        self.router = self.router.merge(other.router);
        self
    }

    pub(crate) fn nest(mut self, prefix: &str, other: Self) -> Self {
        self.routes
            .extend(other.routes.into_iter().map(|(method, path, binding)| {
                (
                    method,
                    format!("{}{path}", prefix.trim_end_matches('/')),
                    binding,
                )
            }));
        self.router = self.router.nest(prefix, other.router);
        self
    }

    // The sole adapter for the existing Console construction boundary; no raw Router input.
    pub(crate) fn console(
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
    pub(crate) fn docs() -> Self {
        Self {
            router: utoipa_swagger_ui::SwaggerUi::new("/docs")
                .config(utoipa_swagger_ui::Config::from("/openapi.json"))
                .into(),
            routes: ["/docs", "/docs/*rest"]
                .into_iter()
                .map(|path| ("GET".to_string(), path.to_string(), None))
                .collect(),
        }
    }

    pub(crate) fn contributions(&self) -> Vec<ExternalEndpointContribution> {
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

    pub(crate) fn into_router(self) -> Router<S> {
        self.router
    }
}
