//! Stable lifetime port for the four owned operations whose protocol waiter may disappear.
#[derive(Clone, Copy, Debug)]
pub enum ManagedOwnedOperation {
    Candidate,
    Create,
    DerivedPublication,
    Retirement,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManagedOwnedOperationSnapshot {
    pub closed: bool,
    /// Counts in Candidate, Create, DerivedPublication, Retirement order.
    pub active: [usize; 4],
}
pub trait ManagedOperationPermit: Send + Sync {}
#[async_trait::async_trait]
pub trait ManagedOperationLifetime: Send + Sync {
    fn admit(
        &self,
        operation: ManagedOwnedOperation,
    ) -> anyhow::Result<Box<dyn ManagedOperationPermit>>;
    fn close(&self);
    fn snapshot(&self) -> ManagedOwnedOperationSnapshot;
    /// Timeout ends only the wait, never the admitted business operation.
    async fn wait(&self, budget: std::time::Duration) -> anyhow::Result<()>;
}
