use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sysinfo::{
    Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, Uid, UpdateKind, Users,
    MINIMUM_CPU_UPDATE_INTERVAL,
};

/// Upper bound on how many process samples one response carries. Kept at the
/// managed-interface `maxItems` ceiling so the projection schema stays valid.
pub const MAX_RUNTIME_PROCESS_SAMPLES: usize = 256;

/// Short cache so a 2s console poll does not re-scan `/proc` several times.
const PROCESS_SAMPLE_TTL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeProcessSample {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: Option<String>,
    pub user: Option<String>,
    pub status: String,
    /// CPU usage as a percentage of the whole machine (all logical CPUs):
    /// `100%` means every logical CPU is fully busy with this process.
    pub cpu_usage_percent: f32,
    /// CPU usage relative to one logical core; may exceed `100%` when the
    /// process runs on multiple cores.
    pub cpu_usage_single_core_percent: f32,
    pub memory_bytes: u64,
    pub memory_usage_percent: f32,
    pub start_time_unix_seconds: u64,
    /// Whether the current process may signal this process: same user, not the
    /// current process, not one of its ancestors, and not the namespace init.
    pub terminable: bool,
    /// Whether this process belongs to the 1flowbase backend tree: the API
    /// server itself or one of its descendants (plugin/runtime workers).
    pub backend_process: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RuntimeProcessSnapshot {
    pub total: usize,
    pub processes: Vec<RuntimeProcessSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProcessTerminationOutcome {
    /// `SIGTERM` was delivered.
    Signalled,
    /// The PID is no longer observable in the current namespace.
    NotObservable,
    /// Guarded: own process, an ancestor, namespace init, or a different user.
    Forbidden,
    /// The OS rejected the signal.
    Failed,
}

struct RuntimeProcessSamplerInner {
    previous_sampled_at: Option<Instant>,
    snapshot: Arc<RuntimeProcessSnapshot>,
}

/// Enumerates every process the current process can see through the OS, using
/// only the current user's permissions. The visible set is bounded by the
/// runtime PID namespace, not by this sampler. Termination is limited to
/// processes this user may actually signal.
pub struct RuntimeProcessSampler {
    source: Arc<crate::RuntimeSampleSource>,
    inner: Mutex<RuntimeProcessSamplerInner>,
}

impl Default for RuntimeProcessSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProcessSampler {
    pub fn new() -> Self {
        Self::with_sample_source(Arc::new(crate::RuntimeSampleSource::default()))
    }
    pub fn with_sample_source(source: Arc<crate::RuntimeSampleSource>) -> Self {
        Self {
            source,
            inner: Mutex::new(RuntimeProcessSamplerInner {
                previous_sampled_at: None,
                snapshot: Arc::new(RuntimeProcessSnapshot::default()),
            }),
        }
    }

    pub fn collect(&self) -> RuntimeProcessSnapshot {
        let Ok(mut inner) = self.inner.lock() else {
            return RuntimeProcessSnapshot::default();
        };
        let sampled_at = Instant::now();
        let refresh_floor = PROCESS_SAMPLE_TTL.max(MINIMUM_CPU_UPDATE_INTERVAL);
        if inner
            .previous_sampled_at
            .is_some_and(|previous| sampled_at.duration_since(previous) < refresh_floor)
        {
            return inner.snapshot.as_ref().clone();
        }

        let Ok(snapshot) = self.source.collect_processes() else {
            return RuntimeProcessSnapshot::default();
        };
        inner.snapshot = snapshot;
        inner.previous_sampled_at = Some(sampled_at);
        inner.snapshot.as_ref().clone()
    }

    /// Sends `SIGTERM` to one process after re-applying the same guards used
    /// for the `terminable` projection.
    pub fn terminate(&self, pid: u32) -> RuntimeProcessTerminationOutcome {
        self.source.terminate(pid)
    }
}

pub(crate) fn snapshot_from_system(
    system: &System,
    metadata_system: &System,
    users: &Users,
) -> RuntimeProcessSnapshot {
    let current_pid = sysinfo::get_current_pid().ok();
    let protected_pids = current_pid
        .map(|pid| protected_process_pids(system, pid))
        .unwrap_or_default();
    let backend_pids = current_pid
        .map(|pid| descendant_process_pids(system, pid))
        .unwrap_or_default();
    let current_uid = current_pid
        .and_then(|pid| system.process(pid))
        .and_then(|process| matching_process_metadata(process, metadata_system))
        .and_then(|process| process.user_id())
        .cloned();

    let total_memory = system.total_memory().max(1);
    // sysinfo reports process CPU per logical core (one busy core == 100%).
    let logical_cpu_count = system.cpus().len().max(1) as f32;
    let mut processes = system
        .processes()
        .values()
        .filter(|process| process.thread_kind().is_none())
        .map(|process| {
            let metadata = matching_process_metadata(process, metadata_system);
            let (cpu_usage_percent, cpu_usage_single_core_percent) =
                process_cpu_usage_percentages(process.cpu_usage(), logical_cpu_count);
            RuntimeProcessSample {
                pid: process.pid().as_u32(),
                parent_pid: process.parent().map(|pid| pid.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                command: metadata.and_then(|metadata| {
                    (!metadata.cmd().is_empty()).then(|| {
                        metadata
                            .cmd()
                            .iter()
                            .map(|argument| argument.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                }),
                user: metadata
                    .and_then(|metadata| metadata.user_id())
                    .and_then(|user_id| users.get_user_by_id(user_id))
                    .map(|user| user.name().to_string()),
                status: process_status_label(process.status()).to_string(),
                cpu_usage_percent,
                cpu_usage_single_core_percent,
                memory_bytes: process.memory(),
                memory_usage_percent: (process.memory() as f64 / total_memory as f64 * 100.0)
                    as f32,
                start_time_unix_seconds: process.start_time(),
                terminable: metadata.is_some_and(|metadata| {
                    is_terminable(metadata, &protected_pids, current_uid.as_ref())
                }),
                backend_process: backend_pids.contains(&process.pid()),
            }
        })
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| {
        right
            .memory_bytes
            .cmp(&left.memory_bytes)
            .then_with(|| left.pid.cmp(&right.pid))
    });

    let total = processes.len();
    processes.truncate(MAX_RUNTIME_PROCESS_SAMPLES);
    RuntimeProcessSnapshot { total, processes }
}

/// The primary observation owns PID, parent, status, CPU and memory. Optional
/// fields may join only its public PID/start-time identity (seconds precision).
fn matching_process_metadata<'a>(
    process: &sysinfo::Process,
    metadata_system: &'a System,
) -> Option<&'a sysinfo::Process> {
    metadata_system.process(process.pid()).filter(|metadata| {
        metadata.pid() == process.pid() && metadata.start_time() == process.start_time()
    })
}

pub(crate) fn terminate_from_system(
    system: &mut System,
    pid: u32,
) -> RuntimeProcessTerminationOutcome {
    let target = Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[target]),
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_user(UpdateKind::OnlyIfNotSet),
    );

    let Some(current_pid) = sysinfo::get_current_pid().ok() else {
        return RuntimeProcessTerminationOutcome::Failed;
    };
    let protected_pids = protected_process_pids(system, current_pid);
    let current_uid = system
        .process(current_pid)
        .and_then(|process| process.user_id())
        .cloned();
    let Some(process) = system.process(target) else {
        return RuntimeProcessTerminationOutcome::NotObservable;
    };
    if !is_terminable(process, &protected_pids, current_uid.as_ref()) {
        return RuntimeProcessTerminationOutcome::Forbidden;
    }
    match process.kill_with(Signal::Term) {
        Some(true) => RuntimeProcessTerminationOutcome::Signalled,
        Some(false) | None => RuntimeProcessTerminationOutcome::Failed,
    }
}

pub(crate) fn process_cpu_usage_percentages(
    usage_per_core: f32,
    logical_cpu_count: f32,
) -> (f32, f32) {
    let single_core = usage_per_core.max(0.0);
    (
        (single_core / logical_cpu_count).clamp(0.0, 100.0),
        single_core,
    )
}

/// The current process, its ancestors, and PID 1 must never be signalled: they
/// own the console itself or its namespace.
fn protected_process_pids(system: &System, current_pid: Pid) -> HashSet<Pid> {
    let mut protected = HashSet::from([current_pid, Pid::from_u32(1)]);
    let mut cursor = system
        .process(current_pid)
        .and_then(|process| process.parent());
    while let Some(pid) = cursor {
        if !protected.insert(pid) {
            break;
        }
        cursor = system.process(pid).and_then(|process| process.parent());
    }
    protected
}

/// The API server process and every process below it in the parent chain:
/// in-process runtime host workers and plugin subprocesses.
fn descendant_process_pids(system: &System, root: Pid) -> HashSet<Pid> {
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for process in system.processes().values() {
        if process.thread_kind().is_some() {
            continue;
        }
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(process.pid());
        }
    }

    let mut descendants = HashSet::new();
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        if !descendants.insert(pid) {
            continue;
        }
        if let Some(next) = children.get(&pid) {
            pending.extend(next.iter().copied());
        }
    }
    descendants
}

fn is_terminable(
    process: &sysinfo::Process,
    protected_pids: &HashSet<Pid>,
    current_uid: Option<&Uid>,
) -> bool {
    if protected_pids.contains(&process.pid()) {
        return false;
    }
    if matches!(process.status(), sysinfo::ProcessStatus::Zombie) {
        // A zombie is already dead and only awaits reaping; no signal can end it.
        return false;
    }
    match (process.user_id(), current_uid) {
        (Some(target_uid), Some(own_uid)) => target_uid == own_uid,
        _ => false,
    }
}

fn process_status_label(status: sysinfo::ProcessStatus) -> &'static str {
    use sysinfo::ProcessStatus as Status;
    match status {
        Status::Idle => "idle",
        Status::Run => "running",
        Status::Sleep => "sleeping",
        Status::Stop => "stopped",
        Status::Zombie => "zombie",
        Status::Tracing => "tracing",
        Status::Dead => "dead",
        Status::Wakekill => "wakekill",
        Status::Waking => "waking",
        Status::Parked => "parked",
        Status::LockBlocked => "lock_blocked",
        Status::UninterruptibleDiskSleep => "uninterruptible_disk_sleep",
        Status::Unknown(_) => "unknown",
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "_tests/processes_scope_tests.rs"]
mod process_scope_tests;
