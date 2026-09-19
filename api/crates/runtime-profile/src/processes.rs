use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
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
    system: System,
    users: Users,
    previous_sampled_at: Option<Instant>,
    snapshot: RuntimeProcessSnapshot,
}

/// Enumerates every process the current process can see through the OS, using
/// only the current user's permissions. The visible set is bounded by the
/// runtime PID namespace, not by this sampler. Termination is limited to
/// processes this user may actually signal.
pub struct RuntimeProcessSampler {
    inner: Mutex<RuntimeProcessSamplerInner>,
}

impl Default for RuntimeProcessSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProcessSampler {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(RuntimeProcessSamplerInner {
                system: System::new(),
                users: Users::new_with_refreshed_list(),
                previous_sampled_at: None,
                snapshot: RuntimeProcessSnapshot::default(),
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
            return inner.snapshot.clone();
        }

        inner.system.refresh_memory();
        inner.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        let current_pid = sysinfo::get_current_pid().ok();
        let protected_pids = current_pid
            .map(|pid| protected_process_pids(&inner.system, pid))
            .unwrap_or_default();
        let backend_pids = current_pid
            .map(|pid| descendant_process_pids(&inner.system, pid))
            .unwrap_or_default();
        let current_uid = current_pid
            .and_then(|pid| inner.system.process(pid))
            .and_then(|process| process.user_id())
            .cloned();

        let total_memory = inner.system.total_memory().max(1);
        // sysinfo reports process CPU per logical core (one busy core == 100%).
        // Divide by the logical CPU count so 100% means the whole machine.
        let logical_cpu_count = inner.system.cpus().len().max(1) as f32;
        let mut processes = inner
            .system
            .processes()
            .values()
            .filter(|process| process.thread_kind().is_none())
            .map(|process| RuntimeProcessSample {
                pid: process.pid().as_u32(),
                parent_pid: process.parent().map(|pid| pid.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                command: (!process.cmd().is_empty()).then(|| {
                    process
                        .cmd()
                        .iter()
                        .map(|argument| argument.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" ")
                }),
                user: process
                    .user_id()
                    .and_then(|user_id| inner.users.get_user_by_id(user_id))
                    .map(|user| user.name().to_string()),
                status: process_status_label(process.status()).to_string(),
                cpu_usage_percent: (process.cpu_usage() / logical_cpu_count).clamp(0.0, 100.0),
                memory_bytes: process.memory(),
                memory_usage_percent: (process.memory() as f64 / total_memory as f64 * 100.0)
                    as f32,
                start_time_unix_seconds: process.start_time(),
                terminable: is_terminable(process, &protected_pids, current_uid.as_ref()),
                backend_process: backend_pids.contains(&process.pid()),
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
        let snapshot = RuntimeProcessSnapshot { total, processes };
        inner.snapshot = snapshot.clone();
        inner.previous_sampled_at = Some(sampled_at);
        snapshot
    }

    /// Sends `SIGTERM` to one process after re-applying the same guards used
    /// for the `terminable` projection.
    pub fn terminate(&self, pid: u32) -> RuntimeProcessTerminationOutcome {
        let Ok(mut inner) = self.inner.lock() else {
            return RuntimeProcessTerminationOutcome::Failed;
        };
        let target = Pid::from_u32(pid);
        inner.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::nothing().with_user(UpdateKind::OnlyIfNotSet),
        );

        let Some(current_pid) = sysinfo::get_current_pid().ok() else {
            return RuntimeProcessTerminationOutcome::Failed;
        };
        let protected_pids = protected_process_pids(&inner.system, current_pid);
        let current_uid = inner
            .system
            .process(current_pid)
            .and_then(|process| process.user_id())
            .cloned();
        let Some(process) = inner.system.process(target) else {
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
