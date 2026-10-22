use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};

/// Fixture phases; payloads contain no database URLs, credentials or SQL.
#[derive(Clone, Copy, Debug)]
pub enum PostgresTestPhase {
    /// Empty database/schema creation plus formal migrations, once per command.
    Prepare,
    /// Database clone plus explicit schema/ledger presence checks, no migrations.
    Clone,
    /// Owned database removal (including exact-owner orphan clones for a template).
    Cleanup,
    /// The caller's real migrator or ledger validation, including pool connection.
    Migration,
    /// Per-fixture application bootstrap; never part of the template.
    Bootstrap,
    /// Only empty database/schema creation, without migration or bootstrap.
    Fresh,
}

impl PostgresTestPhase {
    fn label(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Clone => "clone",
            Self::Cleanup => "cleanup",
            Self::Migration => "migration",
            Self::Bootstrap => "bootstrap",
            Self::Fresh => "fresh",
        }
    }
}

static METRICS_WRITE: Mutex<()> = Mutex::new(());

/// Append one phase to a process-owned JSONL file when metrics are requested.
/// The command owner supplies a directory under `tmp/test-governance`.
/// An absent variable is a no-op. Callers retain the original operation error
/// if optional metrics cannot be written.
pub fn record_phase_duration(
    phase: PostgresTestPhase,
    elapsed: Duration,
    succeeded: bool,
) -> io::Result<()> {
    let Some(directory) = std::env::var_os("POSTGRES_TEST_METRICS_DIR") else {
        return Ok(());
    };
    write_phase(&PathBuf::from(directory), phase, elapsed, succeeded)
}

pub(crate) fn write_phase(
    directory: &std::path::Path,
    phase: PostgresTestPhase,
    elapsed: Duration,
    succeeded: bool,
) -> io::Result<()> {
    let _write = METRICS_WRITE
        .lock()
        .map_err(|_| io::Error::other("PostgreSQL test metrics writer was poisoned"))?;
    std::fs::create_dir_all(directory)?;
    let pid = std::process::id();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(format!("postgres-test-{pid}.jsonl")))?;
    writeln!(
        file,
        "{{\"phase\":\"{}\",\"elapsed_ns\":{},\"succeeded\":{},\"pid\":{pid}}}",
        phase.label(),
        elapsed.as_nanos(),
        succeeded,
    )
}

pub(crate) fn record(phase: PostgresTestPhase, elapsed: Duration, succeeded: bool) {
    if let Err(error) = record_phase_duration(phase, elapsed, succeeded) {
        eprintln!("could not write optional PostgreSQL test phase metrics: {error}");
    }
}
