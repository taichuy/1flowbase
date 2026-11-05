use super::runtime_types::*;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(super) struct Observation {
    entries: VecDeque<(Instant, u64, EventRuntimeActivity)>,
    bytes: u64,
    policy: Option<EventCenterRuntimePolicy>,
}
impl Observation {
    pub fn new(policy: Option<EventCenterRuntimePolicy>) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            policy,
        }
    }
    pub fn record(&mut self, activity: EventRuntimeActivity) {
        let Some(policy) = self.policy else {
            return;
        };
        let now = Instant::now();
        let ttl = Duration::from_millis(policy.observation_ttl_ms);
        while self
            .entries
            .front()
            .is_some_and(|(at, _, _)| now.saturating_duration_since(*at) >= ttl)
        {
            if let Some((_, bytes, _)) = self.entries.pop_front() {
                self.bytes -= bytes;
            }
        }
        if policy.observation_ttl_ms == 0 {
            return;
        }
        let Ok(bytes) = serde_json::to_vec(&activity).map(|value| value.len() as u64) else {
            return;
        };
        if bytes > policy.observation_max_bytes {
            return;
        }
        while bytes > policy.observation_max_bytes.saturating_sub(self.bytes) {
            let Some((_, removed, _)) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= removed;
        }
        self.bytes += bytes;
        self.entries.push_back((now, bytes, activity));
    }
    /// Pure read: expiration is projected here; collection happens only on mutation.
    pub fn snapshot(&self, workspace_id: Uuid) -> (Vec<EventRuntimeActivity>, u64) {
        let Some(policy) = self.policy else {
            return (Vec::new(), 0);
        };
        let now = Instant::now();
        let ttl = Duration::from_millis(policy.observation_ttl_ms);
        let mut bytes = 0;
        let activity = self
            .entries
            .iter()
            .filter(|(at, _, value)| {
                value.workspace_id == workspace_id && now.saturating_duration_since(*at) < ttl
            })
            .map(|(_, size, value)| {
                bytes += size;
                value.clone()
            })
            .collect();
        (activity, bytes)
    }
}

#[cfg(test)]
#[path = "_tests/observation.rs"]
mod tests;
