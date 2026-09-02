use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
};

use extension_contracts::{AfterCommitFact, LifecycleContract};
use serde::de::DeserializeOwned;
use thiserror::Error;

use super::EffectiveLifecycleSubscriberPlan;

pub type LifecycleHandlerFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), LifecycleHandlerError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct LifecycleHandlerError {
    message: String,
}

impl LifecycleHandlerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait TypedLifecycleSubscriberHandler<T>: Send + Sync
where
    T: LifecycleContract,
{
    fn handle<'a>(&'a self, fact: &'a AfterCommitFact<T>) -> LifecycleHandlerFuture<'a>;
}

trait ErasedLifecycleSubscriberHandler: Send + Sync {
    fn handle<'a>(&'a self, canonical_fact: &'a [u8]) -> LifecycleHandlerFuture<'a>;
}

struct TypedHandlerAdapter<T, H> {
    handler: Arc<H>,
    marker: PhantomData<fn() -> T>,
}

impl<T, H> ErasedLifecycleSubscriberHandler for TypedHandlerAdapter<T, H>
where
    T: LifecycleContract + DeserializeOwned,
    H: TypedLifecycleSubscriberHandler<T> + 'static,
{
    fn handle<'a>(&'a self, canonical_fact: &'a [u8]) -> LifecycleHandlerFuture<'a> {
        Box::pin(async move {
            let fact =
                serde_json::from_slice::<AfterCommitFact<T>>(canonical_fact).map_err(|error| {
                    LifecycleHandlerError::new(format!("invalid typed lifecycle fact: {error}"))
                })?;
            self.handler.handle(&fact).await
        })
    }
}

pub struct LifecycleHandlerBinding {
    handler_id: String,
    handler_version: String,
    fact_contract_id: String,
    fact_contract_version: String,
    handler: Arc<dyn ErasedLifecycleSubscriberHandler>,
}

impl LifecycleHandlerBinding {
    pub fn typed<T, H>(
        handler_id: impl Into<String>,
        handler_version: impl Into<String>,
        handler: Arc<H>,
    ) -> Self
    where
        T: LifecycleContract + DeserializeOwned,
        H: TypedLifecycleSubscriberHandler<T> + 'static,
    {
        Self {
            handler_id: handler_id.into(),
            handler_version: handler_version.into(),
            fact_contract_id: T::CONTRACT_ID.to_string(),
            fact_contract_version: T::CONTRACT_VERSION.to_string(),
            handler: Arc::new(TypedHandlerAdapter::<T, H> {
                handler,
                marker: PhantomData,
            }),
        }
    }
}

struct CompiledLifecycleHandler {
    fact_contract_id: String,
    fact_contract_version: String,
    handler: Arc<dyn ErasedLifecycleSubscriberHandler>,
}

pub struct EffectiveLifecycleHandlerRegistry {
    graph_fingerprint: String,
    handlers: BTreeMap<(String, String), CompiledLifecycleHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LifecycleHandlerRegistryError {
    #[error("duplicate lifecycle handler binding {handler_id}@{handler_version}")]
    DuplicateBinding {
        handler_id: String,
        handler_version: String,
    },
    #[error("lifecycle handler {handler_id}@{handler_version} is not bound")]
    MissingBinding {
        handler_id: String,
        handler_version: String,
    },
    #[error("lifecycle handler {handler_id}@{handler_version} is not active in the frozen plan")]
    InactiveBinding {
        handler_id: String,
        handler_version: String,
    },
    #[error("lifecycle handler {handler_id}@{handler_version} expects {actual_id}@{actual_version}, plan requires {expected_id}@{expected_version}")]
    ContractMismatch {
        handler_id: String,
        handler_version: String,
        expected_id: String,
        expected_version: String,
        actual_id: String,
        actual_version: String,
    },
    #[error("frozen lifecycle graph fingerprint is not available")]
    FrozenGraphUnavailable,
    #[error("frozen lifecycle handler {handler_id}@{handler_version} is not available")]
    HandlerUnavailable {
        handler_id: String,
        handler_version: String,
    },
}

#[expect(
    clippy::result_large_err,
    reason = "the stable compiler API preserves structured lifecycle mismatch diagnostics"
)]
pub fn compile_lifecycle_handler_registry(
    plan: &EffectiveLifecycleSubscriberPlan,
    bindings: Vec<LifecycleHandlerBinding>,
) -> Result<EffectiveLifecycleHandlerRegistry, LifecycleHandlerRegistryError> {
    let mut indexed = BTreeMap::new();
    for binding in bindings {
        let key = (binding.handler_id.clone(), binding.handler_version.clone());
        if indexed.insert(key.clone(), binding).is_some() {
            return Err(LifecycleHandlerRegistryError::DuplicateBinding {
                handler_id: key.0,
                handler_version: key.1,
            });
        }
    }

    let mut active = BTreeSet::new();
    let mut handlers = BTreeMap::new();
    for subscriber in plan.subscribers() {
        let key = (
            subscriber.handler_id.clone(),
            subscriber.handler_version.clone(),
        );
        active.insert(key.clone());
        let binding =
            indexed
                .get(&key)
                .ok_or_else(|| LifecycleHandlerRegistryError::MissingBinding {
                    handler_id: key.0.clone(),
                    handler_version: key.1.clone(),
                })?;
        if binding.fact_contract_id != subscriber.fact_contract_id
            || binding.fact_contract_version != subscriber.fact_contract_version
        {
            return Err(LifecycleHandlerRegistryError::ContractMismatch {
                handler_id: key.0,
                handler_version: key.1,
                expected_id: subscriber.fact_contract_id.clone(),
                expected_version: subscriber.fact_contract_version.clone(),
                actual_id: binding.fact_contract_id.clone(),
                actual_version: binding.fact_contract_version.clone(),
            });
        }
        handlers
            .entry(key)
            .or_insert_with(|| CompiledLifecycleHandler {
                fact_contract_id: binding.fact_contract_id.clone(),
                fact_contract_version: binding.fact_contract_version.clone(),
                handler: Arc::clone(&binding.handler),
            });
    }

    if let Some((handler_id, handler_version)) =
        indexed.keys().find(|key| !active.contains(*key)).cloned()
    {
        return Err(LifecycleHandlerRegistryError::InactiveBinding {
            handler_id,
            handler_version,
        });
    }

    Ok(EffectiveLifecycleHandlerRegistry {
        graph_fingerprint: plan.graph_fingerprint().to_string(),
        handlers,
    })
}

impl EffectiveLifecycleHandlerRegistry {
    pub async fn deliver(
        &self,
        graph_fingerprint: &str,
        handler_id: &str,
        handler_version: &str,
        fact_contract_id: &str,
        fact_contract_version: &str,
        canonical_fact: &[u8],
    ) -> Result<(), LifecycleHandlerError> {
        if graph_fingerprint != self.graph_fingerprint {
            return Err(LifecycleHandlerError::new(
                LifecycleHandlerRegistryError::FrozenGraphUnavailable.to_string(),
            ));
        }
        let handler = self
            .handlers
            .get(&(handler_id.to_string(), handler_version.to_string()))
            .ok_or_else(|| {
                LifecycleHandlerError::new(
                    LifecycleHandlerRegistryError::HandlerUnavailable {
                        handler_id: handler_id.to_string(),
                        handler_version: handler_version.to_string(),
                    }
                    .to_string(),
                )
            })?;
        if handler.fact_contract_id != fact_contract_id
            || handler.fact_contract_version != fact_contract_version
        {
            return Err(LifecycleHandlerError::new(
                "lifecycle fact contract does not match frozen handler",
            ));
        }
        handler.handler.handle(canonical_fact).await
    }
}
