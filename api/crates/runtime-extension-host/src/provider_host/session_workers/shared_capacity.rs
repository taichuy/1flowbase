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
        let started = std::time::Instant::now();
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let available = system_memory_snapshot()?.available;
            let rss = rss_bytes(pid)
                .ok_or_else(|| transport_binding_error("shared worker RSS is unavailable"))?;
            {
                let mut workers = self.workers.lock().map_err(|_| {
                    transport_binding_error("shared worker admission state is unavailable")
                })?;
                workers
                    .retain(|pid, worker| !worker.active.is_empty() || rss_bytes(*pid).is_some());
                let mut outstanding = 0_u64;
                for (observed_pid, worker) in workers.iter() {
                    // Keep reservations when measurement disappears until all
                    // owners have finished cancellation/exit cleanup.
                    outstanding = outstanding
                        .saturating_add(unobserved(worker, rss_bytes(*observed_pid).unwrap_or(0)));
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
                            rss_at_admission: rss,
                        },
                    );
                    tracing::debug!(
                        pid,
                        measured_rss_bytes = rss,
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
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
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
}
