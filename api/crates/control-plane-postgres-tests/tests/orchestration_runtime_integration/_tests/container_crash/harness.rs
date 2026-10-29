use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) struct ReceiptLedger {
    pub address: String,
    pub receipts: Arc<std::sync::Mutex<Vec<Value>>>,
    pub received: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}
impl ReceiptLedger {
    pub async fn start(hold_first_response: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let receipts = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received = Arc::new(tokio::sync::Notify::new());
        let records = receipts.clone();
        let signal = received.clone();
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                let count = {
                    let mut records = records.lock().unwrap();
                    records.push(serde_json::from_str(&line).unwrap());
                    records.len()
                };
                // Recording happens in the parent, outside the engine process.
                signal.notify_one();
                if hold_first_response && count == 1 {
                    // EOF from the killed child releases this socket, so a fresh
                    // recovery process can connect without reusing its state.
                    let mut byte = [0];
                    let _ = reader.read(&mut byte).await;
                } else {
                    reader.get_mut().write_all(b"recorded\n").await.unwrap();
                }
            }
        });
        Self {
            address,
            receipts,
            received,
            task,
        }
    }
}
impl Drop for ReceiptLedger {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) fn spawn_child(config: &Value, log: &std::path::Path) -> tokio::process::Child {
    tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "container_crash::container_crash_child_process",
            "--nocapture",
        ])
        .env("BOUNDED_CONTAINER_CRASH_CHILD", config.to_string())
        .stdout(std::fs::File::create(log).unwrap())
        .stderr(std::fs::File::options().append(true).open(log).unwrap())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

pub(super) async fn wait_for_db_gate(store: &PgControlPlaneStore, blocker_pid: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(25), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))",
            )
            .bind(blocker_pid)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if waiting {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("child must reach the requested PostgreSQL barrier; inspect child log");
}

pub(super) async fn install_gate(store: &PgControlPlaneStore, phase: &str) {
    let (table, event, predicate) = match phase {
        "before_round_start" => ("flow_container_rounds", "insert", "new.round_index=0"),
        "after_round_start" => ("node_runs", "insert", "new.node_id='node-llm'"),
        "after_node_commit" => (
            "flow_container_rounds",
            "update",
            "new.round_index=0 and new.status='succeeded'",
        ),
        "after_round_advance" => ("flow_container_rounds", "insert", "new.round_index=1"),
        "after_break_marker" => (
            "node_runs",
            "update",
            "new.node_id='loop' and new.status='succeeded'",
        ),
        "external_effect" => return,
        _ => panic!("unknown crash phase"),
    };
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "create function container_crash_gate() returns trigger language plpgsql as $$ begin if {predicate} then perform pg_advisory_xact_lock(27110005); end if; return new; end $$"
    ))).execute(store.pool()).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "create trigger container_crash_gate before {event} on {table} for each row execute function container_crash_gate()"
    ))).execute(store.pool()).await.unwrap();
}
pub(super) async fn remove_gate(store: &PgControlPlaneStore, phase: &str) {
    if phase != "external_effect" {
        sqlx::query("drop function container_crash_gate() cascade")
            .execute(store.pool())
            .await
            .unwrap();
    }
}
