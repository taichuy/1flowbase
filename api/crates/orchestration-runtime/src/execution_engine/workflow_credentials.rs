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
