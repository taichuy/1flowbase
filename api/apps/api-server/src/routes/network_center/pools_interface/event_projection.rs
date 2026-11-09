use super::*;
#[cfg(test)]
use crate::openapi_interface::contract::ApiOutput;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext};
use serde_json::{json, Value};

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct StaticProxyEvent<'a> {
    display_name: &'a str,
    host: &'a str,
    port: u16,
    username: &'a str,
    enabled: bool,
    sequence: i32,
}
// Public fields match the credential owner's explicit built-in proxy contract:
// control_plane::network_egress::proxy_edit::proxy_field_is_secret.
// Extension config has no public-field declaration and remains a secret carrier.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PublicProxyConfigEvent<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a Value>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ProxyEvent<'a> {
    provider_code: &'a str,
    display_name: &'a str,
    description: &'a str,
    config: PublicProxyConfigEvent<'a>,
}
fn proxy(body: &CreateNetworkEgressProxyBody) -> ProxyEvent<'_> {
    let public = domain::StaticProxyProtocol::from_provider_code(&body.provider_code).is_some();
    ProxyEvent {
        provider_code: &body.provider_code,
        display_name: &body.display_name,
        description: &body.description,
        config: PublicProxyConfigEvent {
            host: public.then(|| body.config.get("host")).flatten(),
            port: public.then(|| body.config.get("port")).flatten(),
            username: public.then(|| body.config.get("username")).flatten(),
        },
    }
}
impl ApiInput for NetworkPoolsInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "network_egress_proxies.create" => Some(
                json!({"type":"object","properties":{"body":event::schema::<ProxyEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            "network_egress_proxies.update" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"provider_id":{"type":"string"}},"required":["provider_id"]},"body":event::schema::<ProxyEvent<'static>>()},"required":["path","body"],"additionalProperties":false}),
            ),
            "network_egress_pool_members.create_static_http" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"pool_id":{"type":"string"}},"required":["pool_id"]},"body":event::schema::<StaticProxyEvent<'static>>()},"required":["path","body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::HttpNodeProxyOptions | Self::List => json!({}),
            Self::CreateProxy(body) => json!({"body":event::to_value(&proxy(body))?}),
            Self::GetProxy { provider_id } => json!({"path":{"provider_id":provider_id}}),
            Self::UpdateProxy { provider_id, body } => {
                json!({"path":{"provider_id":provider_id},"body":event::to_value(&proxy(body))?})
            }
            Self::TestMember { pool_id, member_id } => {
                json!({"path":{"pool_id":pool_id,"member_id":member_id}})
            }
            Self::Create(body) => json!({"body":event::to_value(body)?}),
            Self::Update { pool_id, body } => {
                json!({"path":{"pool_id":pool_id},"body":event::to_value(body)?})
            }
            Self::Delete { pool_id } => json!({"path":{"pool_id":pool_id}}),
            Self::CreateMember { pool_id, body } => {
                json!({"path":{"pool_id":pool_id},"body":event::to_value(body)?})
            }
            Self::AddStatic { pool_id, body } => {
                json!({"path":{"pool_id":pool_id},"body":event::to_value(&StaticProxyEvent {
                display_name:&body.display_name, host:&body.host, port:body.port, username:&body.username,
                enabled:body.enabled, sequence:body.sequence,
            })?})
            }
            Self::AddProvider { pool_id, body } => {
                json!({"path":{"pool_id":pool_id},"body":event::to_value(body)?})
            }
            Self::UpdateMember {
                pool_id,
                member_id,
                body,
            } => {
                json!({"path":{"pool_id":pool_id,"member_id":member_id},"body":event::to_value(body)?})
            }
            Self::DeleteMember { pool_id, member_id } => {
                json!({"path":{"pool_id":pool_id,"member_id":member_id}})
            }
            Self::DeleteMembers { pool_id, body } => {
                json!({"path":{"pool_id":pool_id},"body":event::to_value(body)?})
            }
        })
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
