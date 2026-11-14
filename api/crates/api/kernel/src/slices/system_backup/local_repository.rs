#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupVerificationReceipt {
    pub verified: bool,
    pub checked_at: time::OffsetDateTime,
}
