//! Admission for shared workers measures current pressure and reserves only
//! unobserved call growth. Process address-space protection is a separate policy.
use extension_contracts::{MULTIPLEX_CALL_EVENT_BUDGET_BYTES, MULTIPLEX_OUTPUT_BUDGET_BYTES};
use extension_package_runtime::error::FrameworkResult;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

use super::super::operations::transport_binding_error;
use super::{admission_timeout, capacity::system_memory_snapshot};

#[derive(Debug, Default)]
struct WorkerMemory {
    next_ticket: u64,
    // A peak observation is an estimate, not a promised process limit.
    observed_increment: u64,
    active: HashMap<u64, Reservation>,
}

#[derive(Debug)]
struct Reservation {
    bytes: u64,
    rss_at_admission: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedWorkerCapacity {
    workers: Arc<Mutex<HashMap<u32, WorkerMemory>>>,
    changed: Arc<Notify>,
}

#[derive(Debug)]
pub(crate) struct SharedInvocationPermit {
    capacity: SharedWorkerCapacity,
    pid: u32,
    ticket: u64,
}

fn rss_bytes(pid: u32) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        status.lines().find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
                .and_then(|value| value.checked_mul(1024))
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let pid = sysinfo::Pid::from_u32(pid);
        let mut system = sysinfo::System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        system.process(pid).map(|process| process.memory())
    }
}

fn unobserved(worker: &WorkerMemory, rss: u64) -> u64 {
    let active = worker.active.len().max(1) as u64;
    worker
        .active
        .values()
        .map(|reservation| {
            // Each observed page is credited at most once across concurrent calls.
            // A decrease in RSS restores the reservation for possible future growth.
            let realized = rss.saturating_sub(reservation.rss_at_admission) / active;
            reservation.bytes.saturating_sub(realized)
        })
        .fold(0, u64::saturating_add)
}

impl SharedWorkerCapacity {
    pub(super) async fn acquire(
        &self,
        pid: u32,
        request_bytes: usize,
        deadline: tokio::time::Instant,
    ) -> FrameworkResult<SharedInvocationPermit> {
        self.acquire_sampled(
            pid,
            request_bytes,
            deadline,
            || Ok(system_memory_snapshot()?.available),
            rss_bytes,
            std::time::Duration::from_secs(1),
        )
        .await
    }

    // The sampler is internal so tests can drive the real wait/reservation path
    // without depending on the machine's current memory pressure or a live PID.
    async fn acquire_sampled<A, R>(
        &self,
        pid: u32,
        request_bytes: usize,
        deadline: tokio::time::Instant,
        available_bytes: A,
        rss: R,
        recheck: std::time::Duration,
    ) -> FrameworkResult<SharedInvocationPermit>
    where
        A: Fn() -> FrameworkResult<u64>,
        R: Fn(u32) -> Option<u64>,
    {
        let started = std::time::Instant::now();
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let available = available_bytes()?;
            let measured_rss = rss(pid)
                .ok_or_else(|| transport_binding_error("shared worker RSS is unavailable"))?;
            {
                let mut workers = self.workers.lock().map_err(|_| {
                    transport_binding_error("shared worker admission state is unavailable")
                })?;
                workers.retain(|pid, worker| !worker.active.is_empty() || rss(*pid).is_some());
                let mut outstanding = 0_u64;
                for (observed_pid, worker) in workers.iter() {
                    // Keep reservations when measurement disappears until all
                    // owners have finished cancellation/exit cleanup.
                    outstanding = outstanding
                        .saturating_add(unobserved(worker, rss(*observed_pid).unwrap_or(0)));
                }
                let worker = workers.entry(pid).or_default();
                let reservation = (request_bytes as u64)
                    .saturating_mul(2)
                    .saturating_add(MULTIPLEX_CALL_EVENT_BUDGET_BYTES as u64)
                    .max(worker.observed_increment);
                // Host writer and SDK output buffers are shared by the process,
                // so charge their potential growth once, never once per session.
                let fixed_buffers = (workers.len() as u64)
                    .saturating_mul((MULTIPLEX_OUTPUT_BUDGET_BYTES as u64).saturating_mul(2));
                if available
                    >= outstanding
                        .saturating_add(reservation)
                        .saturating_add(fixed_buffers)
                {
                    let worker = workers.get_mut(&pid).expect("inserted worker");
                    worker.next_ticket = worker.next_ticket.checked_add(1).ok_or_else(|| {
                        transport_binding_error("shared worker admission identity exhausted")
                    })?;
                    let ticket = worker.next_ticket;
                    worker.active.insert(
                        ticket,
                        Reservation {
                            bytes: reservation,
                            rss_at_admission: measured_rss,
                        },
                    );
                    tracing::debug!(
                        pid,
                        measured_rss_bytes = measured_rss,
                        available_bytes = available,
                        unobserved_bytes = outstanding,
                        incremental_reservation_bytes = reservation,
                        wait_ms = started.elapsed().as_millis() as u64,
                        "shared worker invocation admitted"
                    );
                    return Ok(SharedInvocationPermit {
                        capacity: self.clone(),
                        pid,
                        ticket,
                    });
                }
            }
            tokio::time::timeout_at(deadline, async {
                tokio::select! {
                    _ = &mut changed => {},
                    _ = tokio::time::sleep(recheck) => {},
                }
            })
            .await
            .map_err(|_| admission_timeout())?;
        }
    }
}

impl Drop for SharedInvocationPermit {
    fn drop(&mut self) {
        if let Ok(mut workers) = self.capacity.workers.lock() {
            if let Some(worker) = workers.get_mut(&self.pid) {
                let active = worker.active.len().max(1) as u64;
                if let Some(reservation) = worker.active.remove(&self.ticket) {
                    if let Some(rss) = rss_bytes(self.pid) {
                        worker.observed_increment = worker
                            .observed_increment
                            .max(rss.saturating_sub(reservation.rss_at_admission) / active);
                    }
                }
            }
        }
        self.capacity.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    const TEST_PID: u32 = u32::MAX;

    fn one_call_bytes(request_bytes: usize) -> u64 {
        (request_bytes as u64)
            .saturating_mul(2)
            .saturating_add(MULTIPLEX_CALL_EVENT_BUDGET_BYTES as u64)
    }

    fn fixed_worker_bytes() -> u64 {
        (MULTIPLEX_OUTPUT_BUDGET_BYTES as u64) * 2
    }

    #[test]
    fn observed_growth_is_not_reserved_again_or_credited_to_every_call() {
        let worker = WorkerMemory {
            active: HashMap::from([
                (
                    1,
                    Reservation {
                        bytes: 100,
                        rss_at_admission: 1000,
                    },
                ),
                (
                    2,
                    Reservation {
                        bytes: 100,
                        rss_at_admission: 1000,
                    },
                ),
            ]),
            ..Default::default()
        };
        assert_eq!(unobserved(&worker, 1000), 200);
        assert_eq!(unobserved(&worker, 1100), 100);
        assert_eq!(unobserved(&worker, 1200), 0);
        assert_eq!(unobserved(&worker, 900), 200);
    }

    #[test]
    fn new_call_does_not_claim_pages_created_before_its_admission() {
        let worker = WorkerMemory {
            active: HashMap::from([
                (
                    1,
                    Reservation {
                        bytes: 100,
                        rss_at_admission: 1000,
                    },
                ),
                (
                    2,
                    Reservation {
                        bytes: 100,
                        rss_at_admission: 1100,
                    },
                ),
            ]),
            ..Default::default()
        };
        assert_eq!(unobserved(&worker, 1100), 150);
    }

    #[tokio::test]
    async fn ten_tiny_calls_share_fixed_buffers_under_low_pressure() {
        let capacity = SharedWorkerCapacity::default();
        let request_bytes = 16;
        let available = fixed_worker_bytes() + 10 * one_call_bytes(request_bytes);
        assert!(available < 10 * 256 * 1024 * 1024);
        let mut permits = Vec::new();
        for _ in 0..10 {
            permits.push(
                capacity
                    .acquire_sampled(
                        TEST_PID,
                        request_bytes,
                        tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                        || Ok(available),
                        |_| Some(1024),
                        std::time::Duration::from_secs(1),
                    )
                    .await
                    .unwrap(),
            );
        }
        assert_eq!(capacity.workers.lock().unwrap()[&TEST_PID].active.len(), 10);
        drop(permits);
    }

    #[tokio::test]
    async fn pressure_waits_until_permit_cleanup_then_admits_next_call() {
        let capacity = SharedWorkerCapacity::default();
        let request_bytes = 16;
        let available = fixed_worker_bytes() + one_call_bytes(request_bytes);
        let first = capacity
            .acquire_sampled(
                TEST_PID,
                request_bytes,
                tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                || Ok(available),
                |_| Some(1024),
                std::time::Duration::from_secs(1),
            )
            .await
            .unwrap();
        let samples = Arc::new(AtomicUsize::new(0));
        let waiting_capacity = capacity.clone();
        let waiting_samples = samples.clone();
        let waiter = tokio::spawn(async move {
            waiting_capacity
                .acquire_sampled(
                    TEST_PID,
                    request_bytes,
                    tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                    move || {
                        waiting_samples.fetch_add(1, Ordering::SeqCst);
                        Ok(available)
                    },
                    |_| Some(1024),
                    std::time::Duration::from_secs(1),
                )
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while samples.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(
            !waiter.is_finished(),
            "second call must wait under pressure"
        );
        drop(first);
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(capacity.workers.lock().unwrap()[&TEST_PID].active.len(), 1);
        drop(second);
    }

    #[tokio::test]
    async fn pressure_wait_ends_at_deadline() {
        let capacity = SharedWorkerCapacity::default();
        let available = Arc::new(AtomicU64::new(fixed_worker_bytes()));
        let sampled = available.clone();
        let error = capacity
            .acquire_sampled(
                TEST_PID,
                16,
                tokio::time::Instant::now() + std::time::Duration::from_millis(25),
                move || Ok(sampled.load(Ordering::SeqCst)),
                |_| Some(1024),
                std::time::Duration::from_secs(1),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(capacity.workers.lock().unwrap()[&TEST_PID]
            .active
            .is_empty());
    }
}
