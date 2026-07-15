use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

const BUCKET_SECONDS: u64 = 10;
const ONE_MINUTE_SECONDS: u64 = 60;
const FIVE_MINUTES_SECONDS: u64 = 300;
const FIFTEEN_MINUTES_SECONDS: u64 = 900;
const ROLLING_BUCKETS: u64 = FIFTEEN_MINUTES_SECONDS / BUCKET_SECONDS;
const IDLE_CLEANUP_INTERVAL_SECONDS: u64 = 60;
const SLOW_EXECUTION_THRESHOLD: Duration = Duration::from_secs(30);
const HIGH_FAILURE_RATE: f64 = 0.2;
const HIGH_DISCONNECT_RATE: f64 = 0.2;
const HIGH_SLOW_RATIO: f64 = 0.3;
const HIGH_ACTIVE_PRESSURE: f64 = 0.8;
const MIN_FAILURE_ATTEMPTS_1M: u64 = 3;
const MIN_FAILURE_ATTEMPTS_5M: u64 = 5;
const MIN_DISCONNECT_ATTEMPTS_5M: u64 = 3;
const MIN_BUSY_ACTIVE_TOTAL: u64 = 3;

tokio::task_local! {
    static CURRENT_APPLICATION_ID: Uuid;
}

pub async fn scope_application_activity<F, T>(application_id: Uuid, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CURRENT_APPLICATION_ID.scope(application_id, future).await
}

pub fn current_application_id() -> Option<Uuid> {
    CURRENT_APPLICATION_ID.try_with(|value| *value).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationActivityKind {
    HttpRequest,
    SseConnection,
    WebSocketConnection,
    ApplicationExecution,
    ToolCall,
    ModelRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationActivityFinish {
    Completed,
    Failed,
    Cancelled,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivitySnapshot {
    pub meta: ApplicationRuntimeActivityMeta,
    pub active: ApplicationRuntimeActivityActive,
    pub peaks: ApplicationRuntimeActivityPeaks,
    pub rolling_minute: ApplicationRuntimeActivityRollingMinute,
    pub windows: ApplicationRuntimeActivityWindows,
    pub health: ApplicationRuntimeActivityHealth,
    pub age_distribution: ApplicationRuntimeActivityAgeDistribution,
    pub long_connection_age_distribution: ApplicationRuntimeActivityAgeDistribution,
    pub pressure: ApplicationRuntimeActivityPressure,
    pub resources: ApplicationRuntimeActivityResources,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityMeta {
    pub application_id: Uuid,
    pub scope: &'static str,
    pub storage: &'static str,
    pub instance_started_at: String,
    pub snapshot_at: String,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityActive {
    pub total: u64,
    pub http_requests: u64,
    pub sse_connections: u64,
    pub websocket_connections: u64,
    pub application_executions: u64,
    pub tool_calls: u64,
    pub model_requests: u64,
    pub waiting: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityPeaks {
    pub process_peak_concurrency: u64,
    pub recent_peak_concurrency: u64,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityRollingMinute {
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub disconnected: u64,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityWindows {
    pub one_minute: ApplicationRuntimeActivityWindow,
    pub five_minutes: ApplicationRuntimeActivityWindow,
    pub fifteen_minutes: ApplicationRuntimeActivityWindow,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityWindow {
    pub window_seconds: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub disconnected: u64,
    pub peak_concurrency: u64,
    pub failure_rate: f64,
    pub disconnect_rate: f64,
    pub throughput_per_minute: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationRuntimeActivityHealthState {
    Healthy,
    Busy,
    Slow,
    Unstable,
    Failing,
    FailingNow,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationRuntimeActivityTrend {
    Rising,
    Steady,
    Falling,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityHealth {
    pub state: ApplicationRuntimeActivityHealthState,
    pub failure_rate_1m: f64,
    pub failure_rate_5m: f64,
    pub failure_rate_15m: f64,
    pub disconnect_rate_5m: f64,
    pub slow_ratio: f64,
    pub active_pressure: f64,
    pub throughput_5m_per_minute: f64,
    pub throughput_15m_per_minute: f64,
    pub throughput_trend: ApplicationRuntimeActivityTrend,
    pub failure_trend: f64,
}

impl Default for ApplicationRuntimeActivityHealth {
    fn default() -> Self {
        Self {
            state: ApplicationRuntimeActivityHealthState::Healthy,
            failure_rate_1m: 0.0,
            failure_rate_5m: 0.0,
            failure_rate_15m: 0.0,
            disconnect_rate_5m: 0.0,
            slow_ratio: 0.0,
            active_pressure: 0.0,
            throughput_5m_per_minute: 0.0,
            throughput_15m_per_minute: 0.0,
            throughput_trend: ApplicationRuntimeActivityTrend::Steady,
            failure_trend: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityAgeDistribution {
    pub under_5s: u64,
    pub from_5s_to_30s: u64,
    pub from_30s_to_120s: u64,
    pub over_120s: u64,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityPressure {
    pub slow_active_executions: u64,
    pub execution_slots_used: Option<u64>,
    pub execution_slots_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ApplicationRuntimeActivityResources {
    pub process_rss_bytes: Option<u64>,
}

#[derive(Clone, Default)]
pub struct ApplicationRuntimeActivityTracker {
    inner: Arc<Mutex<ApplicationRuntimeActivityInner>>,
}

#[derive(Default)]
struct ApplicationRuntimeActivityInner {
    next_id: u64,
    last_cleanup_slot: u64,
    applications: HashMap<Uuid, ApplicationActivityState>,
}

#[derive(Default)]
struct ApplicationActivityState {
    active: HashMap<u64, ActiveActivity>,
    process_peak_concurrency: u64,
    buckets: Vec<RollingBucket>,
}

#[derive(Clone, Copy)]
struct ActiveActivity {
    kind: ApplicationActivityKind,
    started_at: Instant,
}

#[derive(Clone)]
struct RollingBucket {
    slot: u64,
    completed: u64,
    failed: u64,
    cancelled: u64,
    disconnected: u64,
    peak_concurrency: u64,
}

pub struct ApplicationActivityGuard {
    tracker: ApplicationRuntimeActivityTracker,
    application_id: Uuid,
    id: u64,
    finish: Option<ApplicationActivityFinish>,
}

impl ApplicationRuntimeActivityTracker {
    fn lock_inner(&self) -> MutexGuard<'_, ApplicationRuntimeActivityInner> {
        match self.inner.lock() {
            Ok(inner) => inner,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn start(
        &self,
        application_id: Uuid,
        kind: ApplicationActivityKind,
    ) -> ApplicationActivityGuard {
        let mut inner = self.lock_inner();
        inner.cleanup_idle_allocations();
        inner.next_id += 1;
        let id = inner.next_id;
        let state = inner.applications.entry(application_id).or_default();
        state.active.insert(
            id,
            ActiveActivity {
                kind,
                started_at: Instant::now(),
            },
        );
        state.process_peak_concurrency = state
            .process_peak_concurrency
            .max(state.active.len() as u64);
        state.record_current_peak();

        ApplicationActivityGuard {
            tracker: self.clone(),
            application_id,
            id,
            finish: Some(ApplicationActivityFinish::Completed),
        }
    }

    pub fn snapshot(
        &self,
        application_id: Uuid,
        instance_started_at: OffsetDateTime,
    ) -> ApplicationRuntimeActivitySnapshot {
        let snapshot_at = OffsetDateTime::now_utc();
        let mut inner = self.lock_inner();
        inner.cleanup_idle_allocations();
        let state = inner.applications.get(&application_id);
        let active = state
            .map(ApplicationActivityState::active_counts)
            .unwrap_or_default();
        let age_distribution = state
            .map(|state| state.age_distribution(false))
            .unwrap_or_default();
        let long_connection_age_distribution = state
            .map(|state| state.age_distribution(true))
            .unwrap_or_default();
        let pressure = state
            .map(ApplicationActivityState::pressure)
            .unwrap_or_default();
        let rolling_minute = state
            .map(ApplicationActivityState::rolling_minute)
            .unwrap_or_default();
        let windows = state
            .map(ApplicationActivityState::windows)
            .unwrap_or_default();
        let recent_peak_concurrency = state
            .map(ApplicationActivityState::recent_peak_concurrency)
            .unwrap_or_default();
        let process_peak_concurrency = state
            .map(|state| state.process_peak_concurrency)
            .unwrap_or_default();
        let health = runtime_health(&active, &pressure, &windows, recent_peak_concurrency);

        ApplicationRuntimeActivitySnapshot {
            meta: ApplicationRuntimeActivityMeta {
                application_id,
                scope: "current_instance",
                storage: "memory",
                instance_started_at: format_time(instance_started_at),
                snapshot_at: format_time(snapshot_at),
            },
            active,
            peaks: ApplicationRuntimeActivityPeaks {
                process_peak_concurrency,
                recent_peak_concurrency,
            },
            rolling_minute,
            health,
            windows,
            age_distribution,
            long_connection_age_distribution,
            pressure,
            resources: ApplicationRuntimeActivityResources {
                process_rss_bytes: process_rss_bytes(),
            },
        }
    }

    fn finish(&self, application_id: Uuid, id: u64, finish: ApplicationActivityFinish) {
        let mut inner = self.lock_inner();
        inner.cleanup_idle_allocations();
        let Some(state) = inner.applications.get_mut(&application_id) else {
            return;
        };
        if state.active.remove(&id).is_none() {
            return;
        }
        state.record_finish(finish);
    }
}

impl ApplicationRuntimeActivityInner {
    fn cleanup_idle_allocations(&mut self) {
        let slot = current_slot();
        if self.last_cleanup_slot + IDLE_CLEANUP_INTERVAL_SECONDS > slot {
            return;
        }
        self.last_cleanup_slot = slot;

        for state in self.applications.values_mut() {
            state.prune_expired_buckets(slot);
            if state.active.is_empty() && state.buckets.is_empty() {
                state.active = HashMap::new();
                state.buckets = Vec::new();
            }
        }
    }
}

impl ApplicationActivityGuard {
    pub fn finish(mut self, finish: ApplicationActivityFinish) {
        self.finish = Some(finish);
    }
}

impl Drop for ApplicationActivityGuard {
    fn drop(&mut self) {
        if let Some(finish) = self.finish.take() {
            self.tracker.finish(self.application_id, self.id, finish);
        }
    }
}

impl ApplicationActivityState {
    fn active_counts(&self) -> ApplicationRuntimeActivityActive {
        let mut active = ApplicationRuntimeActivityActive {
            total: self.active.len() as u64,
            ..Default::default()
        };
        for item in self.active.values() {
            match item.kind {
                ApplicationActivityKind::HttpRequest => active.http_requests += 1,
                ApplicationActivityKind::SseConnection => active.sse_connections += 1,
                ApplicationActivityKind::WebSocketConnection => active.websocket_connections += 1,
                ApplicationActivityKind::ApplicationExecution => active.application_executions += 1,
                ApplicationActivityKind::ToolCall => active.tool_calls += 1,
                ApplicationActivityKind::ModelRequest => active.model_requests += 1,
            }
        }
        active
    }

    fn pressure(&self) -> ApplicationRuntimeActivityPressure {
        ApplicationRuntimeActivityPressure {
            slow_active_executions: self
                .active
                .values()
                .filter(|item| item.kind == ApplicationActivityKind::ApplicationExecution)
                .filter(|item| item.started_at.elapsed() >= SLOW_EXECUTION_THRESHOLD)
                .count() as u64,
            execution_slots_used: None,
            execution_slots_limit: None,
        }
    }

    fn age_distribution(
        &self,
        long_connections_only: bool,
    ) -> ApplicationRuntimeActivityAgeDistribution {
        let mut distribution = ApplicationRuntimeActivityAgeDistribution::default();
        for item in self.active.values() {
            if long_connections_only
                && !matches!(
                    item.kind,
                    ApplicationActivityKind::SseConnection
                        | ApplicationActivityKind::WebSocketConnection
                )
            {
                continue;
            }
            let elapsed = item.started_at.elapsed();
            if elapsed < Duration::from_secs(5) {
                distribution.under_5s += 1;
            } else if elapsed < Duration::from_secs(30) {
                distribution.from_5s_to_30s += 1;
            } else if elapsed < Duration::from_secs(120) {
                distribution.from_30s_to_120s += 1;
            } else {
                distribution.over_120s += 1;
            }
        }
        distribution
    }

    fn record_current_peak(&mut self) {
        let active_count = self.active.len() as u64;
        let bucket = self.current_bucket();
        bucket.peak_concurrency = bucket.peak_concurrency.max(active_count);
    }

    fn record_finish(&mut self, finish: ApplicationActivityFinish) {
        let bucket = self.current_bucket();
        match finish {
            ApplicationActivityFinish::Completed => bucket.completed += 1,
            ApplicationActivityFinish::Failed => bucket.failed += 1,
            ApplicationActivityFinish::Cancelled => bucket.cancelled += 1,
            ApplicationActivityFinish::Disconnected => bucket.disconnected += 1,
        }
    }

    fn rolling_minute(&self) -> ApplicationRuntimeActivityRollingMinute {
        let window = self.window(ONE_MINUTE_SECONDS);
        ApplicationRuntimeActivityRollingMinute {
            completed: window.completed,
            failed: window.failed,
            cancelled: window.cancelled,
            disconnected: window.disconnected,
        }
    }

    fn windows(&self) -> ApplicationRuntimeActivityWindows {
        ApplicationRuntimeActivityWindows {
            one_minute: self.window(ONE_MINUTE_SECONDS),
            five_minutes: self.window(FIVE_MINUTES_SECONDS),
            fifteen_minutes: self.window(FIFTEEN_MINUTES_SECONDS),
        }
    }

    fn window(&self, seconds: u64) -> ApplicationRuntimeActivityWindow {
        let current_slot = current_slot();
        let bucket_count = seconds.div_ceil(BUCKET_SECONDS);
        let mut window = ApplicationRuntimeActivityWindow {
            window_seconds: seconds,
            ..Default::default()
        };
        for bucket in self
            .buckets
            .iter()
            .filter(|bucket| bucket.slot + bucket_count > current_slot)
        {
            window.completed += bucket.completed;
            window.failed += bucket.failed;
            window.cancelled += bucket.cancelled;
            window.disconnected += bucket.disconnected;
            window.peak_concurrency = window.peak_concurrency.max(bucket.peak_concurrency);
        }
        let attempts = window.completed + window.failed + window.cancelled;
        window.failure_rate = ratio(window.failed, attempts);
        window.disconnect_rate = ratio(
            window.disconnected,
            window.completed + window.failed + window.disconnected,
        );
        window.throughput_per_minute = window.completed as f64 / (seconds as f64 / 60.0);
        window
    }

    fn recent_peak_concurrency(&self) -> u64 {
        self.window(FIVE_MINUTES_SECONDS).peak_concurrency
    }

    fn current_bucket(&mut self) -> &mut RollingBucket {
        let slot = current_slot();
        if let Some(index) = self.buckets.iter().position(|bucket| bucket.slot == slot) {
            return &mut self.buckets[index];
        }
        self.prune_expired_buckets(slot);
        let index = self.buckets.len();
        self.buckets.push(RollingBucket {
            slot,
            completed: 0,
            failed: 0,
            cancelled: 0,
            disconnected: 0,
            peak_concurrency: 0,
        });
        &mut self.buckets[index]
    }

    fn prune_expired_buckets(&mut self, slot: u64) {
        self.buckets
            .retain(|bucket| bucket.slot + ROLLING_BUCKETS > slot);
    }
}

fn current_slot() -> u64 {
    OffsetDateTime::now_utc().unix_timestamp().max(0) as u64 / BUCKET_SECONDS
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn runtime_health(
    active: &ApplicationRuntimeActivityActive,
    pressure: &ApplicationRuntimeActivityPressure,
    windows: &ApplicationRuntimeActivityWindows,
    recent_peak_concurrency: u64,
) -> ApplicationRuntimeActivityHealth {
    let failure_rate_1m = windows.one_minute.failure_rate;
    let failure_rate_5m = windows.five_minutes.failure_rate;
    let failure_rate_15m = windows.fifteen_minutes.failure_rate;
    let disconnect_rate_5m = windows.five_minutes.disconnect_rate;
    let slow_ratio = ratio(
        pressure.slow_active_executions,
        active.application_executions,
    );
    let active_pressure = ratio(active.total, recent_peak_concurrency);
    let failure_attempts_1m =
        windows.one_minute.completed + windows.one_minute.failed + windows.one_minute.cancelled;
    let failure_attempts_5m = windows.five_minutes.completed
        + windows.five_minutes.failed
        + windows.five_minutes.cancelled;
    let disconnect_attempts_5m = windows.five_minutes.completed
        + windows.five_minutes.failed
        + windows.five_minutes.disconnected;
    let throughput_5m_per_minute = windows.five_minutes.throughput_per_minute;
    let throughput_15m_per_minute = windows.fifteen_minutes.throughput_per_minute;
    let throughput_delta = throughput_5m_per_minute - throughput_15m_per_minute;

    let state = if failure_attempts_1m >= MIN_FAILURE_ATTEMPTS_1M
        && failure_rate_1m >= HIGH_FAILURE_RATE
    {
        ApplicationRuntimeActivityHealthState::FailingNow
    } else if failure_attempts_5m >= MIN_FAILURE_ATTEMPTS_5M && failure_rate_5m >= HIGH_FAILURE_RATE
    {
        ApplicationRuntimeActivityHealthState::Failing
    } else if pressure.slow_active_executions > 0 && slow_ratio >= HIGH_SLOW_RATIO {
        ApplicationRuntimeActivityHealthState::Slow
    } else if disconnect_attempts_5m >= MIN_DISCONNECT_ATTEMPTS_5M
        && disconnect_rate_5m >= HIGH_DISCONNECT_RATE
    {
        ApplicationRuntimeActivityHealthState::Unstable
    } else if active.total >= MIN_BUSY_ACTIVE_TOTAL && active_pressure >= HIGH_ACTIVE_PRESSURE {
        ApplicationRuntimeActivityHealthState::Busy
    } else {
        ApplicationRuntimeActivityHealthState::Healthy
    };

    ApplicationRuntimeActivityHealth {
        state,
        failure_rate_1m,
        failure_rate_5m,
        failure_rate_15m,
        disconnect_rate_5m,
        slow_ratio,
        active_pressure,
        throughput_5m_per_minute,
        throughput_15m_per_minute,
        throughput_trend: trend(throughput_delta),
        failure_trend: failure_rate_5m - failure_rate_15m,
    }
}

fn trend(delta: f64) -> ApplicationRuntimeActivityTrend {
    if delta > 0.01 {
        ApplicationRuntimeActivityTrend::Rising
    } else if delta < -0.01 {
        ApplicationRuntimeActivityTrend::Falling
    } else {
        ApplicationRuntimeActivityTrend::Steady
    }
}

fn format_time(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

fn process_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages = status.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        Some(pages * 4096)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_reports_active_counts_peaks_and_recent_outcomes_without_payloads() {
        let tracker = ApplicationRuntimeActivityTracker::default();
        let application_id = Uuid::now_v7();
        let started_at = OffsetDateTime::now_utc();

        let http = tracker.start(application_id, ApplicationActivityKind::HttpRequest);
        let sse = tracker.start(application_id, ApplicationActivityKind::SseConnection);
        let model = tracker.start(application_id, ApplicationActivityKind::ModelRequest);
        model.finish(ApplicationActivityFinish::Failed);

        let snapshot = tracker.snapshot(application_id, started_at);
        assert_eq!(snapshot.meta.scope, "current_instance");
        assert_eq!(snapshot.meta.storage, "memory");
        assert_eq!(snapshot.active.total, 2);
        assert_eq!(snapshot.active.http_requests, 1);
        assert_eq!(snapshot.active.sse_connections, 1);
        assert_eq!(snapshot.active.model_requests, 0);
        assert_eq!(snapshot.peaks.process_peak_concurrency, 3);
        assert_eq!(snapshot.peaks.recent_peak_concurrency, 3);
        assert_eq!(snapshot.rolling_minute.failed, 1);
        assert_eq!(snapshot.age_distribution.under_5s, 2);
        assert_eq!(snapshot.long_connection_age_distribution.under_5s, 1);

        drop(http);
        sse.finish(ApplicationActivityFinish::Disconnected);
        let snapshot = tracker.snapshot(application_id, started_at);
        assert_eq!(snapshot.active.total, 0);
        assert_eq!(snapshot.rolling_minute.completed, 1);
        assert_eq!(snapshot.rolling_minute.disconnected, 1);
    }

    #[test]
    fn tracker_recovers_poisoned_lock_without_panicking() {
        let tracker = ApplicationRuntimeActivityTracker::default();
        let poisoned_tracker = tracker.clone();
        let poison_result = std::thread::spawn(move || {
            let _inner = poisoned_tracker
                .inner
                .lock()
                .expect("activity tracker lock starts healthy");
            panic!("poison activity tracker lock");
        })
        .join();
        assert!(poison_result.is_err());

        let application_id = Uuid::now_v7();
        let started_at = OffsetDateTime::now_utc();
        let guard = tracker.start(application_id, ApplicationActivityKind::HttpRequest);

        let snapshot = tracker.snapshot(application_id, started_at);
        assert_eq!(snapshot.active.total, 1);
        assert_eq!(snapshot.active.http_requests, 1);

        drop(guard);

        let snapshot = tracker.snapshot(application_id, started_at);
        assert_eq!(snapshot.active.total, 0);
        assert_eq!(snapshot.rolling_minute.completed, 1);
    }

    #[test]
    fn tracker_reports_multi_window_health_without_request_history() {
        let tracker = ApplicationRuntimeActivityTracker::default();
        let application_id = Uuid::now_v7();
        let started_at = OffsetDateTime::now_utc();
        let slot = current_slot();

        {
            let mut inner = tracker.inner.lock().unwrap();
            inner.applications.insert(
                application_id,
                ApplicationActivityState {
                    active: HashMap::from([
                        (
                            1,
                            ActiveActivity {
                                kind: ApplicationActivityKind::ApplicationExecution,
                                started_at: Instant::now()
                                    .checked_sub(SLOW_EXECUTION_THRESHOLD)
                                    .unwrap(),
                            },
                        ),
                        (
                            2,
                            ActiveActivity {
                                kind: ApplicationActivityKind::HttpRequest,
                                started_at: Instant::now(),
                            },
                        ),
                    ]),
                    process_peak_concurrency: 8,
                    buckets: vec![
                        RollingBucket {
                            slot,
                            completed: 7,
                            failed: 3,
                            cancelled: 0,
                            disconnected: 2,
                            peak_concurrency: 2,
                        },
                        RollingBucket {
                            slot: slot - 12,
                            completed: 40,
                            failed: 2,
                            cancelled: 0,
                            disconnected: 1,
                            peak_concurrency: 5,
                        },
                        RollingBucket {
                            slot: slot - 60,
                            completed: 120,
                            failed: 0,
                            cancelled: 0,
                            disconnected: 0,
                            peak_concurrency: 6,
                        },
                    ],
                },
            );
        }

        let snapshot = tracker.snapshot(application_id, started_at);

        assert_eq!(snapshot.windows.one_minute.completed, 7);
        assert_eq!(snapshot.windows.five_minutes.completed, 47);
        assert_eq!(snapshot.windows.fifteen_minutes.completed, 167);
        assert!((snapshot.health.failure_rate_1m - 0.3).abs() < f64::EPSILON);
        assert!(snapshot.health.failure_rate_5m > snapshot.health.failure_rate_15m);
        assert_eq!(
            snapshot.health.state,
            ApplicationRuntimeActivityHealthState::FailingNow
        );
    }
}
