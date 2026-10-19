use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRouteDefinition {
    pub extension_id: String,
    pub route_id: String,
    pub method: String,
    pub path: String,
    pub resource_code: String,
    pub action_code: String,
}

#[derive(Debug, Default)]
pub struct HostRouteRegistry {
    routes_by_id: HashMap<String, HostRouteDefinition>,
    route_keys: HashSet<(String, String)>,
}

impl HostRouteRegistry {
    pub fn register(&mut self, route: HostRouteDefinition) -> Result<(), HostRouteRegistryError> {
        validate_non_empty(&route.extension_id, "extension_id")?;
        validate_non_empty(&route.route_id, "route_id")?;
        validate_non_empty(&route.method, "method")?;
        validate_non_empty(&route.resource_code, "resource_code")?;
        validate_non_empty(&route.action_code, "action_code")?;
        if !is_controlled_host_route_path(&route.path) {
            return Err(HostRouteRegistryError::new(
                "host extension route must use a controlled route prefix",
            ));
        }
        if self.routes_by_id.contains_key(&route.route_id) {
            return Err(HostRouteRegistryError::new(format!(
                "duplicate route id {}",
                route.route_id
            )));
        }

        let route_key = (route.method.to_ascii_uppercase(), route.path.clone());
        if self.route_keys.contains(&route_key) {
            return Err(HostRouteRegistryError::new(format!(
                "duplicate route path {} {}",
                route.method, route.path
            )));
        }

        self.route_keys.insert(route_key);
        self.routes_by_id.insert(route.route_id.clone(), route);
        Ok(())
    }

    pub fn routes(&self) -> impl Iterator<Item = &HostRouteDefinition> {
        self.routes_by_id.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRouteRegistryError {
    message: String,
}

impl HostRouteRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for HostRouteRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for HostRouteRegistryError {}

fn validate_non_empty(value: &str, field: &str) -> Result<(), HostRouteRegistryError> {
    if value.trim().is_empty() {
        return Err(HostRouteRegistryError::new(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn is_controlled_host_route_path(path: &str) -> bool {
    path.starts_with("/api/system/")
        || path.starts_with("/api/callbacks/")
        || path.starts_with("/api/public/auth/")
}
