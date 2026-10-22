//! Build this binary once, then execute it directly as:
//! `with-postgres-template -- <command> [args...]`.
//!
//! The child inherits the environment plus an explicit template database/schema.
//! Unix cancellation forwards INT/TERM to only the newly owned child process
//! group, waits for its leader, then reaps this command's clone databases. A
//! second cancellation kills that group. SIGKILL/OOM of this wrapper cannot run
//! Drop; children that deliberately leave the owned process group are not managed.

use std::{error::Error, ffi::OsString, io, process::ExitCode};

use postgres_test_support::PostgresTestTemplate;
use tokio::process::{Child, Command};

type CommandError = Box<dyn Error + Send + Sync>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            // Preserve operation errors without proactively printing environment
            // variables, connection strings or child arguments.
            eprintln!("with-postgres-template: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<u8, CommandError> {
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next().ok_or("expected -- <command> [args...]")?;
    let program = if first == "--" {
        arguments
            .next()
            .ok_or("expected a child command after --")?
    } else {
        first
    };
    let arguments: Vec<OsString> = arguments.collect();
    let base_url = std::env::var_os("API_DATABASE_URL")
        .or_else(|| std::env::var_os("DATABASE_URL"))
        .ok_or("API_DATABASE_URL or DATABASE_URL must be set")?
        .into_string()
        .map_err(|_| "selected database URL must be valid UTF-8")?;
    let mut signals = CancellationSignals::install()?;
    let prepare = PostgresTestTemplate::prepare(&base_url);
    tokio::pin!(prepare);
    let (template, cancelled) = tokio::select! {
        result = &mut prepare => (result?, None),
        signal = signals.receive() => {
            // Let in-flight migration finish and explicitly close its connection;
            // do not abandon an advisory-lock owner when cancellation arrives.
            let signal = signal?;
            (prepare.await?, Some(signal))
        }
    };
    let outcome = if let Some(signal) = cancelled {
        Ok(128 + signal)
    } else {
        run_child(&template, &base_url, program, arguments, &mut signals).await
    };
    let cleanup = template.cleanup().await;
    match (outcome, cleanup) {
        (Ok(0), Err(error)) => Err(error.into()),
        (result, Err(error)) => {
            eprintln!("with-postgres-template: owned database cleanup failed: {error}");
            result
        }
        (result, Ok(())) => result,
    }
}

async fn run_child(
    template: &PostgresTestTemplate,
    base_url: &str,
    program: OsString,
    arguments: Vec<OsString>,
    signals: &mut CancellationSignals,
) -> Result<u8, CommandError> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        // Preserve an existing API value exactly; when only DATABASE_URL was
        // supplied, make the API child's selected server explicit and identical.
        .env("API_DATABASE_URL", base_url)
        .env("POSTGRES_TEST_TEMPLATE_DATABASE", template.database_name())
        .env("POSTGRES_TEST_TEMPLATE_SCHEMA", template.schema_name())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn()?;
    let pid = child.id().ok_or("spawned child has no process identity")?;
    tokio::select! {
        status = child.wait() => Ok(exit_code(status?)),
        signal = signals.receive() => {
            let signal = match signal {
                Ok(signal) => signal,
                Err(error) => {
                    if let Err(reap_error) = child.kill().await {
                        eprintln!("with-postgres-template: could not reap child after signal error: {reap_error}");
                    }
                    return Err(error.into());
                }
            };
            if let Err(error) = forward_signal(&mut child, pid, signal).await {
                if let Err(reap_error) = child.kill().await {
                    eprintln!("with-postgres-template: could not reap child after forwarding error: {reap_error}");
                }
                return Err(error.into());
            }
            tokio::select! {
                status = child.wait() => {
                    status?;
                }
                cancellation = signals.receive() => {
                    if let Err(error) = cancellation {
                        eprintln!("with-postgres-template: cancellation watcher failed: {error}");
                    }
                    if let Err(error) = forward_signal(&mut child, pid, 9).await {
                        eprintln!("with-postgres-template: owned group force signal failed: {error}");
                        child.start_kill()?;
                    }
                    child.wait().await?;
                }
            }
            Ok(128 + signal)
        }
    }
}

fn exit_code(status: std::process::ExitStatus) -> u8 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .map(|code| code as u8)
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1) as u8)
    }
    #[cfg(not(unix))]
    {
        status.code().unwrap_or(1) as u8
    }
}

#[cfg(unix)]
async fn forward_signal(child: &mut Child, pid: u32, signal: u8) -> std::io::Result<()> {
    // The process may finish between wait/select and delivery; that is already
    // a successful cancellation boundary, not a reason to target a reused PID.
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg("--")
        .arg(format!("-{pid}"))
        .status()
        .await?;
    if status.success() || child.try_wait()?.is_some() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "owned child signal delivery failed ({status})"
        )))
    }
}

#[cfg(not(unix))]
async fn forward_signal(child: &mut Child, _pid: u32, _signal: u8) -> std::io::Result<()> {
    // std/Tokio do not expose a portable process-group signal API.
    child.start_kill()
}

#[cfg(unix)]
struct CancellationSignals {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl CancellationSignals {
    fn install() -> io::Result<Self> {
        use tokio::signal::unix::{signal, SignalKind};
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }

    async fn receive(&mut self) -> io::Result<u8> {
        tokio::select! {
            signal = self.interrupt.recv() => signal.map(|_| 2).ok_or_else(|| io::Error::other("SIGINT watcher closed")),
            signal = self.terminate.recv() => signal.map(|_| 15).ok_or_else(|| io::Error::other("SIGTERM watcher closed")),
        }
    }
}

#[cfg(not(unix))]
struct CancellationSignals;

#[cfg(not(unix))]
impl CancellationSignals {
    fn install() -> io::Result<Self> {
        Ok(Self)
    }

    async fn receive(&mut self) -> io::Result<u8> {
        tokio::signal::ctrl_c().await.map(|_| 2)
    }
}
