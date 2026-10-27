use super::*;
use sqlx::{pool::PoolConnection, Connection, Postgres};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};
use tokio::{
    sync::{oneshot, watch, Mutex as AsyncMutex, Notify, OwnedMutexGuard},
    time::{timeout, timeout_at, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PgCoordinationLane {
    ManagedMutation,
    Dispatch,
    TemplateInstall,
}

impl PgCoordinationLane {
    fn label(self) -> &'static str {
        match self {
            Self::ManagedMutation => "managed",
            Self::Dispatch => "dispatch",
            Self::TemplateInstall => "template-install",
        }
    }
}

/// The existing PostgreSQL key text and hash seed, shared by session and legacy xact locks.
/// Debug output deliberately omits the key. Never use keys as metrics labels.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PgAdvisoryLockKey {
    lane: PgCoordinationLane,
    text: String,
    seed: i64,
}

impl fmt::Debug for PgAdvisoryLockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgAdvisoryLockKey")
            .field("lane", &self.lane)
            .finish_non_exhaustive()
    }
}

impl PgAdvisoryLockKey {
    pub fn managed_mutation(installation_id: Uuid, scope_id: Uuid) -> Self {
        Self {
            lane: PgCoordinationLane::ManagedMutation,
            text: format!("managed-service-mutation:{installation_id}:{scope_id}"),
            seed: 0,
        }
    }

    pub fn dispatch(application_id: Uuid, run_id: Uuid) -> Self {
        Self {
            lane: PgCoordinationLane::Dispatch,
            text: format!("flow-dispatch:{application_id}:{run_id}"),
            seed: 2012,
        }
    }

    pub fn template_install(workspace_id: Uuid) -> Self {
        Self {
            lane: PgCoordinationLane::TemplateInstall,
            text: format!("application-template:{workspace_id}"),
            seed: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PgSessionStatus {
    /// Ownership is uncertain; cancel local work and never automatically replay it.
    pub lost: bool,
    /// Connection cleanup AND release of the last retained local owner have completed.
    pub closed: bool,
}

/// Clone this in process before Host admission and retain it through callbacks/reaping.
/// The last clone requests unlock. Caller timeout alone must not drop all retained owners.
#[derive(Clone)]
pub struct PgCoordinationLease {
    lifetime: Arc<LeaseLifetime>,
}

struct LeaseLifetime {
    _release: oneshot::Sender<()>,
    status: watch::Receiver<PgSessionStatus>,
}

impl fmt::Debug for PgCoordinationLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgCoordinationLease")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl PgCoordinationLease {
    pub fn status(&self) -> PgSessionStatus {
        let mut status = *self.lifetime.status.borrow();
        if !status.closed && self.lifetime.status.has_changed().is_err() {
            status.lost = true;
        }
        status
    }

    /// The receiver is observational: retaining it does not retain the lock.
    /// Unexpected sender closure without closed=true must be treated as ownership loss.
    pub fn subscribe(&self) -> watch::Receiver<PgSessionStatus> {
        self.lifetime.status.clone()
    }

    /// Wait for loss or orderly completion; the returned status distinguishes them.
    /// This borrows a retained lease. Observers that must not retain ownership use subscribe().
    pub async fn wait_for_loss(&self) -> PgSessionStatus {
        let mut receiver = self.subscribe();
        loop {
            let status = self.status();
            if status.lost || status.closed {
                return status;
            }
            if receiver.changed().await.is_err() {
                return self.status();
            }
        }
    }

    /// Drop this clone and wait until every retained clone and its connection have drained.
    pub async fn release(self) -> PgSessionStatus {
        let mut status = self.subscribe();
        drop(self);
        loop {
            let current = *status.borrow_and_update();
            if current.closed {
                return current;
            }
            if status.changed().await.is_err() {
                return PgSessionStatus {
                    lost: true,
                    closed: false,
                };
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Counts {
    pub(super) active: AtomicUsize,
    pub(super) closing: AtomicUsize,
    pub(super) waiting: AtomicUsize,
    pub(super) drained: Notify,
}

pub(super) struct Waiting(Arc<Counts>);

impl Waiting {
    pub(super) fn new(counts: &Arc<Counts>) -> Self {
        counts.waiting.fetch_add(1, Ordering::SeqCst);
        Self(counts.clone())
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.0.waiting.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(super) struct Activity {
    counts: Arc<Counts>,
    closing: bool,
}

impl Activity {
    pub(super) fn new(counts: &Arc<Counts>) -> Self {
        counts.active.fetch_add(1, Ordering::SeqCst);
        Self {
            counts: counts.clone(),
            closing: false,
        }
    }

    pub(super) fn begin_close(&mut self) {
        if !self.closing {
            self.counts.closing.fetch_add(1, Ordering::SeqCst);
            self.counts.active.fetch_sub(1, Ordering::SeqCst);
            self.closing = true;
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        let counter = if self.closing {
            &self.counts.closing
        } else {
            &self.counts.active
        };
        counter.fetch_sub(1, Ordering::SeqCst);
        self.counts.drained.notify_waiters();
    }
}

struct Lane {
    pool: Option<PgPool>,
    options: Arc<sqlx::postgres::PgConnectOptions>,
    limit: Option<u32>,
    counts: Arc<Counts>,
    label: &'static str,
    shutdown: watch::Sender<bool>,
}

impl Lane {
    fn new(
        business: &PgPool,
        settings: &PgPoolSettings,
        lane: PgCoordinationLane,
        limit: Option<u32>,
    ) -> Self {
        let mut settings = settings.clone();
        // SQLx 0.9 preallocates its idle ArrayQueue at max_connections. An enormous
        // synthetic cap is neither an uncapped factory nor a safe memory allocation.
        // Compatibility lanes use directly owned sessions, never business detachment.
        if let Some(limit) = limit {
            settings.max_connections = limit;
        }
        let options = (*business.connect_options())
            .clone()
            .application_name(match lane {
                PgCoordinationLane::ManagedMutation => "1flowbase:managed-lock",
                PgCoordinationLane::Dispatch => "1flowbase:dispatch-lock",
                PgCoordinationLane::TemplateInstall => "1flowbase:template-lock",
            });
        let pool = limit.map(|_| settings.options().connect_lazy_with(options.clone()));
        let (shutdown, _) = watch::channel(false);
        Self {
            pool,
            options: Arc::new(options),
            limit,
            counts: Arc::default(),
            label: lane.label(),
            shutdown,
        }
    }

    fn observe(&self) -> PgLaneObservation {
        let active = self.counts.active.load(Ordering::SeqCst);
        let closing = self.counts.closing.load(Ordering::SeqCst);
        let pool = match &self.pool {
            Some(pool) => observe_pool(pool, self.limit),
            None => PgPoolObservation {
                max_connections: None,
                size: (active + closing).try_into().unwrap_or(u32::MAX),
                idle: 0,
                checked_out: (active + closing).try_into().unwrap_or(u32::MAX),
            },
        };
        PgLaneObservation {
            pool,
            active,
            closing,
            local_queue: self.counts.waiting.load(Ordering::SeqCst),
        }
    }

    async fn close(&self) {
        self.shutdown.send_replace(true);
        if let Some(pool) = &self.pool {
            pool.close().await;
        }
        loop {
            let drained = self.counts.drained.notified();
            if self.counts.active.load(Ordering::SeqCst) == 0
                && self.counts.closing.load(Ordering::SeqCst) == 0
            {
                break;
            }
            drained.await;
        }
    }
}

#[derive(Default)]
struct KeyRegistry {
    cells: Mutex<HashMap<PgAdvisoryLockKey, Weak<KeyCell>>>,
}

struct KeyCell {
    mutex: Arc<AsyncMutex<()>>,
    key: PgAdvisoryLockKey,
    registry: Weak<KeyRegistry>,
}

impl KeyRegistry {
    fn cell(self: &Arc<Self>, key: &PgAdvisoryLockKey) -> Arc<KeyCell> {
        let mut cells = self
            .cells
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(cell) = cells.get(key).and_then(Weak::upgrade) {
            return cell;
        }
        let cell = Arc::new(KeyCell {
            mutex: Arc::new(AsyncMutex::new(())),
            key: key.clone(),
            registry: Arc::downgrade(self),
        });
        cells.insert(key.clone(), Arc::downgrade(&cell));
        cell
    }

    fn len(&self) -> usize {
        self.cells
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .len()
    }
}

impl Drop for KeyCell {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            let mut cells = registry
                .cells
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            // A new cell may have replaced our expired weak reference while Drop waited.
            if cells
                .get(&self.key)
                .is_some_and(|cell| std::ptr::eq(cell.as_ptr(), self))
            {
                cells.remove(&self.key);
            }
        }
    }
}

struct LocalTurn {
    guard: Option<OwnedMutexGuard<()>>,
    _cell: Arc<KeyCell>,
}

impl Drop for LocalTurn {
    fn drop(&mut self) {
        // The map entry can disappear only after its mutex ownership has ended.
        drop(self.guard.take());
    }
}

pub(super) struct RegistryInner {
    pub(super) settings: PgConnectionSettings,
    business: PgPool,
    managed: Arc<Lane>,
    dispatch: Arc<Lane>,
    template_install: Arc<Lane>,
    keys: Arc<KeyRegistry>,
    pub(super) temporary: Arc<temporary::TemporaryFactory>,
    closed: AtomicBool,
}

impl RegistryInner {
    pub(super) fn new(business: PgPool, settings: PgConnectionSettings) -> Self {
        Self {
            managed: Arc::new(Lane::new(
                &business,
                &settings.business,
                PgCoordinationLane::ManagedMutation,
                Some(settings.managed_limit()),
            )),
            dispatch: Arc::new(Lane::new(
                &business,
                &settings.business,
                PgCoordinationLane::Dispatch,
                settings.dispatch_max_connections,
            )),
            template_install: Arc::new(Lane::new(
                &business,
                &settings.business,
                PgCoordinationLane::TemplateInstall,
                settings.template_install_max_connections,
            )),
            temporary: Arc::new(temporary::TemporaryFactory::new(&business, &settings)),
            business,
            settings,
            keys: Arc::default(),
            closed: AtomicBool::new(false),
        }
    }

    fn lane(&self, lane: PgCoordinationLane) -> &Arc<Lane> {
        match lane {
            PgCoordinationLane::ManagedMutation => &self.managed,
            PgCoordinationLane::Dispatch => &self.dispatch,
            PgCoordinationLane::TemplateInstall => &self.template_install,
        }
    }

    pub(super) fn observe(&self) -> PgConnectionObservation {
        PgConnectionObservation {
            business: observe_pool(&self.business, Some(self.settings.business.max_connections)),
            managed: self.managed.observe(),
            dispatch: self.dispatch.observe(),
            template_install: self.template_install.observe(),
            temporary: self.temporary.observe(),
            local_key_cells: self.keys.len(),
            instance_budget: self.settings.instance_budget.clone(),
        }
    }

    pub(super) async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        tokio::join!(
            self.managed.close(),
            self.dispatch.close(),
            self.template_install.close(),
            self.temporary.close()
        );
        self.business.close().await;
    }
}

impl PgConnectionRegistry {
    /// FIFO among polled, noncancelled local callers of the same key. One absolute deadline
    /// covers the local queue, native pool admission and PostgreSQL lock acquisition.
    pub async fn acquire(
        &self,
        key: PgAdvisoryLockKey,
        deadline: Instant,
    ) -> Result<PgCoordinationLease> {
        let result = self.acquire_inner(key, deadline, false).await?;
        result.ok_or_else(|| anyhow::anyhow!("postgres blocking lock was not acquired"))
    }

    /// Only a negative PG try-lock ACK returns None. Budget wait/failure is an error.
    /// Try-lock probes intentionally do not wait behind a local long-running holder.
    pub async fn try_acquire(
        &self,
        key: PgAdvisoryLockKey,
        deadline: Instant,
    ) -> Result<Option<PgCoordinationLease>> {
        anyhow::ensure!(
            key.lane == PgCoordinationLane::Dispatch,
            "postgres try-lock requires dispatch lane"
        );
        self.acquire_inner(key, deadline, true).await
    }

    async fn acquire_inner(
        &self,
        key: PgAdvisoryLockKey,
        deadline: Instant,
        try_only: bool,
    ) -> Result<Option<PgCoordinationLease>> {
        anyhow::ensure!(
            deadline > Instant::now(),
            "postgres coordination deadline exceeded"
        );
        anyhow::ensure!(
            !self.inner.closed.load(Ordering::SeqCst),
            "postgres connection owner closed"
        );
        let started = Instant::now();
        let lane = self.inner.lane(key.lane).clone();
        let local = if try_only {
            None
        } else {
            let cell = self.inner.keys.cell(&key);
            let waiting = Waiting::new(&lane.counts);
            let guard = timeout_at(deadline, cell.mutex.clone().lock_owned())
                .await
                .map_err(|_| anyhow::anyhow!("postgres local queue deadline exceeded"))?;
            drop(waiting);
            Some(LocalTurn {
                guard: Some(guard),
                _cell: cell,
            })
        };
        let (connection, activity) = match &lane.pool {
            Some(pool) => {
                let connection = timeout_at(deadline, pool.acquire())
                    .await
                    .map_err(|_| anyhow::anyhow!("postgres lane acquire deadline exceeded"))??;
                (
                    OwnedConnection::Pooled(connection),
                    Activity::new(&lane.counts),
                )
            }
            None => {
                // Count establishment as well as held sessions, like native pool size.
                let activity = Activity::new(&lane.counts);
                anyhow::ensure!(!*lane.shutdown.borrow(), "postgres connection owner closed");
                let connection =
                    timeout_at(deadline, sqlx::PgConnection::connect_with(&lane.options))
                        .await
                        .map_err(|_| {
                            anyhow::anyhow!("postgres lane acquire deadline exceeded")
                        })??;
                (OwnedConnection::Direct(connection), activity)
            }
        };
        // Construct the cancellation guard BEFORE any possibly locking statement.
        let mut session = Session {
            connection: Some(connection),
            local,
            activity: Some(activity),
            lane: lane.clone(),
            runtime: tokio::runtime::Handle::current(),
        };
        anyhow::ensure!(
            deadline > Instant::now(),
            "postgres coordination deadline exceeded"
        );
        let query = if try_only {
            "SELECT pg_try_advisory_lock(hashtextextended($1, $2))"
        } else {
            // Both functions return one bool row while the blocking function itself returns void.
            "SELECT true FROM pg_advisory_lock(hashtextextended($1, $2))"
        };
        let acquired: bool = timeout_at(
            deadline,
            sqlx::query_scalar(query)
                .bind(&key.text)
                .bind(key.seed)
                .fetch_one(session.connection()),
        )
        .await
        .map_err(|_| anyhow::anyhow!("postgres lock acquire deadline exceeded"))??;
        tracing::debug!(
            lane = lane.label,
            elapsed_ms = started.elapsed().as_millis() as u64,
            acquired,
            "postgres coordination admission"
        );
        if !acquired {
            session.return_clean().await;
            return Ok(None);
        }
        let (release, released) = oneshot::channel();
        let (status, receiver) = watch::channel(PgSessionStatus::default());
        let interval = self.inner.settings.health_check_interval;
        let io_timeout = self.inner.settings.business.acquire_timeout;
        tokio::spawn(hold_session(
            session, key, released, status, interval, io_timeout,
        ));
        Ok(Some(PgCoordinationLease {
            lifetime: Arc::new(LeaseLifetime {
                _release: release,
                status: receiver,
            }),
        }))
    }
}

/// Dirty until a negative try-lock or positive unlock ACK proves there is no session lock.
enum OwnedConnection {
    Pooled(PoolConnection<Postgres>),
    Direct(sqlx::PgConnection),
}

impl OwnedConnection {
    fn connection(&mut self) -> &mut sqlx::PgConnection {
        match self {
            Self::Pooled(connection) => connection,
            Self::Direct(connection) => connection,
        }
    }

    fn discard_on_drop(&mut self) {
        if let Self::Pooled(connection) = self {
            connection.close_on_drop();
        }
    }

    async fn close(self) {
        match self {
            Self::Pooled(connection) => {
                let _ = connection.close().await;
            }
            Self::Direct(connection) => {
                let _ = connection.close().await;
            }
        }
    }
}

struct Session {
    connection: Option<OwnedConnection>,
    local: Option<LocalTurn>,
    activity: Option<Activity>,
    lane: Arc<Lane>,
    runtime: tokio::runtime::Handle,
}

impl Session {
    fn connection(&mut self) -> &mut sqlx::PgConnection {
        self.connection
            .as_mut()
            .expect("session owns its connection until cleanup")
            .connection()
    }

    async fn return_clean(mut self) {
        if matches!(self.connection, Some(OwnedConnection::Direct(_))) {
            self.close().await;
        } else {
            // SQLx performs its normal ping/recycle. Only a clean ACK makes reuse safe.
            drop(self.connection.take());
        }
    }

    async fn close(mut self) {
        if let Some(activity) = &mut self.activity {
            activity.begin_close();
        }
        if let Some(mut connection) = self.connection.take() {
            connection.discard_on_drop();
            // SQLx 0.9 PoolConnection::close retains its size/permit while closing.
            // A stalled close is dropped at the same bound as SQLx close_on_drop;
            // dropping the close future destroys the raw socket before our accounting ends.
            let _ = timeout(Duration::from_secs(5), connection.close()).await;
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            connection.discard_on_drop();
            let local = self.local.take();
            let mut activity = self.activity.take();
            if let Some(activity) = &mut activity {
                activity.begin_close();
            }
            let lane = self.lane.label;
            // The cleanup future owns the native checkout, local turn and accounting.
            // Caller cancellation cannot release any of them ahead of socket cleanup.
            self.runtime.spawn(async move {
                let _ = timeout(Duration::from_secs(5), connection.close()).await;
                drop(local);
                drop(activity);
                tracing::debug!(lane, "postgres uncertain session closed");
            });
        }
    }
}

async fn hold_session(
    mut session: Session,
    key: PgAdvisoryLockKey,
    mut released: oneshot::Receiver<()>,
    status: watch::Sender<PgSessionStatus>,
    health_interval: Duration,
    io_timeout: Duration,
) {
    let mut ticker = tokio::time::interval_at(Instant::now() + health_interval, health_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut shutdown = session.lane.shutdown.subscribe();
    let lost = loop {
        if *shutdown.borrow() {
            break true;
        }
        tokio::select! {
            biased;
            _ = &mut released => break false,
            _ = shutdown.changed() => break true,
            _ = ticker.tick() => {
                if !matches!(timeout(io_timeout, session.connection().ping()).await, Ok(Ok(()))) {
                    break true;
                }
            }
        }
    };
    if lost {
        status.send_replace(PgSessionStatus {
            lost: true,
            closed: false,
        });
        // Revoke the broken physical session immediately, retaining the local turn until
        // the Host's final clone is gone. Another PG node may already own the lock.
        let local = session.local.take();
        session.close().await;
        let _ = released.await;
        drop(local);
        status.send_replace(PgSessionStatus {
            lost: true,
            closed: true,
        });
        return;
    }
    let unlocked = timeout(
        io_timeout,
        sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock(hashtextextended($1, $2))")
            .bind(&key.text)
            .bind(key.seed)
            .fetch_one(session.connection()),
    )
    .await;
    let lost = !matches!(unlocked, Ok(Ok(true)));
    if lost {
        status.send_replace(PgSessionStatus {
            lost: true,
            closed: false,
        });
        session.close().await;
    } else {
        session.return_clean().await;
    }
    status.send_replace(PgSessionStatus { lost, closed: true });
}
