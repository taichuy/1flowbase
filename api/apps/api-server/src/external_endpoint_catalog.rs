use std::collections::{BTreeMap, BTreeSet};

use interface_runtime::{CompiledInterfaceRegistry, ProtocolBinding, ProtocolProjection};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExternalEndpointClassification {
    CanonicalBusinessInterface,
    ProtocolControl,
    OperationalControl,
    Unclassified,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExternalEndpointIdentity {
    Http {
        method: String,
        route_template: String,
        variant: Option<String>,
    },
    Mcp {
        protocol_method: String,
        variant: Option<String>,
    },
}

impl ExternalEndpointIdentity {
    pub(crate) fn http(method: &str, route_template: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: None,
        }
    }

    pub(crate) fn http_variant(method: &str, route_template: &str, variant: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: Some(variant.to_string()),
        }
    }

    pub(crate) fn mcp(protocol_method: &str) -> Self {
        Self::Mcp {
            protocol_method: protocol_method.to_string(),
            variant: None,
        }
    }

    fn http_control_variant(method: &str, route_template: &str, variant: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: Some(variant.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalEndpointContribution {
    identity: ExternalEndpointIdentity,
    source: String,
    classification: ExternalEndpointClassification,
    binding_id: Option<String>,
}

impl ExternalEndpointContribution {
    pub(crate) fn unclassified_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            classification: ExternalEndpointClassification::Unclassified,
            binding_id: None,
        }
    }

    fn protocol_control_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            classification: ExternalEndpointClassification::ProtocolControl,
            binding_id: None,
        }
    }

    fn operational_control_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            classification: ExternalEndpointClassification::OperationalControl,
            binding_id: None,
        }
    }

    fn protocol_control_identity(source: &str, identity: ExternalEndpointIdentity) -> Self {
        Self {
            identity,
            source: source.to_string(),
            classification: ExternalEndpointClassification::ProtocolControl,
            binding_id: None,
        }
    }

    fn from_binding(source: &str, binding: &ProtocolBinding) -> Option<Self> {
        let identity = match binding.projection() {
            ProtocolProjection::Http(route) => {
                ExternalEndpointIdentity::http(route.method(), route.path())
            }
            ProtocolProjection::HttpVariant { route, variant } => {
                ExternalEndpointIdentity::http_variant(route.method(), route.path(), variant)
            }
            ProtocolProjection::Mcp { tool } => ExternalEndpointIdentity::mcp(tool),
            ProtocolProjection::Internal { .. } | ProtocolProjection::Worker { .. } => return None,
        };
        Some(Self {
            identity,
            source: source.to_string(),
            classification: ExternalEndpointClassification::CanonicalBusinessInterface,
            binding_id: Some(binding.binding_id().as_str().to_string()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalEndpointRow {
    identity: ExternalEndpointIdentity,
    sources: BTreeSet<String>,
    classification: ExternalEndpointClassification,
    binding_id: Option<String>,
}

impl ExternalEndpointRow {
    #[cfg(test)]
    pub(crate) fn classification(&self) -> ExternalEndpointClassification {
        self.classification
    }

    #[cfg(test)]
    pub(crate) fn sources(&self) -> &BTreeSet<String> {
        &self.sources
    }

    #[cfg(test)]
    pub(crate) fn binding_id(&self) -> Option<&str> {
        self.binding_id.as_deref()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ExternalEndpointCatalogError {
    #[error("duplicate external endpoint contribution `{origin}` for {identity:?}")]
    DuplicateContribution {
        origin: String,
        identity: ExternalEndpointIdentity,
    },
    #[error("conflicting external endpoint classifications for {identity:?}")]
    ConflictingClassification { identity: ExternalEndpointIdentity },
    #[error("conflicting canonical bindings for {identity:?}: `{first}` and `{second}`")]
    ConflictingBinding {
        identity: ExternalEndpointIdentity,
        first: String,
        second: String,
    },
    #[error("external endpoint is not an approved control: {identity:?}")]
    #[cfg(test)]
    UnknownControl { identity: ExternalEndpointIdentity },
    #[error("external endpoint catalog contains unclassified rows: {identities:?}")]
    UnclassifiedRows {
        identities: Vec<ExternalEndpointIdentity>,
    },
    #[error("external endpoint {identity:?} references unknown binding `{binding_id}`")]
    UnknownBinding {
        identity: ExternalEndpointIdentity,
        binding_id: String,
    },
    #[error("OpenAPI endpoint inventory is invalid: {0}")]
    InvalidOpenApi(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApprovedExternalControl {
    Health,
    ConsoleHealth,
    Docs,
    OpenApi,
    McpInitialize,
    McpInitializedNotification,
    NativeWebSocketUpgrade,
    ResponsesWebSocketUpgrade,
    NativeSseKeepalive,
    CompatibilitySseKeepalive,
    AssistantWebSocketUpgrade,
}

impl ApprovedExternalControl {
    const ALL: [Self; 11] = [
        Self::Health,
        Self::ConsoleHealth,
        Self::Docs,
        Self::OpenApi,
        Self::McpInitialize,
        Self::McpInitializedNotification,
        Self::NativeWebSocketUpgrade,
        Self::ResponsesWebSocketUpgrade,
        Self::NativeSseKeepalive,
        Self::CompatibilitySseKeepalive,
        Self::AssistantWebSocketUpgrade,
    ];

    fn contribution(self) -> ExternalEndpointContribution {
        match self {
            Self::Health => ExternalEndpointContribution::operational_control_http(
                "root-operational-control",
                "GET",
                "/health",
            ),
            Self::ConsoleHealth => ExternalEndpointContribution::operational_control_http(
                "console-operational-control",
                "GET",
                "/api/console/health",
            ),
            Self::Docs => ExternalEndpointContribution::protocol_control_http(
                "swagger-protocol-control",
                "GET",
                "/docs",
            ),
            Self::OpenApi => ExternalEndpointContribution::protocol_control_http(
                "openapi-protocol-control",
                "GET",
                "/openapi.json",
            ),
            Self::McpInitialize => ExternalEndpointContribution::protocol_control_identity(
                "mcp-protocol-control",
                ExternalEndpointIdentity::mcp("initialize"),
            ),
            Self::McpInitializedNotification => {
                ExternalEndpointContribution::protocol_control_identity(
                    "mcp-protocol-control",
                    ExternalEndpointIdentity::mcp("notifications/initialized"),
                )
            }
            Self::NativeWebSocketUpgrade => {
                ExternalEndpointContribution::protocol_control_identity(
                    "native-websocket-protocol-control",
                    ExternalEndpointIdentity::http_control_variant(
                        "GET",
                        "/api/agent/v1/runs/websocket",
                        "websocket-upgrade",
                    ),
                )
            }
            Self::ResponsesWebSocketUpgrade => {
                ExternalEndpointContribution::protocol_control_identity(
                    "responses-websocket-protocol-control",
                    ExternalEndpointIdentity::http_control_variant(
                        "GET",
                        "/v1/responses",
                        "websocket-upgrade",
                    ),
                )
            }
            Self::NativeSseKeepalive => ExternalEndpointContribution::protocol_control_identity(
                "native-sse-protocol-control",
                ExternalEndpointIdentity::http_control_variant(
                    "POST",
                    "/api/agent/v1/runs",
                    "sse-keepalive-terminal",
                ),
            ),
            Self::CompatibilitySseKeepalive => {
                ExternalEndpointContribution::protocol_control_identity(
                    "compatibility-sse-protocol-control",
                    ExternalEndpointIdentity::http_control_variant(
                        "POST",
                        "/v1/responses",
                        "sse-keepalive-terminal",
                    ),
                )
            }
            Self::AssistantWebSocketUpgrade => {
                ExternalEndpointContribution::protocol_control_identity(
                    "assistant-websocket-protocol-control",
                    ExternalEndpointIdentity::http_control_variant(
                        "GET",
                        "/api/console/assistant/runs/websocket",
                        "websocket-upgrade",
                    ),
                )
            }
        }
    }
}

pub(crate) fn is_approved_external_control_http(method: &str, route_template: &str) -> bool {
    let identity = ExternalEndpointIdentity::http(method, route_template);
    ApprovedExternalControl::ALL
        .into_iter()
        .map(ApprovedExternalControl::contribution)
        .any(|control| control.identity == identity)
}

#[derive(Default)]
pub(crate) struct ExternalEndpointCatalogCompiler {
    rows: BTreeMap<ExternalEndpointIdentity, ExternalEndpointRow>,
    required_frozen_bindings: BTreeMap<ExternalEndpointIdentity, ExternalEndpointIdentity>,
}

impl ExternalEndpointCatalogCompiler {
    pub(crate) fn contribute_openapi_document(
        &mut self,
        source: &str,
        document: &serde_json::Value,
    ) -> Result<(), ExternalEndpointCatalogError> {
        let paths = document
            .get("paths")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                ExternalEndpointCatalogError::InvalidOpenApi(
                    "document does not contain an object `paths` member".to_string(),
                )
            })?;
        const HTTP_METHODS: [&str; 8] = [
            "get", "post", "put", "patch", "delete", "options", "head", "trace",
        ];
        for (path, path_item) in paths {
            let path_item = path_item.as_object().ok_or_else(|| {
                ExternalEndpointCatalogError::InvalidOpenApi(format!(
                    "path item `{path}` is not an object"
                ))
            })?;
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(method) else {
                    continue;
                };
                let identity = match (method, path.as_str()) {
                    (
                        "get",
                        "/api/agent/v1/runs/websocket"
                        | "/v1/responses"
                        | "/api/console/assistant/runs/websocket",
                    ) => ExternalEndpointIdentity::http_control_variant(
                        method,
                        path,
                        "websocket-upgrade",
                    ),
                    _ => ExternalEndpointIdentity::http(method, path),
                };
                let required_frozen_binding = runtime_model_frozen_binding(method, path, operation);
                self.contribute(ExternalEndpointContribution {
                    identity: identity.clone(),
                    source: source.to_string(),
                    classification: ExternalEndpointClassification::Unclassified,
                    binding_id: None,
                })?;
                if let Some(required_frozen_binding) = required_frozen_binding {
                    self.required_frozen_bindings
                        .insert(identity, required_frozen_binding);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn contribute_mcp_protocol_surface(
        &mut self,
        binding_id: &str,
    ) -> Result<(), ExternalEndpointCatalogError> {
        for method in ["initialize", "notifications/initialized"] {
            self.contribute(ExternalEndpointContribution {
                identity: ExternalEndpointIdentity::mcp(method),
                source: "api-server.mcp-router".to_string(),
                classification: ExternalEndpointClassification::Unclassified,
                binding_id: None,
            })?;
        }
        for method in ["tools/list", "tools/call"] {
            self.contribute(ExternalEndpointContribution {
                identity: ExternalEndpointIdentity::mcp(method),
                source: "api-server.mcp-router".to_string(),
                classification: ExternalEndpointClassification::CanonicalBusinessInterface,
                binding_id: Some(binding_id.to_string()),
            })?;
        }
        Ok(())
    }

    pub(crate) fn contribute(
        &mut self,
        contribution: ExternalEndpointContribution,
    ) -> Result<(), ExternalEndpointCatalogError> {
        let ExternalEndpointContribution {
            identity,
            source,
            classification,
            binding_id,
        } = contribution;
        match self.rows.get_mut(&identity) {
            None => {
                self.rows.insert(
                    identity.clone(),
                    ExternalEndpointRow {
                        identity,
                        sources: BTreeSet::from([source]),
                        classification,
                        binding_id,
                    },
                );
            }
            Some(row) => {
                if !row.sources.insert(source.clone()) {
                    return Err(ExternalEndpointCatalogError::DuplicateContribution {
                        origin: source,
                        identity,
                    });
                }
                row.classification =
                    merge_classification(row.classification, classification, row.identity.clone())?;
                match (&row.binding_id, binding_id) {
                    (Some(first), Some(second)) if first != &second => {
                        return Err(ExternalEndpointCatalogError::ConflictingBinding {
                            identity,
                            first: first.clone(),
                            second,
                        });
                    }
                    (None, Some(binding_id)) => row.binding_id = Some(binding_id),
                    (Some(_), Some(_)) | (Some(_), None) | (None, None) => {}
                }
            }
        }
        Ok(())
    }

    pub(crate) fn absorb_registry(
        &mut self,
        source: &str,
        registry: &CompiledInterfaceRegistry,
    ) -> Result<(), ExternalEndpointCatalogError> {
        for binding in registry.bindings() {
            if let Some(contribution) = ExternalEndpointContribution::from_binding(source, binding)
            {
                let frozen_identity = contribution.identity.clone();
                let binding_id = contribution
                    .binding_id
                    .clone()
                    .expect("external protocol binding contribution has a binding id");
                self.contribute(contribution)?;
                let concrete_identities = self
                    .required_frozen_bindings
                    .iter()
                    .filter_map(|(identity, required)| {
                        (required == &frozen_identity).then_some(identity.clone())
                    })
                    .collect::<Vec<_>>();
                for identity in concrete_identities {
                    self.contribute(ExternalEndpointContribution {
                        identity,
                        source: format!("{source}.runtime-model-descriptor-projection"),
                        classification: ExternalEndpointClassification::CanonicalBusinessInterface,
                        binding_id: Some(binding_id.clone()),
                    })?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn contribute_approved_controls(
        &mut self,
        include_docs: bool,
    ) -> Result<(), ExternalEndpointCatalogError> {
        for control in ApprovedExternalControl::ALL {
            if !include_docs && matches!(control, ApprovedExternalControl::Docs) {
                continue;
            }
            self.contribute(control.contribution())?;
        }
        self.contribute_derived_cors_and_head_controls()
    }

    fn contribute_derived_cors_and_head_controls(
        &mut self,
    ) -> Result<(), ExternalEndpointCatalogError> {
        let routes = self
            .rows
            .keys()
            .filter_map(|identity| match identity {
                ExternalEndpointIdentity::Http {
                    method,
                    route_template,
                    variant: None,
                } => Some((method.clone(), route_template.clone())),
                ExternalEndpointIdentity::Http {
                    variant: Some(_), ..
                }
                | ExternalEndpointIdentity::Mcp { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let paths = routes
            .iter()
            .map(|(_, path)| path.clone())
            .collect::<BTreeSet<_>>();
        for route_template in paths {
            if route_template != "/api/ex/*slug" {
                self.contribute(ExternalEndpointContribution::protocol_control_identity(
                    "tower-http.cors",
                    ExternalEndpointIdentity::http_control_variant(
                        "OPTIONS",
                        &route_template,
                        "cors-preflight",
                    ),
                ))?;
            }
        }
        for (method, route_template) in routes {
            if method == "GET" {
                self.contribute(ExternalEndpointContribution::protocol_control_identity(
                    "axum.auto-head",
                    ExternalEndpointIdentity::http_control_variant(
                        "HEAD",
                        &route_template,
                        "get-mirror",
                    ),
                ))?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn reject_unapproved_control(
        &self,
        identity: ExternalEndpointIdentity,
    ) -> ExternalEndpointCatalogError {
        ExternalEndpointCatalogError::UnknownControl { identity }
    }

    #[cfg(test)]
    pub(crate) fn compile(self) -> ExternalEndpointCatalog {
        ExternalEndpointCatalog { rows: self.rows }
    }

    pub(crate) fn compile_complete(
        self,
        registry: &CompiledInterfaceRegistry,
    ) -> Result<ExternalEndpointCatalog, ExternalEndpointCatalogError> {
        let valid_bindings = registry
            .bindings()
            .map(|binding| binding.binding_id().as_str().to_string())
            .collect::<BTreeSet<_>>();
        let unclassified = self
            .rows
            .values()
            .filter(|row| row.classification == ExternalEndpointClassification::Unclassified)
            .map(|row| row.identity.clone())
            .collect::<Vec<_>>();
        if !unclassified.is_empty() {
            return Err(ExternalEndpointCatalogError::UnclassifiedRows {
                identities: unclassified,
            });
        }
        for row in self.rows.values() {
            if let Some(binding_id) = &row.binding_id {
                if !valid_bindings.contains(binding_id) {
                    return Err(ExternalEndpointCatalogError::UnknownBinding {
                        identity: row.identity.clone(),
                        binding_id: binding_id.clone(),
                    });
                }
            }
        }
        Ok(ExternalEndpointCatalog { rows: self.rows })
    }
}

fn runtime_model_frozen_binding(
    method: &str,
    path: &str,
    operation: &serde_json::Value,
) -> Option<ExternalEndpointIdentity> {
    let is_descriptor_operation = operation
        .get("x-data-model-templates")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|templates| !templates.is_empty());
    (is_descriptor_operation && path.starts_with("/api/runtime/models/{model_code}/")).then(|| {
        ExternalEndpointIdentity::http(method, "/api/runtime/models/:model_code/*operation_path")
    })
}

pub(crate) struct ExternalEndpointCatalog {
    rows: BTreeMap<ExternalEndpointIdentity, ExternalEndpointRow>,
}

impl ExternalEndpointCatalog {
    pub(crate) fn rows(&self) -> impl ExactSizeIterator<Item = &ExternalEndpointRow> {
        self.rows.values()
    }

    #[cfg(test)]
    pub(crate) fn row(&self, identity: &ExternalEndpointIdentity) -> Option<&ExternalEndpointRow> {
        self.rows.get(identity)
    }

    pub(crate) fn classification_count(
        &self,
        classification: ExternalEndpointClassification,
    ) -> usize {
        self.rows
            .values()
            .filter(|row| row.classification == classification)
            .count()
    }
}

fn normalize_route_template(route_template: &str) -> String {
    let route_template = if route_template.len() > 1 {
        route_template.trim_end_matches('/')
    } else {
        route_template
    };
    route_template
        .split('/')
        .map(|segment| {
            if segment.starts_with(':') || segment.starts_with('{') && segment.ends_with('}') {
                "{}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn merge_classification(
    current: ExternalEndpointClassification,
    incoming: ExternalEndpointClassification,
    identity: ExternalEndpointIdentity,
) -> Result<ExternalEndpointClassification, ExternalEndpointCatalogError> {
    match (current, incoming) {
        (left, right) if left == right => Ok(left),
        (ExternalEndpointClassification::Unclassified, classified)
        | (classified, ExternalEndpointClassification::Unclassified) => Ok(classified),
        _ => Err(ExternalEndpointCatalogError::ConflictingClassification { identity }),
    }
}
