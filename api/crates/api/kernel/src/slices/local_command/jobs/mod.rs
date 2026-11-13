//! Process-owned jobs. No replay after restart, no ephemeral-store cache, no global registry.
mod output;
mod types;
use super::process;
use output::{Output, Sink};
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, MutexGuard, Weak},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub use types::*;
use uuid::Uuid;

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn unix_ms() -> Result<u64, JobError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| JobError::InvalidInput("system clock precedes Unix epoch".into()))?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| JobError::InvalidInput("system clock overflow".into()))
}

#[derive(Default)]
pub struct JobRegistry {
    jobs: Mutex<HashMap<Uuid, Arc<Job>>>,
}
struct Job {
    owner: JobOwner,
    input: StartInput,
    started: Instant,
    cancellation: process::Cancellation,
    state: Mutex<State>,
    changed: Condvar,
    output: Arc<Mutex<Output>>,
}
struct State {
    snapshot: JobSnapshot,
    stop: Option<JobState>,
    finished: Option<Instant>,
    shutdown: bool,
}
impl Job {
    fn snapshot(&self) -> JobSnapshot {
        let mut snapshot = lock(&self.state).snapshot.clone();
        let output = lock(&self.output);
        snapshot.stdout_bytes = output.stdout_bytes;
        snapshot.stderr_bytes = output.stderr_bytes;
        snapshot.discarded_bytes = output.discarded_bytes;
        snapshot.truncated = output.discarded_bytes != 0;
        if snapshot.state == JobState::Running {
            snapshot.duration_ms =
                u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        }
        snapshot
    }
    fn expired(&self) -> bool {
        lock(&self.state).finished.is_some_and(|finished| {
            finished.elapsed() >= Duration::from_millis(self.input.retention_ms)
        })
    }
    fn shutdown(&self) {
        let mut state = lock(&self.state);
        state.shutdown = true;
        if state.snapshot.state == JobState::Running {
            state.stop.get_or_insert(JobState::Cancelled);
            self.cancellation.cancel();
        }
        self.changed.notify_all();
    }
}
impl Drop for JobRegistry {
    fn drop(&mut self) {
        for job in lock(&self.jobs).values() {
            job.shutdown();
        }
    }
}
impl JobRegistry {
    pub fn start(
        self: &Arc<Self>,
        owner: JobOwner,
        input: StartInput,
    ) -> Result<JobSnapshot, JobError> {
        if input.command.trim().is_empty() || input.command.contains('\0') {
            return Err(JobError::InvalidInput(
                "command must be nonempty and contain no NUL".into(),
            ));
        }
        if input.timeout_ms == 0 || input.retention_ms == 0 || input.max_output_bytes == 0 {
            return Err(JobError::InvalidInput(
                "timeout_ms, retention_ms and max_output_bytes must be positive".into(),
            ));
        }
        let started = Instant::now();
        let deadline = started
            .checked_add(Duration::from_millis(input.timeout_ms))
            .ok_or_else(|| JobError::InvalidInput("timeout_ms overflow".into()))?;
        deadline
            .checked_add(Duration::from_millis(input.retention_ms))
            .ok_or_else(|| JobError::InvalidInput("retention_ms overflow".into()))?;
        let created = unix_ms()?;
        created
            .checked_add(input.timeout_ms)
            .and_then(|time| time.checked_add(input.retention_ms))
            .ok_or_else(|| JobError::InvalidInput("timestamp overflow".into()))?;
        let mut jobs = lock(&self.jobs);
        jobs.retain(|_, job| !job.expired());
        if let Some(job) = jobs
            .values()
            .find(|job| job.owner == owner && job.input.request_id == input.request_id)
        {
            return if job.input == input {
                Ok(job.snapshot())
            } else {
                Err(JobError::Conflict)
            };
        }
        // Reserve both private files before any process can start.
        let output = Arc::new(Mutex::new(Output::new(input.max_output_bytes)?));
        let execution_id = Uuid::now_v7();
        let snapshot = JobSnapshot {
            execution_id,
            request_id: input.request_id,
            state: JobState::Running,
            cancel_requested: false,
            exit_code: None,
            signal: None,
            duration_ms: 0,
            output_complete: false,
            cleanup_complete: false,
            truncated: false,
            discarded_bytes: 0,
            stdout_bytes: 0,
            stderr_bytes: 0,
            created_at_unix_ms: created,
            expires_at_unix_ms: None,
            timeout_ms: input.timeout_ms,
            retention_ms: input.retention_ms,
            max_output_bytes: input.max_output_bytes,
        };
        let job = Arc::new(Job {
            owner,
            input,
            started,
            cancellation: process::Cancellation::default(),
            state: Mutex::new(State {
                snapshot: snapshot.clone(),
                stop: None,
                finished: None,
                shutdown: false,
            }),
            changed: Condvar::new(),
            output,
        });
        jobs.insert(execution_id, job.clone());
        let watched = job.clone();
        let registry = Arc::downgrade(self);
        if let Err(error) = std::thread::Builder::new()
            .name("command-job-lifecycle".into())
            .spawn(move || monitor(registry, watched, deadline))
        {
            jobs.remove(&execution_id);
            return Err(error.into());
        }
        let running = job.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("command-job".into())
            .spawn(move || run(running))
        {
            job.shutdown();
            jobs.remove(&execution_id);
            return Err(error.into());
        }
        Ok(snapshot)
    }
    fn find(&self, owner: &JobOwner, id: Uuid) -> Result<Arc<Job>, JobError> {
        let mut jobs = lock(&self.jobs);
        if jobs.get(&id).is_some_and(|job| job.expired()) {
            jobs.remove(&id);
        }
        jobs.get(&id)
            .filter(|job| job.owner == *owner)
            .cloned()
            .ok_or(JobError::NotFound)
    }
    pub fn get(&self, owner: &JobOwner, id: Uuid) -> Result<JobSnapshot, JobError> {
        Ok(self.find(owner, id)?.snapshot())
    }
    pub fn read(
        &self,
        owner: &JobOwner,
        id: Uuid,
        input: ReadInput,
    ) -> Result<ReadOutput, JobError> {
        let job = self.find(owner, id)?;
        let complete = lock(&job.state).snapshot.state != JobState::Running;
        let result = lock(&job.output).read(input, complete);
        result
    }
    pub fn cancel(&self, owner: &JobOwner, id: Uuid) -> Result<JobSnapshot, JobError> {
        let job = self.find(owner, id)?;
        {
            let mut state = lock(&job.state);
            if state.snapshot.state == JobState::Running {
                state.snapshot.cancel_requested = true;
                state.stop.get_or_insert(JobState::Cancelled);
                job.cancellation.cancel();
                job.changed.notify_all();
            }
        }
        Ok(job.snapshot())
    }
}

fn run(job: Arc<Job>) {
    // The native implementation joins both readers and reaps the shell before returning.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        process::execute_observed(
            &job.input.command,
            &job.cancellation,
            Arc::new(Sink {
                output: job.output.clone(),
                stream: OutputStream::Stdout,
            }),
            Arc::new(Sink {
                output: job.output.clone(),
                stream: OutputStream::Stderr,
            }),
        )
    }))
    .unwrap_or_else(|_| Err(std::io::Error::other("execution worker panicked")));
    #[cfg(test)]
    if let Err(error) = &result {
        eprintln!(
            "local-command job={} execution failed: {error:?}",
            job.input.request_id
        );
    }
    let mut state = lock(&job.state);
    state.snapshot.state = match &result {
        Ok(metadata) => state.stop.unwrap_or(if metadata.exit_code == Some(0) {
            JobState::Completed
        } else {
            JobState::Failed
        }),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
            state.stop.unwrap_or(JobState::Failed)
        }
        Err(_) => JobState::Failed,
    };
    state.snapshot.duration_ms =
        u64::try_from(job.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Ok(metadata) = result {
        state.snapshot.exit_code = metadata.exit_code;
        state.snapshot.signal = metadata.signal;
        state.snapshot.output_complete = metadata.output_complete;
        state.snapshot.cleanup_complete = metadata.cleanup_complete;
    }
    state.snapshot.expires_at_unix_ms = unix_ms()
        .ok()
        .and_then(|time| time.checked_add(job.input.retention_ms));
    state.finished = Some(Instant::now());
    job.changed.notify_all();
}

fn monitor(registry: Weak<JobRegistry>, job: Arc<Job>, deadline: Instant) {
    let mut state = lock(&job.state);
    while state.snapshot.state == JobState::Running && !state.shutdown {
        if Instant::now() >= deadline && state.stop.is_none() {
            state.stop = Some(JobState::TimedOut);
            job.cancellation.cancel();
        }
        state = if state.stop.is_some() {
            job.changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        } else {
            job.changed
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0
        };
    }
    while !state.shutdown {
        let Some(finished) = state.finished else {
            return;
        };
        let retention = Duration::from_millis(job.input.retention_ms);
        if finished.elapsed() >= retention {
            break;
        }
        state = job
            .changed
            .wait_timeout(state, retention.saturating_sub(finished.elapsed()))
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0;
    }
    let id = state.snapshot.execution_id;
    drop(state);
    if let Some(registry) = registry.upgrade() {
        lock(&registry.jobs).remove(&id);
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
