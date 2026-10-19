use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostWorkerDefinition {
    pub extension_id: String,
    pub worker_id: String,
    pub queue: String,
    pub handler: String,
}

#[derive(Debug, Default)]
pub struct HostWorkerRegistry {
    frozen: bool,
    workers_by_id: HashMap<String, HostWorkerDefinition>,
}

impl HostWorkerRegistry {
    pub fn register(
        &mut self,
        worker: HostWorkerDefinition,
    ) -> Result<(), HostWorkerRegistryError> {
        if self.frozen {
            return Err(HostWorkerRegistryError::new(
                "host worker registry is frozen",
            ));
        }
        validate_non_empty(&worker.extension_id, "extension_id")?;
        validate_non_empty(&worker.worker_id, "worker_id")?;
        validate_non_empty(&worker.queue, "queue")?;
        validate_non_empty(&worker.handler, "handler")?;
        if self.workers_by_id.contains_key(&worker.worker_id) {
            return Err(HostWorkerRegistryError::new(format!(
                "duplicate worker id {}",
                worker.worker_id
            )));
        }

        self.workers_by_id.insert(worker.worker_id.clone(), worker);
        Ok(())
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn workers(&self) -> Vec<&HostWorkerDefinition> {
        self.workers_by_id.values().collect()
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostWorkerRegistryError {
    message: String,
}

impl HostWorkerRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for HostWorkerRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for HostWorkerRegistryError {}

fn validate_non_empty(value: &str, field: &str) -> Result<(), HostWorkerRegistryError> {
    if value.trim().is_empty() {
        return Err(HostWorkerRegistryError::new(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}
