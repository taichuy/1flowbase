//! Exercises the built helper directly; never launches a nested Cargo command.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    time::Duration,
};

use postgres_test_support::PostgresTestSchema;
use sqlx::PgPool;

const CHILD_MODE: &str = "POSTGRES_TEMPLATE_FIXTURE_CHILD_MODE";
const RECEIPT: &str = "POSTGRES_TEMPLATE_FIXTURE_RECEIPT";

fn base_database_url() -> String {
    std::env::var("API_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

struct OwnedWrapper(Child);

impl Drop for OwnedWrapper {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        #[cfg(unix)]
        {
            // Only the test-spawned wrapper PID is signalled. The wrapper owns
            // forwarding to its separate child group and database cleanup.
            let _ = Command::new("kill")
                .arg("-TERM")
                .arg("--")
                .arg(self.0.id().to_string())
                .status();
        }
        #[cfg(not(unix))]
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct EvidenceDirectory(PathBuf);

impl EvidenceDirectory {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .join("tmp/test-governance/postgres-template-command")
            .join(uuid::Uuid::now_v7().simple().to_string());
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for EvidenceDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spawn_wrapper(mode: &str, evidence: &EvidenceDirectory) -> OwnedWrapper {
    spawn_wrapper_with_urls(mode, evidence, None, &base_database_url())
}

fn spawn_wrapper_with_urls(
    mode: &str,
    evidence: &EvidenceDirectory,
    api_url: Option<&str>,
    database_url: &str,
) -> OwnedWrapper {
    let mut command = Command::new(env!("CARGO_BIN_EXE_with-postgres-template"));
    command
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "fixture_child", "--ignored", "--nocapture"])
        .env("DATABASE_URL", database_url)
        .env_remove("API_DATABASE_URL")
        .env(CHILD_MODE, mode)
        .env(RECEIPT, evidence.0.join("database-names"))
        .env("POSTGRES_TEST_METRICS_DIR", evidence.0.join("metrics"))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(api_url) = api_url {
        command.env("API_DATABASE_URL", api_url);
    }
    OwnedWrapper(command.spawn().unwrap())
}

async fn wait_for_receipt(wrapper: &mut OwnedWrapper, evidence: &EvidenceDirectory) -> Vec<String> {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let status = wrapper.0.try_wait().unwrap();
            if let Ok(receipt) = std::fs::read_to_string(evidence.0.join("database-names")) {
                let names: Vec<String> = receipt.lines().map(str::to_owned).collect();
                // The rename in the child makes publication atomic.
                assert_eq!(names.len(), 3);
                return names;
            }
            if let Some(status) = status {
                panic!("wrapper exited before publishing its owned fixture: {status}");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("wrapper should prepare its explicit template and clone")
}

async fn wait_for_exit(wrapper: &mut OwnedWrapper) -> ExitStatus {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(status) = wrapper.0.try_wait().unwrap() {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("wrapper should wait its child and clean owned databases")
}

async fn assert_databases_removed(names: &[String]) {
    let pool = PgPool::connect(&base_database_url()).await.unwrap();
    for name in &names[..2] {
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_database where datname=$1)")
                .bind(name)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!exists, "command-owned database {name} leaked");
    }
    pool.close().await;
}

#[tokio::test]
async fn command_preserves_success_and_failure_exit_codes_and_reaps_abrupt_clones() {
    for (mode, exit_code) in [("success", 0), ("failure", 23)] {
        let evidence = EvidenceDirectory::new();
        let mut wrapper = spawn_wrapper(mode, &evidence);
        let names = wait_for_receipt(&mut wrapper, &evidence).await;
        let status = wait_for_exit(&mut wrapper).await;
        assert_eq!(status.code(), Some(exit_code));
        assert_databases_removed(&names).await;
        let files: Vec<_> = std::fs::read_dir(evidence.0.join("metrics"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(
            files.len() >= 2,
            "wrapper and child must write separate process files"
        );
        let phases = files
            .iter()
            .map(|path| std::fs::read_to_string(path).unwrap())
            .collect::<String>();
        assert!(phases.contains("\"phase\":\"prepare\""));
        assert!(phases.contains("\"phase\":\"clone\""));
        assert!(phases.contains("\"phase\":\"cleanup\""));
        assert!(!phases.contains("postgres://"));
    }
}

#[tokio::test]
async fn command_preserves_api_database_precedence_when_database_url_targets_another_server() {
    let evidence = EvidenceDirectory::new();
    let base_url = base_database_url();
    let mut wrapper = spawn_wrapper_with_urls(
        "success",
        &evidence,
        Some(&base_url),
        "postgres://invalid@127.0.0.1:1/wrong_server",
    );
    let names = wait_for_receipt(&mut wrapper, &evidence).await;
    assert_eq!(wait_for_exit(&mut wrapper).await.code(), Some(0));
    assert_databases_removed(&names).await;
}

#[cfg(unix)]
#[tokio::test]
async fn command_forwards_term_and_int_waits_and_reaps_its_clones() {
    for (signal, exit_code) in [("-TERM", 143), ("-INT", 130)] {
        let evidence = EvidenceDirectory::new();
        let mut wrapper = spawn_wrapper("wait", &evidence);
        let names = wait_for_receipt(&mut wrapper, &evidence).await;
        let delivered = Command::new("kill")
            .arg(signal)
            .arg("--")
            .arg(wrapper.0.id().to_string())
            .status()
            .unwrap();
        assert!(delivered.success());
        assert_eq!(wait_for_exit(&mut wrapper).await.code(), Some(exit_code));
        assert_databases_removed(&names).await;
    }
}

#[tokio::test]
#[ignore = "subprocess fixture entry; the command lifecycle tests invoke it explicitly"]
async fn fixture_child() {
    let mode = std::env::var(CHILD_MODE).expect("the wrapper must select a fixture child mode");
    assert!(
        std::env::var_os("API_DATABASE_URL").is_some(),
        "a DATABASE_URL-only wrapper must make the API child's source explicit"
    );
    let database_name = std::env::var("POSTGRES_TEST_TEMPLATE_DATABASE").unwrap();
    let schema_name = std::env::var("POSTGRES_TEST_TEMPLATE_SCHEMA").unwrap();
    let clone = PostgresTestSchema::create_from_template(
        &base_database_url(),
        &database_name,
        &schema_name,
    )
    .await
    .unwrap();
    let clone_name = clone.database_name().to_owned();
    let pool = clone.connect().await.unwrap();
    sqlx::migrate!("../storage/durable/postgres/migrations")
        .run(&pool)
        .await
        .unwrap();
    let receipt = PathBuf::from(std::env::var_os(RECEIPT).unwrap());
    let pending = receipt.with_extension("pending");
    std::fs::write(
        &pending,
        format!("{database_name}\n{clone_name}\n{schema_name}\n"),
    )
    .unwrap();
    std::fs::rename(pending, receipt).unwrap();
    match mode.as_str() {
        "success" => {
            pool.close().await;
            drop(pool);
        }
        "failure" => std::process::exit(23),
        "wait" => std::future::pending::<()>().await,
        _ => panic!("unknown child fixture mode"),
    }
}
