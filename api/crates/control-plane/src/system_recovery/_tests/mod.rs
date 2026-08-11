use std::{sync::Arc, time::Duration};

use domain::RecoveryJobId;
use time::OffsetDateTime;

use super::{
    SystemMaintenance, SystemMaintenanceDrainError, SystemMaintenancePhase, SystemWriteOwner,
};

mod coordinator_tests;

#[tokio::test]
async fn maintenance_fences_new_writes_and_waits_for_every_owner_to_drain() {
    let maintenance = Arc::new(SystemMaintenance::default());
    let api_write = maintenance
        .try_enter_write(SystemWriteOwner::ApiMutation)
        .unwrap();
    let worker_write = maintenance
        .try_enter_write(SystemWriteOwner::WorkflowScheduleExecution)
        .unwrap();
    let job_id = RecoveryJobId::new();
    let lease = maintenance
        .begin(job_id, OffsetDateTime::now_utc())
        .unwrap();

    assert_eq!(
        maintenance
            .try_enter_write(SystemWriteOwner::ProviderRequestLogPersistence)
            .unwrap_err()
            .recovery_job_id,
        job_id
    );
    assert_eq!(maintenance.snapshot().active_write_count(), 2);
    assert_eq!(
        lease.wait_for_drain(Duration::from_millis(1)).await,
        Err(SystemMaintenanceDrainError::Timeout)
    );

    drop(api_write);
    drop(worker_write);
    let snapshot = lease.wait_for_drain(Duration::from_secs(1)).await.unwrap();
    assert_eq!(snapshot.phase, SystemMaintenancePhase::Active);
    assert_eq!(snapshot.active_write_count(), 0);
}

#[tokio::test]
async fn dropping_the_current_lease_reopens_writes() {
    let maintenance = Arc::new(SystemMaintenance::default());
    let lease = maintenance
        .begin(RecoveryJobId::new(), OffsetDateTime::now_utc())
        .unwrap();
    lease.wait_for_drain(Duration::from_secs(1)).await.unwrap();
    drop(lease);

    assert_eq!(maintenance.snapshot().phase, SystemMaintenancePhase::Online);
    maintenance
        .try_enter_write(SystemWriteOwner::ApiMutation)
        .unwrap();
}
