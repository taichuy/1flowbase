//! W3 / Root #2691: real Adapter, installed Python worker, Host carrier and PG callback.
//! These fixtures are executed only by the assembled Test Batch, never by packet development.
use crate::{
    app_state::ApiState,
    managed_publication::ManagedGenerationPublisher,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
};
use control_plane::plugin_management::*;
use control_plane_contracts::ports::ManagedServiceMutationRepository;
use extension_contracts::{PluginDataBinding, PluginDataFuture, PluginDataPort, PluginDataRequest};
use runtime_core::runtime_backend::CapabilityRuntimePort;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use storage_durable_postgres::{PgConnectionSettings, PgControlPlaneStore, PgPoolSettings};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tower::ServiceExt;
use uuid::Uuid;

// A per-installation barrier pauses the published reset owner before actual
// retirement. It neither owns the mutation nor changes the worker/PG protocol.
type ResetGate = (oneshot::Sender<()>, oneshot::Receiver<()>);
static RESET_GATES: OnceLock<Mutex<BTreeMap<Uuid, ResetGate>>> = OnceLock::new();
pub(crate) async fn reset_barrier(handle: &extension_contracts::ManagedExecutionHandle) {
    let id: Uuid = handle
        .identity()
        .installation_id()
        .as_str()
        .parse()
        .unwrap();
    let gate = RESET_GATES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .remove(&id);
    if let Some((entered, release)) = gate {
        let _ = entered.send(());
        let _ = release.await;
    }
}

struct Callback {
    store: PgControlPlaneStore,
    entered: mpsc::UnboundedSender<PluginDataBinding>,
    completed: mpsc::UnboundedSender<()>,
    gate: Semaphore,
    stall: Mutex<Option<Arc<CallbackStall>>>,
}
#[derive(Default)]
struct CallbackStall(Mutex<bool>, std::sync::Condvar);
impl CallbackStall {
    fn wait(&self) {
        let mut released = self.0.lock().unwrap();
        while !*released {
            released = self.1.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_all();
    }
}
struct ReleaseStall(Arc<CallbackStall>);
impl Drop for ReleaseStall {
    fn drop(&mut self) {
        self.0.release();
    }
}
impl PluginDataPort for Callback {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        input: &'a PluginDataRequest,
    ) -> PluginDataFuture<'a> {
        Box::pin(async move {
            let stall = self.stall.lock().unwrap().clone();
            self.entered.send(binding.clone()).unwrap();
            if let Some(stall) = stall {
                stall.wait();
            }
            self.gate.acquire().await.unwrap().forget();
            // Delegate to the real production repository: authority, target
            // resolution, transaction, SQL count and result restoration all run.
            let result = PluginDataPort::execute(&self.store, binding, input).await;
            let _ = self.completed.send(());
            result
        })
    }
}

const WORKER: &str = r#"#!/usr/bin/env python3
import json, os, sys, threading, time
calls={}
def send(kind,cid,**fields):
    print(json.dumps(dict(protocol='stdio_json_multiplex_v1',kind=kind,call_id=cid,**fields)),flush=True)
def ack(cid,b):
    if b.get('cancelled'): open(b['cancelled'],'w').close()
    if b.get('no_ack'): return
    while b.get('ack_gate') and not os.path.exists(b['ack_gate']): time.sleep(.005)
    send('cancelled',cid)
def terminal(cid,b):
    while not os.path.exists(b['terminal_gate']): time.sleep(.005)
    send('response',cid,response={'ok':True,'result':{'pid':os.getpid()}})
for line in sys.stdin:
    f=json.loads(line); cid=f['call_id']
    if f['kind']=='call':
        i=f['request']['input']; b=i['input_payload']['body']; calls[cid]=b
        if b.get('started'): open(b['started'],'w').write(str(os.getpid()))
        if b.get('dispatches'):
            with open(b['dispatches'],'a') as log: log.write(cid+'\n')
        if b.get('fail'):
            send('response',cid,response={'ok':False,'error':{'message':'credential=must-not-escape'}})
        elif b.get('invalid_output'): send('response',cid,response={'ok':True,'result':'invalid'})
        elif b.get('hold'): pass
        elif b.get('callback',True):
            send('callback',cid,callback_id='1',service='plugin_data_v1',request={'operations':[{'operation':'count','target':{'kind':'owned_collection','collection_code':'processed_models'},'filters':[]}]})
            if b.get('terminal_gate'): threading.Thread(target=terminal,args=(cid,b),daemon=True).start()
        else: send('response',cid,response={'ok':True,'result':{'pid':os.getpid(),'call_context':i.get('call_context')}})
    elif f['kind']=='callback_result':
        assert 'result' in f['response'], f['response']
        send('response',cid,response={'ok':True,'result':f['response']['result']})
    elif f['kind']=='cancel': threading.Thread(target=ack,args=(cid,calls[cid]),daemon=True).start()
"#;

fn package() -> Vec<u8> {
    let mut manifest: Value = serde_yaml::from_str(include_str!(
        "../../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    manifest["binding_targets"] = json!(["system"]);
    manifest["runtime"]["protocol"] = "stdio_json_multiplex_v1".into();
    let data = manifest["managed"]["module"]["contributions"][2].clone();
    let data_binding = manifest["managed"]["execution_bindings"][2].clone();
    let mut contributions = vec![data];
    let mut bindings = vec![data_binding];
    let mut operations = Vec::new();
    for (name, method, concurrency, reset) in [
        ("mutate", "POST", "serialized", false),
        ("parallel", "POST", "parallel", false),
        ("read", "GET", "serialized", false),
        ("reset", "POST", "serialized", true),
    ] {
        let id = format!("acme.composition-a.{name}");
        contributions.push(json!({"contribution_id":id,"contributor_module_id":"acme.composition-a","point_id":"1flowbase.managed-service.operation","contract_version":"1","required_permissions":["service.execute","plugin_data.owned.read"],"mode":"append"}));
        bindings.push(json!({"contribution_id":id,"execution_mode":"process_per_call","runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/worker.py"},"handler":name}));
        operations.push(json!({"interface_id":id,"contribution_id":id,"method":method,"path":format!("/api/console/managed-services/acme.composition-a/{name}"),"summary":"Exercise managed ownership","description":"Run the connection ownership fixture.","execution_concurrency":concurrency,"invalidates_streams":reset,"invalidates_workers":reset,"input_schema":{"type":"object"},"output_schema":{"type":"object"}}));
    }
    manifest["managed"]["module"]["contributions"] = contributions.into();
    manifest["managed"]["execution_bindings"] = bindings.into();
    manifest["managed_service"] = json!({"scope":"system","worker_lifetime":"actor_workspace_scope","worker_idle_timeout_ms":1800000,"feature":{"feature_id":"acme.composition-a.settings","label":"Ownership fixture","description":"Exercise managed ownership","route_id":"ownership-fixture","path":"/settings/ownership-fixture"},"operations":operations});
    let manifest = serde_yaml::to_string(&manifest).unwrap();
    plugin_framework::parse_plugin_manifest(&manifest).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (path, bytes, mode) in [
        ("manifest.yaml", manifest.as_bytes(), 0o644),
        ("bin/worker.py", WORKER.as_bytes(), 0o755),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

struct Fixture {
    state: Arc<ApiState>,
    publisher: Arc<ManagedGenerationPublisher>,
    host: Arc<runtime_extension_host::RuntimeExtensionHost>,
    callback: Arc<Callback>,
    entered: mpsc::UnboundedReceiver<PluginDataBinding>,
    completed: mpsc::UnboundedReceiver<()>,
    observer: PgPool,
    installation: Uuid,
    cookie: String,
    csrf: String,
    root: PathBuf,
}
impl Fixture {
    async fn new(business: u32) -> Self {
        let (initial, url) = crate::_tests::support::test_api_state_with_database_url().await;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(business)
            .connect(&url)
            .await
            .unwrap();
        let mut settings =
            PgConnectionSettings::compatible(PgPoolSettings::with_max_connections(business));
        settings.health_check_interval = Duration::from_millis(20);
        let mut state = initial.as_ref().clone();
        state.store = PgControlPlaneStore::with_connection_settings(pool, settings).unwrap();
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let graph = Arc::new(assembly.compile_graph().unwrap());
        let native = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let notifications = Arc::new(crate::host_extensions::notification_publication::HostOptionalNotificationPublication::default());
        let bindings = crate::host_extensions::lifecycle::production_lifecycle_handler_factories(
            notifications.clone(),
        )
        .unwrap()
        .activate(assembly.host_extension_manifests())
        .unwrap();
        let (_, catalog) =
            crate::host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(&native, bindings)
                .unwrap();
        state.store = state
            .store
            .with_lifecycle_publication_catalog(catalog.clone());
        notifications
            .activate(Arc::new(state.store.clone()))
            .unwrap();
        let (entered, entries) = mpsc::unbounded_channel();
        let (completed, completions) = mpsc::unbounded_channel();
        let callback = Arc::new(Callback {
            store: state.store.clone(),
            entered,
            completed,
            gate: Semaphore::new(0),
            stall: Mutex::new(None),
        });
        let host = Arc::new(runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver_and_plugin_data(time::OffsetDateTime::now_utc(), Arc::new(ApiRuntimeArtifactResolver::new(state.store.clone(), &state.api_node_id, &state.provider_install_root)), callback.clone()).unwrap());
        host.mark_ready().unwrap();
        state.provider_runtime = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(host.clone(), graph)
                .unwrap()
                .with_managed_composition(
                    state.store.clone(),
                    state.api_node_id.clone(),
                    assembly.module_descriptors().to_vec(),
                ),
        );
        let composition = state.provider_runtime.managed_composition().unwrap();
        composition.attach_native_lifecycle_plan(native).unwrap();
        catalog
            .attach_workspace_source(Arc::new(
                crate::extension_bus::ManagedWorkspacePublicationSource(Arc::downgrade(
                    &composition,
                )),
            ))
            .unwrap();
        let state = Arc::new(state);
        let actor: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let management = PluginManagementService::new(
            state.store.clone(),
            ApiProviderRuntime::new(state.provider_runtime.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id)
        .with_managed_activation();
        let installation = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor,
                file_name: "ownership.1flowbasepkg".into(),
                package_bytes: package(),
            })
            .await
            .unwrap()
            .installation
            .id;
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor,
                installation_id: installation,
            })
            .await
            .unwrap();
        let initial_app = crate::app_with_state(state.clone());
        let (cookie, csrf) =
            crate::_tests::support::login_and_capture_cookie(&initial_app, "root", "change-me")
                .await;
        let publisher = ManagedGenerationPublisher::new(
            &state,
            crate::_tests::support::test_config(),
            vec![],
            initial_app,
        )
        .unwrap();
        let registrations = crate::managed_services::load(&state.store, &state.api_node_id)
            .await
            .unwrap();
        publisher.publish(
            publisher
                .prepare(registrations, composition.current_snapshots().await)
                .await
                .unwrap(),
        );
        let observer = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        let root = std::env::temp_dir().join(format!("managed-ownership-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        Self {
            state,
            publisher,
            host,
            callback,
            entered: entries,
            completed: completions,
            observer,
            installation,
            cookie,
            csrf,
            root,
        }
    }
    fn request(&self, operation: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(if operation == "read" { "GET" } else { "POST" })
            .uri(format!(
                "/api/console/managed-services/acme.composition-a/{operation}"
            ))
            .header("cookie", &self.cookie)
            .header("x-csrf-token", &self.csrf)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }
    fn start(&self, operation: &str, body: Value) -> tokio::task::JoinHandle<Response> {
        let app = self.publisher.ingress();
        let request = self.request(operation, body);
        tokio::spawn(async move { app.oneshot(request).await.unwrap() })
    }
    async fn entered(&mut self) -> PluginDataBinding {
        tokio::time::timeout(Duration::from_secs(5), self.entered.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn locked(&self) -> bool {
        let mut transaction = self.observer.begin().await.unwrap();
        let available: bool = sqlx::query_scalar("select pg_try_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))").bind(self.installation.to_string()).bind(domain::SYSTEM_SCOPE_ID.to_string()).fetch_one(&mut *transaction).await.unwrap();
        transaction.rollback().await.unwrap();
        !available
    }
    async fn close(self) {
        self.host.stop().await.unwrap();
        self.state.store.connections().close().await;
        self.observer.close().await;
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
async fn eventually(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
async fn file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn response(task: tokio::task::JoinHandle<Response>) -> (StatusCode, Value) {
    let response = tokio::time::timeout(Duration::from_secs(8), task)
        .await
        .unwrap()
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn deadline() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 5000
}

#[tokio::test]
async fn adapter_host_pg_callback_progresses_at_p1_and_p5_with_four_local_followers() {
    for p in [1, 5] {
        let mut fixture = Fixture::new(p).await;
        let holder = fixture.start("mutate", json!({}));
        let binding = fixture.entered().await;
        assert_eq!(binding.workspace_id, domain::SYSTEM_SCOPE_ID.to_string());
        assert!(
            binding.actor_id.is_some(),
            "real caller capture must precede the worker"
        );
        let mut followers = Vec::new();
        for count in 1..=4 {
            followers.push(fixture.start("mutate", json!({})));
            eventually(|| {
                fixture
                    .state
                    .store
                    .connections()
                    .observe()
                    .managed
                    .local_queue
                    == count
            })
            .await;
        }
        let observation = fixture.state.store.connections().observe();
        assert_eq!(observation.business.max_connections, Some(p));
        assert_eq!(observation.managed.active, 1);
        assert_eq!(observation.managed.pool.size, 1);
        assert!(fixture.locked().await);
        let transactions: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and application_name='1flowbase:managed-lock' and xact_start is not null").fetch_one(&fixture.observer).await.unwrap();
        assert_eq!(transactions, 0);
        let mut checkouts = Vec::new();
        for _ in 0..p {
            let mut connection =
                tokio::time::timeout(Duration::from_secs(2), fixture.state.store.pool().acquire())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, i32>("select 7")
                    .fetch_one(&mut *connection)
                    .await
                    .unwrap(),
                7
            );
            checkouts.push(connection);
        }
        drop(checkouts);
        fixture.callback.gate.add_permits(5);
        let (status, output) = response(holder).await;
        assert_eq!(status, StatusCode::OK, "{output}");
        assert_eq!(
            output["data"]["results"][0]["count"], 0,
            "real PG callback result must be restored"
        );
        for task in followers {
            assert_eq!(response(task).await.0, StatusCode::OK);
        }
        eventually(|| fixture.state.store.connections().observe().managed.active == 0).await;
        assert!(!fixture.locked().await);
        fixture.close().await;
    }
}

#[tokio::test]
async fn legacy_business_transaction_lock_control_starves_the_real_worker_callback_at_p5() {
    let mut fixture = Fixture::new(5).await;
    // Parallel is deliberately lock-free; the controlled old guard below
    // reproduces the former holder + four same-key blocking transactions.
    let task = fixture.start("parallel", json!({}));
    fixture.entered().await;
    let mut holder = fixture.state.store.pool().begin().await.unwrap();
    const LEGACY: &str = "select pg_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))";
    sqlx::query(LEGACY)
        .bind(fixture.installation.to_string())
        .bind(domain::SYSTEM_SCOPE_ID.to_string())
        .execute(&mut *holder)
        .await
        .unwrap();
    let mut waiters = Vec::new();
    for _ in 0..4 {
        let pool = fixture.state.store.pool().clone();
        let id = fixture.installation;
        waiters.push(tokio::spawn(async move {
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query(LEGACY)
                .bind(id.to_string())
                .bind(domain::SYSTEM_SCOPE_ID.to_string())
                .execute(&mut *transaction)
                .await
                .unwrap();
        }));
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event='advisory'").fetch_one(&fixture.observer).await.unwrap();
            if blocked == 4 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert_eq!(fixture.state.store.pool().size(), 5);
    assert_eq!(fixture.state.store.pool().num_idle(), 0);
    assert!(fixture.state.store.pool().try_acquire().is_none());
    fixture.callback.gate.add_permits(1);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), fixture.completed.recv())
            .await
            .is_err(),
        "the old P5 topology must be distinguishable from the new ownership path"
    );
    for waiter in waiters {
        waiter.abort();
        let _ = waiter.await;
    }
    holder.rollback().await.unwrap();
    assert_eq!(response(task).await.0, StatusCode::OK);
    fixture.close().await;
}

#[tokio::test]
async fn mutation_repository_two_coordinators_preserve_key_compatibility_and_release_healthy_watch()
{
    let fixture = Fixture::new(1).await;
    let other_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*fixture.observer.connect_options()).clone())
        .await
        .unwrap();
    let other = PgControlPlaneStore::new(other_pool);
    let first = fixture
        .state
        .store
        .lock_managed_service_mutation(fixture.installation, domain::SYSTEM_SCOPE_ID, deadline())
        .await
        .unwrap();
    let weak = Arc::downgrade(&first.owner);
    let signal = first.lost.clone();
    let other_id = Uuid::now_v7();
    let different = other
        .lock_managed_service_mutation(other_id, domain::SYSTEM_SCOPE_ID, deadline())
        .await
        .unwrap();
    drop(different);
    eventually(|| other.connections().observe().managed.active == 0).await;
    let second_store = other.clone();
    let id = fixture.installation;
    let pending = tokio::spawn(async move {
        second_store
            .lock_managed_service_mutation(id, domain::SYSTEM_SCOPE_ID, deadline())
            .await
            .unwrap()
    });
    eventually(|| other.connections().observe().local_key_cells == 1).await;
    assert!(
        fixture.locked().await,
        "legacy xact try-lock must conflict with session owner"
    );
    assert!(!pending.is_finished());
    drop(first);
    let second = pending.await.unwrap();
    eventually(|| weak.upgrade().is_none() && signal.has_changed().is_err()).await;
    assert!(fixture.locked().await);
    drop(second);
    eventually(|| other.connections().observe().managed.active == 0).await;
    other.connections().close().await;
    fixture.close().await;
}

#[tokio::test]
async fn adapter_drop_keeps_pg_lock_until_delayed_ack_and_no_ack_reap() {
    for no_ack in [false, true] {
        let fixture = Fixture::new(1).await;
        let started = fixture.root.join("started");
        let cancelled = fixture.root.join("cancelled");
        let ack = fixture.root.join("ack");
        let task = fixture.start("mutate", json!({"hold":true,"started":started,"cancelled":cancelled,"ack_gate":ack,"no_ack":no_ack}));
        file(&started).await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        file(&cancelled).await;
        assert!(
            fixture.locked().await,
            "cancel notification cannot release ownership"
        );
        let pid: i32 = std::fs::read_to_string(&started).unwrap().parse().unwrap();
        assert!(Path::new(&format!("/proc/{pid}")).exists());
        if !no_ack {
            std::fs::write(&ack, "ack").unwrap();
        }
        eventually(|| fixture.state.store.connections().observe().managed.active == 0).await;
        if no_ack {
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "no ACK requires confirmed reap"
            );
        }
        assert!(!fixture.locked().await);
        fixture.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn terminal_response_keeps_pg_ownership_for_a_callback_still_executing() {
    let mut fixture = Fixture::new(1).await;
    let stall = Arc::new(CallbackStall::default());
    let release = ReleaseStall(stall.clone());
    *fixture.callback.stall.lock().unwrap() = Some(stall);
    let terminal = fixture.root.join("terminal");
    let started = fixture.root.join("started");
    let task = fixture.start(
        "mutate",
        json!({"terminal_gate":terminal,"started":started}),
    );
    fixture.entered().await;
    // The callback is inside a synchronous poll. Abort cannot destroy its
    // retained clone until that poll returns, even after the worker terminal.
    std::fs::write(&terminal, "terminal").unwrap();
    let (status, body) = response(task).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        fixture.locked().await,
        "terminal must not destroy in-flight callback ownership"
    );
    assert_eq!(
        fixture.state.store.connections().observe().managed.active,
        1
    );
    fixture.callback.gate.add_permits(1);
    drop(release);
    eventually(|| fixture.state.store.connections().observe().managed.active == 0).await;
    assert!(!fixture.locked().await);
    let pid = std::fs::read_to_string(started).unwrap();
    assert!(
        Path::new(&format!("/proc/{pid}")).exists(),
        "the operation must not pin its persistent process"
    );
    fixture.close().await;
}

#[tokio::test]
async fn mutation_queue_expiry_uses_one_deadline_without_an_extra_session() {
    let fixture = Fixture::new(1).await;
    let guard = fixture
        .state
        .store
        .lock_managed_service_mutation(fixture.installation, domain::SYSTEM_SCOPE_ID, deadline())
        .await
        .unwrap();
    let registry = fixture.state.store.connections().clone();
    let store = fixture.state.store.clone();
    let id = fixture.installation;
    let short = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 150;
    let waiting = tokio::spawn(async move {
        store
            .lock_managed_service_mutation(id, domain::SYSTEM_SCOPE_ID, short)
            .await
    });
    eventually(|| registry.observe().managed.local_queue == 1).await;
    assert_eq!(registry.observe().managed.pool.size, 1);
    assert!(waiting.await.unwrap().is_err());
    assert_eq!(registry.observe().managed.local_queue, 0);
    assert_eq!(registry.observe().managed.active, 1);
    drop(guard);
    eventually(|| registry.observe().managed.active == 0).await;
    assert!(!fixture.locked().await);
    fixture.close().await;
}

#[tokio::test]
async fn pg_session_loss_cancels_host_without_replaying_unknown_external_result() {
    let fixture = Fixture::new(1).await;
    let started = fixture.root.join("started");
    let cancelled = fixture.root.join("cancelled");
    let dispatches = fixture.root.join("dispatches");
    let task = fixture.start(
        "mutate",
        json!({"hold":true,"started":started,"cancelled":cancelled,"dispatches":dispatches}),
    );
    file(&started).await;
    let pid: i32 = sqlx::query_scalar("select pid from pg_stat_activity where datname=current_database() and application_name='1flowbase:managed-lock'").fetch_one(&fixture.observer).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("select pg_terminate_backend($1)")
            .bind(pid)
            .fetch_one(&fixture.observer)
            .await
            .unwrap()
    );
    let (status, output) = response(task).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{output}");
    file(&cancelled).await;
    eventually(|| {
        fixture
            .state
            .store
            .connections()
            .observe()
            .managed
            .local_queue
            == 0
            && fixture.state.store.connections().observe().local_key_cells == 0
    })
    .await;
    assert_eq!(
        std::fs::read_to_string(dispatches).unwrap().lines().count(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn adapter_reset_publication_keeps_ownership_after_caller_drop_until_reap() {
    let fixture = Fixture::new(1).await;
    let started = fixture.root.join("started");
    let (entered, entry) = oneshot::channel();
    let (release, gate) = oneshot::channel();
    RESET_GATES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .insert(fixture.installation, (entered, gate));
    let task = fixture.start("reset", json!({"callback":false,"started":started}));
    tokio::time::timeout(Duration::from_secs(5), entry)
        .await
        .unwrap()
        .unwrap();
    assert!(fixture.locked().await);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(
        fixture.locked().await,
        "published reset outlives the waiting adapter"
    );
    let pid = std::fs::read_to_string(&started).unwrap();
    assert!(Path::new(&format!("/proc/{pid}")).exists());
    release.send(()).unwrap();
    eventually(|| fixture.state.store.connections().observe().managed.active == 0).await;
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    assert!(!fixture.locked().await);
    fixture.close().await;
}

#[tokio::test]
async fn adapter_get_parallel_capture_contract_errors_and_generation_drain_are_preserved() {
    let fixture = Fixture::new(1).await;
    let anonymous = fixture
        .publisher
        .ingress()
        .oneshot(
            Request::builder()
                .uri("/api/console/managed-services/acme.composition-a/read")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        fixture.state.store.connections().observe().managed.active,
        0
    );
    let guard = fixture
        .state
        .store
        .lock_managed_service_mutation(fixture.installation, domain::SYSTEM_SCOPE_ID, deadline())
        .await
        .unwrap();
    for operation in ["read", "parallel"] {
        let (status, body) = response(fixture.start(operation, json!({"callback":false}))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["data"]["call_context"]["actor_id"].is_string());
    }
    drop(guard);
    eventually(|| fixture.state.store.connections().observe().managed.active == 0).await;
    for (input, expected_status, expected) in [
        (
            json!({"fail":true}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "managed_operation_failed",
        ),
        (
            json!({"invalid_output":true}),
            StatusCode::BAD_GATEWAY,
            "managed_service_output_contract",
        ),
    ] {
        let (status, body) = response(fixture.start("mutate", input)).await;
        assert_eq!(status, expected_status, "{body}");
        assert!(body.to_string().contains(expected), "{body}");
        assert!(!body.to_string().contains("credential="));
    }
    let composition = fixture
        .state
        .provider_runtime
        .managed_composition()
        .unwrap();
    let registrations = fixture
        .publisher
        .current_state()
        .unwrap()
        .console_surface_registry
        .managed_services()
        .to_vec();
    let mut mismatch = registrations.clone();
    mismatch[0].installation_id = Uuid::now_v7();
    fixture.publisher.publish(
        fixture
            .publisher
            .prepare(mismatch, composition.current_snapshots().await)
            .await
            .unwrap(),
    );
    let (status, body) = response(fixture.start("mutate", json!({"callback":false}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "managed_service_generation_mismatch");
    fixture.publisher.publish(
        fixture
            .publisher
            .prepare(registrations, composition.current_snapshots().await)
            .await
            .unwrap(),
    );
    let snapshot = composition.snapshot(domain::SYSTEM_SCOPE_ID).await.unwrap();
    let handle = &snapshot.bindings[&plugin_framework::extension_bus::ContributionId::new(
        "acme.composition-a.mutate",
    )
    .unwrap()]
        .handle;
    fixture
        .host
        .drain_managed_contributions(&[handle.clone()])
        .await
        .unwrap()
        .wait_drained()
        .await
        .unwrap();
    let (status, _) = response(fixture.start("mutate", json!({"callback":false}))).await;
    assert!(status.is_server_error());
    fixture.close().await;
}
