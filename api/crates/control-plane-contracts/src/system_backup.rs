use async_trait::async_trait;
use domain::{
    ArtifactRebuildability, BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupComponentRestoreTarget, BackupSourceIdentity,
};
use thiserror::Error;

use crate::ports::BackupComponentWriter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupComponentDescriptor {
    pub component_id: BackupComponentId,
    pub kind: BackupComponentKind,
    pub source_identity: BackupSourceIdentity,
    pub content_type: String,
    pub disposition: BackupComponentDisposition,
    pub rebuildability: ArtifactRebuildability,
    pub restore_target: BackupComponentRestoreTarget,
}

#[derive(Debug, Error)]
pub enum BackupSourceError {
    #[error("backup source is unavailable")]
    Unavailable,
    #[error("backup source changed while being captured")]
    Changed,
    #[error("backup source is invalid")]
    Invalid,
}

#[async_trait]
pub trait BackupComponentSource: Send + Sync {
    fn descriptor(&self) -> BackupComponentDescriptor;

    async fn write_to(&self, destination: BackupComponentWriter) -> Result<(), BackupSourceError>;
}
