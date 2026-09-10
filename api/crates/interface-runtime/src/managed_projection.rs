//! Typed interface owners project safe values; the composition root validates their wire schema.
//! This port carries no Host/worker identity and does not grant access to the original value.
use serde_json::Value;

use crate::{ContractIdentity, InterfaceContract};

#[derive(Debug, Clone)]
pub struct ManagedInterfaceProjection {
    contract: ContractIdentity,
    schema: Value,
    value: Value,
}

impl ManagedInterfaceProjection {
    pub fn from_contract<T: InterfaceContract>(value: &T) -> Option<Self> {
        Some(Self {
            contract: ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).ok()?,
            schema: T::managed_projection_schema()?,
            value: value.project_for_managed_hook()?,
        })
    }

    pub fn contract(&self) -> &ContractIdentity {
        &self.contract
    }

    pub fn schema(&self) -> &Value {
        &self.schema
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}
