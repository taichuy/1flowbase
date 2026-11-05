use super::runtime_types::*;
use std::{
    collections::VecDeque,
    mem::size_of,
    time::{Duration, Instant},
};
use uuid::Uuid;

type Entry = (Instant, u64, EventRuntimeActivity);

/// Fixed-size, payload-free ledger. Both ledgers and activity use the same host budget.
struct Accounting {
    workspace_id: Uuid,
    started: Instant,
    started_at_unix_ms: i64,
    expired: u64,
    evicted: u64,
    oversize_dropped: u64,
}
const ACCOUNTING_BYTES: u64 = size_of::<Accounting>() as u64;

pub(super) struct Observation {
    entries: VecDeque<Entry>,
    accounting: VecDeque<Accounting>,
    bytes: u64,
    policy: Option<EventCenterRuntimePolicy>,
}
impl Observation {
    pub fn new(policy: Option<EventCenterRuntimePolicy>) -> Self {
        Self {
            entries: VecDeque::new(),
            accounting: VecDeque::new(),
            bytes: 0,
            policy,
        }
    }
    pub fn record(&mut self, activity: EventRuntimeActivity) {
        self.record_at(activity, Instant::now());
    }

    fn record_at(&mut self, activity: EventRuntimeActivity, now: Instant) {
        let Some(policy) = self.policy else {
            return;
        };
        let ttl = Duration::from_millis(policy.observation_ttl_ms);
        let window = accounting_window(policy);
        // Expire accounting first: losses outside its finite coverage cannot be counted.
        self.accounting.retain(|ledger| {
            let retain = now.saturating_duration_since(ledger.started) < window;
            if !retain {
                self.bytes -= ACCOUNTING_BYTES;
            }
            retain
        });
        while self
            .entries
            .front()
            .is_some_and(|(at, _, _)| now.saturating_duration_since(*at) >= ttl)
        {
            let (at, bytes, expired) = self.entries.pop_front().expect("front exists");
            self.bytes -= bytes;
            if let Some(ledger) = self.ledger_mut(expired.workspace_id) {
                if expiration_in_window(at, ledger.started, now, ttl) {
                    ledger.expired = ledger.expired.saturating_add(1);
                }
            }
        }
        if ttl.is_zero() || ACCOUNTING_BYTES > policy.observation_max_bytes {
            return;
        }
        if self.ledger_mut(activity.workspace_id).is_none() {
            self.make_room(
                ACCOUNTING_BYTES,
                now,
                ttl,
                policy.observation_max_bytes,
                None,
            );
            self.accounting.push_back(Accounting {
                workspace_id: activity.workspace_id,
                started: now,
                started_at_unix_ms: activity.observed_at_unix_ms,
                expired: 0,
                evicted: 0,
                oversize_dropped: 0,
            });
            self.bytes += ACCOUNTING_BYTES;
        }
        let Ok(bytes) = serde_json::to_vec(&activity)
            .map(|value| (value.len() as u64).saturating_add(size_of::<Entry>() as u64))
        else {
            return;
        };
        // An activity must fit alongside its own accounting metadata.
        if bytes
            > policy
                .observation_max_bytes
                .saturating_sub(ACCOUNTING_BYTES)
        {
            if let Some(ledger) = self.ledger_mut(activity.workspace_id) {
                ledger.oversize_dropped = ledger.oversize_dropped.saturating_add(1);
            }
            return;
        }
        self.make_room(
            bytes,
            now,
            ttl,
            policy.observation_max_bytes,
            Some(activity.workspace_id),
        );
        self.bytes += bytes;
        self.entries.push_back((now, bytes, activity));
    }

    fn ledger_mut(&mut self, workspace_id: Uuid) -> Option<&mut Accounting> {
        self.accounting
            .iter_mut()
            .find(|ledger| ledger.workspace_id == workspace_id)
    }

    fn make_room(
        &mut self,
        bytes: u64,
        now: Instant,
        ttl: Duration,
        max_bytes: u64,
        protected_workspace: Option<Uuid>,
    ) {
        while bytes > max_bytes.saturating_sub(self.bytes) {
            if let Some((at, removed, activity)) = self.entries.pop_front() {
                self.bytes -= removed;
                if let Some(ledger) = self.ledger_mut(activity.workspace_id) {
                    // Entries are pruned before admission, so these are budget losses.
                    if now.saturating_duration_since(at) < ttl {
                        ledger.evicted = ledger.evicted.saturating_add(1);
                    }
                }
            } else if let Some(index) = self
                .accounting
                .iter()
                .position(|ledger| Some(ledger.workspace_id) != protected_workspace)
            {
                self.accounting.remove(index);
                // Evicted accounting is explicitly unavailable, never exported globally.
                self.bytes -= ACCOUNTING_BYTES;
            } else {
                break;
            }
        }
    }

    /// Pure read: expiration and finite accounting coverage are projected, never consumed.
    pub fn snapshot(
        &self,
        workspace_id: Uuid,
    ) -> (Vec<EventRuntimeActivity>, u64, EventObservationLossSnapshot) {
        self.snapshot_at(workspace_id, Instant::now())
    }

    fn snapshot_at(
        &self,
        workspace_id: Uuid,
        now: Instant,
    ) -> (Vec<EventRuntimeActivity>, u64, EventObservationLossSnapshot) {
        let duration_ms = self
            .policy
            .map_or(0, |policy| policy.observation_ttl_ms.saturating_mul(2));
        let mut loss = EventObservationLossSnapshot {
            accounting_available: false,
            window_started_at_unix_ms: None,
            window_duration_ms: duration_ms,
            expired: 0,
            evicted: 0,
            oversize_dropped: 0,
        };
        let Some(policy) = self.policy else {
            return (Vec::new(), 0, loss);
        };
        let ttl = Duration::from_millis(policy.observation_ttl_ms);
        let ledger = self.accounting.iter().find(|ledger| {
            ledger.workspace_id == workspace_id
                && now.saturating_duration_since(ledger.started) < accounting_window(policy)
        });
        let mut bytes = 0;
        if let Some(ledger) = ledger {
            bytes = ACCOUNTING_BYTES;
            loss.accounting_available = true;
            loss.window_started_at_unix_ms = Some(ledger.started_at_unix_ms);
            loss.expired = ledger.expired;
            loss.evicted = ledger.evicted;
            loss.oversize_dropped = ledger.oversize_dropped;
        }
        let mut activity = Vec::new();
        for (at, size, value) in &self.entries {
            if value.workspace_id != workspace_id {
                continue;
            }
            if now.saturating_duration_since(*at) < ttl {
                bytes += size;
                activity.push(value.clone());
            } else if ledger
                .is_some_and(|ledger| expiration_in_window(*at, ledger.started, now, ttl))
            {
                loss.expired = loss.expired.saturating_add(1);
            }
        }
        (activity, bytes, loss)
    }
}

fn accounting_window(policy: EventCenterRuntimePolicy) -> Duration {
    Duration::from_millis(policy.observation_ttl_ms.saturating_mul(2))
}

fn expiration_in_window(at: Instant, started: Instant, now: Instant, ttl: Duration) -> bool {
    now.saturating_duration_since(at).saturating_sub(ttl) <= now.saturating_duration_since(started)
}

#[cfg(test)]
#[path = "_tests/observation.rs"]
mod tests;
