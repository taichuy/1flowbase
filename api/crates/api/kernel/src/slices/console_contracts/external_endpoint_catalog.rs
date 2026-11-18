use interface_runtime::{ProtocolBinding, ProtocolProjection};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "test-support", derive(serde::Serialize))]
pub enum ExternalEndpointClassification {
    CanonicalBusinessInterface,
    ProtocolControl,
    OperationalControl,
    Unclassified,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "test-support", derive(serde::Serialize))]
pub enum ExternalEndpointIdentity {
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
    pub fn http(method: &str, route_template: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: None,
        }
    }

    pub fn http_variant(method: &str, route_template: &str, variant: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: Some(variant.to_string()),
        }
    }

    pub fn mcp(protocol_method: &str) -> Self {
        Self::Mcp {
            protocol_method: protocol_method.to_string(),
            variant: None,
        }
    }

    pub fn http_control_variant(method: &str, route_template: &str, variant: &str) -> Self {
        Self::Http {
            method: method.to_ascii_uppercase(),
            route_template: normalize_route_template(route_template),
            variant: Some(variant.to_string()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "test-support", derive(serde::Serialize))]
pub struct ExternalEndpointContribution {
    pub identity: ExternalEndpointIdentity,
    pub source: String,
    pub classification: ExternalEndpointClassification,
    pub binding_id: Option<String>,
    pub carrier_projection: Option<ExternalEndpointIdentity>,
}
impl ExternalEndpointContribution {
    pub fn unclassified_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            carrier_projection: None,
            classification: ExternalEndpointClassification::Unclassified,
            binding_id: None,
        }
    }

    pub(crate) fn mcp_http_carrier(
        source: &str,
        method: &str,
        path: &str,
        binding_id: &str,
    ) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, path),
            source: source.to_string(),
            classification: ExternalEndpointClassification::CanonicalBusinessInterface,
            binding_id: Some(binding_id.to_string()),
            carrier_projection: Some(ExternalEndpointIdentity::mcp("mcp.json-rpc")),
        }
    }

    pub fn protocol_control_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            carrier_projection: None,
            classification: ExternalEndpointClassification::ProtocolControl,
            binding_id: None,
        }
    }

    fn operational_control_http(source: &str, method: &str, route_template: &str) -> Self {
        Self {
            identity: ExternalEndpointIdentity::http(method, route_template),
            source: source.to_string(),
            carrier_projection: None,
            classification: ExternalEndpointClassification::OperationalControl,
            binding_id: None,
        }
    }

    pub fn protocol_control_identity(source: &str, identity: ExternalEndpointIdentity) -> Self {
        Self {
            identity,
            source: source.to_string(),
            carrier_projection: None,
            classification: ExternalEndpointClassification::ProtocolControl,
            binding_id: None,
        }
    }

    pub fn from_binding(source: &str, binding: &ProtocolBinding) -> Option<Self> {
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
            carrier_projection: None,
            classification: ExternalEndpointClassification::CanonicalBusinessInterface,
            binding_id: Some(binding.binding_id().as_str().to_string()),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "test-support", derive(serde::Serialize))]
pub struct ExternalEndpointRow {
    pub identity: ExternalEndpointIdentity,
    pub sources: BTreeSet<String>,
    pub classification: ExternalEndpointClassification,
    pub binding_id: Option<String>,
}
impl ExternalEndpointRow {
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn classification(&self) -> ExternalEndpointClassification {
        self.classification
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn sources(&self) -> &BTreeSet<String> {
        &self.sources
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn binding_id(&self) -> Option<&str> {
        self.binding_id.as_deref()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovedExternalControl {
    OAuthOriginMetadata,
    OAuthOriginOidcMetadata,
    OAuthOriginApiMetadata,
    OAuthOriginApiOAuthMetadata,
    OAuthOriginResource,
    OAuthOriginRegister,
    OAuthOriginAuthorize,
    OAuthOriginToken,
    OAuthOriginRevoke,
    OAuthMetadata,
    OAuthOidcMetadata,
    OAuthApiMetadata,
    OAuthApiOAuthMetadata,
    OAuthApiResource,
    OAuthResource,
    OAuthConfig,
    OAuthRegister,
    OAuthAuthorize,
    OAuthAuthorization,
    OAuthVerify,
    OAuthDecision,
    OAuthToken,
    OAuthRevoke,

    Health,
    ConsoleHealth,
    Docs,
    DocsAssets,
    OpenApi,
    McpInitialize,
    McpInitializedNotification,
    NativeWebSocketUpgrade,
    ResponsesWebSocketUpgrade,
    NativeSseKeepalive,
    CompatibilitySseKeepalive,
    AssistantWebSocketUpgrade,
    NotificationEvents,
}
impl ApprovedExternalControl {
    pub const ALL: [Self; 36] = [
        Self::OAuthOriginMetadata,
        Self::OAuthOriginOidcMetadata,
        Self::OAuthOriginApiMetadata,
        Self::OAuthOriginApiOAuthMetadata,
        Self::OAuthOriginResource,
        Self::OAuthOriginRegister,
        Self::OAuthOriginAuthorize,
        Self::OAuthOriginToken,
        Self::OAuthOriginRevoke,
        Self::OAuthMetadata,
        Self::OAuthOidcMetadata,
        Self::OAuthApiMetadata,
        Self::OAuthApiOAuthMetadata,
        Self::OAuthApiResource,
        Self::OAuthResource,
        Self::OAuthConfig,
        Self::OAuthRegister,
        Self::OAuthAuthorize,
        Self::OAuthAuthorization,
        Self::OAuthVerify,
        Self::OAuthDecision,
        Self::OAuthToken,
        Self::OAuthRevoke,
        Self::Health,
        Self::ConsoleHealth,
        Self::Docs,
        Self::DocsAssets,
        Self::OpenApi,
        Self::McpInitialize,
        Self::McpInitializedNotification,
        Self::NativeWebSocketUpgrade,
        Self::ResponsesWebSocketUpgrade,
        Self::NativeSseKeepalive,
        Self::CompatibilitySseKeepalive,
        Self::AssistantWebSocketUpgrade,
        Self::NotificationEvents,
    ];

    pub fn contribution(self) -> ExternalEndpointContribution {
        match self {
            Self::OAuthOriginMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/.well-known/oauth-authorization-server/api/public/mcp-oauth/origins/:origin",
            ),
            Self::OAuthOriginOidcMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/.well-known/openid-configuration/api/public/mcp-oauth/origins/:origin",
            ),
            Self::OAuthOriginApiOAuthMetadata => {
                ExternalEndpointContribution::protocol_control_http(
                    "mcp-oauth-protocol-control",
                    "GET",
                    "/api/public/mcp-oauth/origins/:origin/.well-known/oauth-authorization-server",
                )
            }
            Self::OAuthOriginApiMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/origins/:origin/.well-known/openid-configuration",
            ),
            Self::OAuthOriginResource => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/origins/:origin/protected-resource/:instance_id",
            ),
            Self::OAuthOriginRegister => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/origins/:origin/register",
            ),
            Self::OAuthOriginAuthorize => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/origins/:origin/authorize",
            ),
            Self::OAuthOriginRevoke => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/origins/:origin/revoke",
            ),
            Self::OAuthOriginToken => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/origins/:origin/token",
            ),
            Self::OAuthMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/.well-known/oauth-authorization-server/api/public/mcp-oauth",
            ),
            Self::OAuthOidcMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/.well-known/openid-configuration/api/public/mcp-oauth",
            ),
            Self::OAuthApiOAuthMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/.well-known/oauth-authorization-server",
            ),
            Self::OAuthApiMetadata => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/.well-known/openid-configuration",
            ),
            Self::OAuthApiResource => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/protected-resource/:instance_id",
            ),
            Self::OAuthResource => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/.well-known/oauth-protected-resource/api/mcp/:instance_id",
            ),
            Self::OAuthConfig => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/config",
            ),
            Self::OAuthRegister => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/register",
            ),
            Self::OAuthAuthorize => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/authorize",
            ),
            Self::OAuthAuthorization => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "GET",
                "/api/public/mcp-oauth/authorization",
            ),
            Self::OAuthVerify => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/verify",
            ),
            Self::OAuthDecision => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/decision",
            ),
            Self::OAuthRevoke => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/revoke",
            ),
            Self::OAuthToken => ExternalEndpointContribution::protocol_control_http(
                "mcp-oauth-protocol-control",
                "POST",
                "/api/public/mcp-oauth/token",
            ),
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
            Self::DocsAssets => ExternalEndpointContribution::protocol_control_http(
                "swagger-protocol-control",
                "GET",
                "/docs/*rest",
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
            Self::NotificationEvents => ExternalEndpointContribution::protocol_control_http(
                "notification-refresh-signal",
                "GET",
                "/api/console/notifications/events",
            ),
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
pub fn is_approved_external_control_http(method: &str, route_template: &str) -> bool {
    let identity = ExternalEndpointIdentity::http(method, route_template);
    ApprovedExternalControl::ALL
        .into_iter()
        .map(ApprovedExternalControl::contribution)
        .any(|control| control.identity == identity)
}
/// Project the existing control owner for a documented HTTP carrier. Callers must
/// first exclude a canonical business binding: SSE/frame controls can share its URL.
pub fn approved_http_control_classification(
    method: &str,
    route_template: &str,
) -> Option<ExternalEndpointClassification> {
    let ExternalEndpointIdentity::Http {
        method,
        route_template,
        ..
    } = ExternalEndpointIdentity::http(method, route_template)
    else {
        unreachable!("HTTP identity constructor always returns HTTP")
    };
    ApprovedExternalControl::ALL
        .into_iter()
        .map(ApprovedExternalControl::contribution)
        .find_map(|control| match control.identity {
            ExternalEndpointIdentity::Http {
                method: candidate_method,
                route_template: candidate_route,
                ..
            } if method == candidate_method && route_template == candidate_route => {
                Some(control.classification)
            }
            _ => None,
        })
}
pub struct ExternalEndpointCatalog {
    pub rows: BTreeMap<ExternalEndpointIdentity, ExternalEndpointRow>,
}
impl ExternalEndpointCatalog {
    /// The published generation owns both static controls and validated managed stream upgrades.
    /// A canonical business variant at the same URL remains independently projected by its binding.
    pub fn http_control_classification(
        &self,
        method: &str,
        route_template: &str,
    ) -> Option<ExternalEndpointClassification> {
        let ExternalEndpointIdentity::Http {
            method,
            route_template,
            ..
        } = ExternalEndpointIdentity::http(method, route_template)
        else {
            unreachable!("HTTP constructor produces HTTP identity");
        };
        self.rows.values().find_map(|row| match &row.identity {
            ExternalEndpointIdentity::Http {
                method: candidate_method,
                route_template: candidate_path,
                ..
            } if candidate_method == &method
                && candidate_path == &route_template
                && matches!(
                    row.classification,
                    ExternalEndpointClassification::OperationalControl
                        | ExternalEndpointClassification::ProtocolControl
                ) =>
            {
                Some(row.classification)
            }
            _ => None,
        })
    }
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &ExternalEndpointRow> {
        self.rows.values()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn row(&self, identity: &ExternalEndpointIdentity) -> Option<&ExternalEndpointRow> {
        self.rows.get(identity)
    }

    pub fn classification_count(&self, classification: ExternalEndpointClassification) -> usize {
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
