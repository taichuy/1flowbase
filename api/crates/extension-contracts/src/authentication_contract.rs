//! Managed authentication verifies external identities. Host owns credentials, identity
//! binding, registration transactions and sessions; worker config is passed separately.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ContributionId, PluginFormFieldSchema};

pub const AUTHENTICATION_POINT: &str = "1flowbase.authentication.provider";
pub const AUTHENTICATION_PERMISSION: &str = "authentication.verify";
pub const AUTHENTICATION_CONTRACT_V1: &str = "1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticationProviderDescriptor {
    pub auth_type: String,
    pub display_name: String,
    pub config_schema: Vec<PluginFormFieldSchema>,
    pub default_public_ui_block: String,
    pub public_variable_keys: Vec<String>,
    pub contribution_id: ContributionId,
}

impl AuthenticationProviderDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        let segments = self.auth_type.split('/').collect::<Vec<_>>();
        if segments.len() > 2
            || segments.iter().any(|segment| {
                segment.is_empty()
                    || matches!(*segment, "." | "..")
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'-' | b'_')
                    })
            })
            || self.auth_type == "password-local"
            || self.display_name.trim().is_empty()
            || self.default_public_ui_block.trim().is_empty()
        {
            return Err("authentication provider requires an external auth_type, display_name and public UI block".into());
        }
        let mut fields = std::collections::BTreeSet::new();
        for field in &self.config_schema {
            if field.key.trim().is_empty()
                || field.label.trim().is_empty()
                || !fields.insert(&field.key)
            {
                return Err("authentication config requires unique labeled fields".into());
            }
        }
        let mut public_keys = std::collections::BTreeSet::new();
        for key in &self.public_variable_keys {
            let normalized = key.to_ascii_lowercase();
            let field = self.config_schema.iter().find(|field| field.key == *key);
            if key.trim().is_empty()
                || normalized.contains("secret")
                || normalized.contains("password")
                || !public_keys.insert(key)
                || field.is_none_or(|field| {
                    field.field_type.eq_ignore_ascii_case("secret")
                        || field.field_type.eq_ignore_ascii_case("password")
                        || field
                            .control
                            .as_deref()
                            .is_some_and(|control| control.eq_ignore_ascii_case("password"))
                })
            {
                return Err("authentication public variable keys must be unique declared non-secret fields and cannot expose secrets or passwords".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthenticationRequest {
    Begin {
        callback_url: String,
        state: String,
        code_challenge: String,
    },
    Complete {
        callback_url: String,
        code: String,
        code_verifier: String,
    },
    RegistrationEligibility {
        profile: Value,
        now_unix: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthenticationResponse {
    Authorization {
        authorization_url: String,
    },
    VerifiedIdentity {
        subject_type: String,
        subject_value: String,
        issuer: String,
        realm: Option<String>,
        profile: Value,
    },
    RegistrationEligibility {
        allowed: bool,
        reason_code: Option<String>,
    },
}
