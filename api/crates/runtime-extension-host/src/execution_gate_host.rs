//! Process-owned group queues; active calls own permits and cancellation, never workers.
use extension_contracts::{
    PluginExecutionGateAuthority, PluginExecutionGateBinding, PluginExecutionGateError,
    PluginExecutionGateRequest, PluginExecutionGateResponse,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};

type GroupKey = (String, String, String);
#[derive(Default)]
struct RegistryInner {
    groups: Mutex<HashMap<GroupKey, Weak<Group>>>,
}
#[derive(Clone, Default)]
pub(crate) struct ExecutionGateRegistry(Arc<RegistryInner>);
struct Group {
    key: GroupKey,
    registry: Weak<RegistryInner>,
    semaphore: Arc<Semaphore>,
}
impl Drop for Group {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            let mut groups = registry.groups.lock().unwrap();
            // A newer group may already have replaced our un-upgradeable weak entry.
            if groups
                .get(&self.key)
                .is_some_and(|entry| std::ptr::eq(entry.as_ptr(), self))
            {
                groups.remove(&self.key);
            }
        }
    }
}
impl ExecutionGateRegistry {
    fn group(&self, binding: &PluginExecutionGateBinding, key: &str) -> Arc<Group> {
        let key = (
            binding.subject.installation_id().as_str().into(),
            binding.workspace_id.clone(),
            key.into(),
        );
        let mut groups = self.0.groups.lock().unwrap();
        if let Some(group) = groups.get(&key).and_then(Weak::upgrade) {
            return group;
        }
        let group = Arc::new(Group {
            key: key.clone(),
            registry: Arc::downgrade(&self.0),
            semaphore: Arc::new(Semaphore::new(1)),
        });
        groups.insert(key, Arc::downgrade(&group));
        group
    }
}
struct Held {
    key: String,
    _permit: OwnedSemaphorePermit,
    _group: Arc<Group>,
}
#[derive(Default)]
struct CallState {
    closed: bool,
    held: HashMap<String, Held>,
}
struct CallInner {
    state: Mutex<CallState>,
    closed: watch::Sender<bool>,
}
#[derive(Clone)]
pub(crate) struct ExecutionGateCall {
    registry: ExecutionGateRegistry,
    binding: PluginExecutionGateBinding,
    authority: Arc<dyn PluginExecutionGateAuthority>,
    inner: Arc<CallInner>,
}
pub(crate) struct ExecutionGateCallGuard(ExecutionGateCall);
impl Drop for ExecutionGateCallGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}
fn error(code: &str) -> PluginExecutionGateError {
    PluginExecutionGateError::new(code)
}
impl ExecutionGateCall {
    pub(crate) fn new(
        registry: ExecutionGateRegistry,
        binding: PluginExecutionGateBinding,
        authority: Arc<dyn PluginExecutionGateAuthority>,
    ) -> Self {
        let (closed, _) = watch::channel(false);
        Self {
            registry,
            binding,
            authority,
            inner: Arc::new(CallInner {
                state: Mutex::new(CallState::default()),
                closed,
            }),
        }
    }
    pub(crate) fn cleanup_guard(&self) -> ExecutionGateCallGuard {
        ExecutionGateCallGuard(self.clone())
    }
    /// Explicit active-call cleanup also invalidates any context clones held by the caller.
    pub(crate) fn close(&self) {
        let mut state = self.inner.state.lock().unwrap();
        state.closed = true;
        state.held.clear();
        self.inner.closed.send_replace(true);
    }
    pub(crate) async fn execute(
        &self,
        value: serde_json::Value,
    ) -> Result<PluginExecutionGateResponse, PluginExecutionGateError> {
        let request: PluginExecutionGateRequest =
            serde_json::from_value(value).map_err(|_| error("execution_gate_request_invalid"))?;
        request.validate()?;
        let mut closed = self.inner.closed.subscribe();
        if *closed.borrow() {
            return Err(error("execution_gate_call_closed"));
        }
        let remaining = self.binding.deadline_unix_ms.saturating_sub(
            i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
                .unwrap_or(i64::MAX),
        );
        if remaining <= 0 {
            return Err(error("execution_gate_deadline_exceeded"));
        }
        let operation = async {
            self.authority.authorize(&self.binding).await?;
            match request {
                PluginExecutionGateRequest::Acquire { key } => {
                    // A call cannot wait on a group it already holds.
                    if self
                        .inner
                        .state
                        .lock()
                        .unwrap()
                        .held
                        .values()
                        .any(|held| held.key == key)
                    {
                        return Err(error("execution_gate_already_acquired"));
                    }
                    let group = self.registry.group(&self.binding, &key);
                    let permit = group
                        .semaphore
                        .clone()
                        .acquire_owned()
                        .await
                        .map_err(|_| error("execution_gate_call_closed"))?;
                    // Authority can change while queued; never admit using an old grant.
                    self.authority.authorize(&self.binding).await?;
                    let token = uuid::Uuid::now_v7().to_string();
                    let mut state = self.inner.state.lock().unwrap();
                    if state.closed {
                        return Err(error("execution_gate_call_closed"));
                    }
                    state.held.insert(
                        token.clone(),
                        Held {
                            key,
                            _permit: permit,
                            _group: group,
                        },
                    );
                    Ok(PluginExecutionGateResponse::Acquire { token })
                }
                PluginExecutionGateRequest::Release { key, token } => {
                    let mut state = self.inner.state.lock().unwrap();
                    if state.closed {
                        return Err(error("execution_gate_call_closed"));
                    }
                    if !state.held.get(&token).is_some_and(|held| held.key == key) {
                        return Err(error("execution_gate_token_invalid"));
                    }
                    state.held.remove(&token);
                    Ok(PluginExecutionGateResponse::Release)
                }
            }
        };
        let operation = async {
            tokio::select! {
                biased;
                _ = closed.changed() => Err(error("execution_gate_call_closed")),
                result = operation => result,
            }
        };
        if self.binding.deadline_unix_ms == i64::MAX {
            operation.await
        } else {
            tokio::time::timeout(
                std::time::Duration::from_millis(remaining as u64),
                operation,
            )
            .await
            .map_err(|_| error("execution_gate_deadline_exceeded"))?
        }
    }
}

#[cfg(test)]
#[path = "_tests/host_services/execution_gate_callbacks.rs"]
mod tests;
