use std::convert::Infallible;

use crate::external_route_assembly::HttpRouteState;

use access_control::{ConsoleRouteAssemblyBinding, ConsoleRouteBinding, ConsoleRouteOwnership};
use axum::{
    handler::Handler,
    routing::{get, patch, post, MethodRouter},
    Router,
};

pub struct ConsoleMethodRouter<S> {
    router: MethodRouter<S, Infallible>,
    methods: Vec<(
        &'static str,
        ConsoleRouteOwnership,
        Option<crate::middleware::system_maintenance::SystemMaintenanceRequestClass>,
    )>,
}

pub fn console_get<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: get(handler),
        methods: vec![("GET", ownership, None)],
    }
}

pub fn console_delete<H, T, S>(
    handler: H,
    ownership: ConsoleRouteOwnership,
) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: axum::routing::delete(handler),
        methods: vec![("DELETE", ownership, None)],
    }
}

pub fn console_patch<H, T, S>(
    handler: H,
    ownership: ConsoleRouteOwnership,
) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: patch(handler),
        methods: vec![("PATCH", ownership, None)],
    }
}

pub fn console_post<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: post(handler),
        methods: vec![("POST", ownership, None)],
    }
}

pub fn console_put<H, T, S>(handler: H, ownership: ConsoleRouteOwnership) -> ConsoleMethodRouter<S>
where
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    ConsoleMethodRouter {
        router: axum::routing::put(handler),
        methods: vec![("PUT", ownership, None)],
    }
}

impl<S> ConsoleMethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn delete<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.delete(handler);
        self.methods.push(("DELETE", ownership, None));
        self
    }

    pub fn patch<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.patch(handler);
        self.methods.push(("PATCH", ownership, None));
        self
    }

    pub fn post<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.post(handler);
        self.methods.push(("POST", ownership, None));
        self
    }

    pub fn put<H, T>(mut self, handler: H, ownership: ConsoleRouteOwnership) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.router = self.router.put(handler);
        self.methods.push(("PUT", ownership, None));
        self
    }

    pub fn coordinator_control(mut self) -> Self {
        if let Some((_, _, request_class)) = self.methods.last_mut() {
            *request_class = Some(
                crate::middleware::system_maintenance::SystemMaintenanceRequestClass::CoordinatorControl,
            );
        }
        self
    }
}

#[derive(Clone)]
pub struct ConsoleRouteAssembly<S> {
    router: Router<S>,
    bindings: Vec<ConsoleRouteAssemblyBinding>,
    maintenance_control_routes: Vec<ConsoleRouteBinding>,
}

impl<S> Default for ConsoleRouteAssembly<S>
where
    S: HttpRouteState,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> ConsoleRouteAssembly<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self
    where
        S: HttpRouteState,
    {
        Self {
            router: S::new_router(),
            bindings: Vec::new(),
            maintenance_control_routes: Vec::new(),
        }
    }

    pub fn route(mut self, path: &str, methods: ConsoleMethodRouter<S>) -> Self
    where
        S: HttpRouteState,
    {
        let ConsoleMethodRouter { router, methods } = methods;
        for (method, ownership, request_class) in methods {
            let route = ConsoleRouteBinding {
                method: method.to_string(),
                path: format!("/api/console{path}"),
            };
            if request_class.is_some() {
                self.maintenance_control_routes.push(route.clone());
            }
            self.bindings
                .push(ConsoleRouteAssemblyBinding { route, ownership });
        }
        self.router = S::route_router(
            self.router,
            &crate::external_route_assembly::axum_route_path(path),
            router,
        );
        self
    }

    pub fn merge(mut self, other: Self) -> Self
    where
        S: HttpRouteState,
    {
        self.router = S::merge_routers(self.router, other.router);
        self.bindings.extend(other.bindings);
        self.maintenance_control_routes
            .extend(other.maintenance_control_routes);
        self
    }

    pub fn bindings(&self) -> &[ConsoleRouteAssemblyBinding] {
        &self.bindings
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn external_endpoint_contributions(
        &self,
    ) -> impl Iterator<Item = crate::external_endpoint_catalog::ExternalEndpointContribution> + '_
    {
        self.bindings.iter().map(|binding| {
            crate::external_endpoint_catalog::ExternalEndpointContribution::unclassified_http(
                "console-route-assembly",
                &binding.route.method,
                &binding.route.path,
            )
        })
    }

    pub fn maintenance_control_routes(&self) -> &[ConsoleRouteBinding] {
        &self.maintenance_control_routes
    }

    pub fn into_router(self) -> Router<S> {
        self.router
    }
}

pub fn route_templates_match(left: &str, right: &str) -> bool {
    let left = left.trim_matches('/').split('/').collect::<Vec<_>>();
    let right = right.trim_matches('/').split('/').collect::<Vec<_>>();
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left == &right
                || ((left.starts_with(':') || left.starts_with('{'))
                    && (right.starts_with(':') || right.starts_with('{')))
        })
}
