use anyhow::{bail, Result};
use uuid::Uuid;

/// A selector, never a bearer credential. Only the host executing the matching run can use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCredentialSubject {
    Creator,
    Executor,
}

impl WorkflowCredentialSubject {
    pub fn reference(self, flow_run_id: Uuid) -> String {
        let subject = match self {
            Self::Creator => "creator",
            Self::Executor => "executor",
        };
        format!("wfcred:{flow_run_id}:{subject}")
    }

    pub fn from_reference(flow_run_id: Uuid, reference: &str) -> Result<Self> {
        for subject in [Self::Creator, Self::Executor] {
            if reference == subject.reference(flow_run_id) {
                return Ok(subject);
            }
        }
        bail!("workflow_credential_reference_invalid")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCredentialReferences {
    pub creator_credential: String,
    pub executor_credential: String,
}

impl WorkflowCredentialReferences {
    pub fn for_run(flow_run_id: Uuid) -> Self {
        Self {
            creator_credential: WorkflowCredentialSubject::Creator.reference(flow_run_id),
            executor_credential: WorkflowCredentialSubject::Executor.reference(flow_run_id),
        }
    }

    pub fn inject_into_pool(&self, pool: &mut serde_json::Map<String, serde_json::Value>) {
        let sys = pool.entry("sys").or_insert_with(|| serde_json::json!({}));
        if !sys.is_object() {
            *sys = serde_json::json!({});
        }
        sys["creator_credential"] = serde_json::json!(self.creator_credential);
        sys["executor_credential"] = serde_json::json!(self.executor_credential);
    }

    /// Refresh only host selectors. Each admitted container round owns the pool
    /// it resumes with; changing the root alone leaves predecessor references.
    pub fn inject_into_snapshot(&self, snapshot: &mut crate::execution_state::CheckpointSnapshot) {
        self.inject_into_pool(&mut snapshot.variable_pool);
        if let Some(container) = snapshot.container_state.as_mut() {
            self.inject_into_pool(&mut container.root_variable_pool);
            for frame in container.frames.values_mut() {
                for round in frame.rounds.values_mut() {
                    self.inject_into_pool(&mut round.variable_pool);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowInternalHttpRequest {
    pub method: String,
    /// Origin-relative Console path, including an optional query string.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub max_response_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowInternalHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[cfg(test)]
#[path = "_tests/workflow_credentials.rs"]
mod tests;
