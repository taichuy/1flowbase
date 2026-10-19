//! Raw HTTP through Host-owned Network Egress, without worker-supplied identity.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpEgressRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body_base64: Option<String>,
}
impl HttpEgressRequest {
    pub fn validate(&self) -> Result<(), crate::HostServiceError> {
        let url = url::Url::parse(&self.url)
            .map_err(|_| crate::HostServiceError::new("http_egress_url_invalid"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(crate::HostServiceError::new("http_egress_url_invalid"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpEgressResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body_base64: String,
}
