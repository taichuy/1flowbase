use super::{
    coordination::{Activity, Counts, Waiting},
    *,
};
use sqlx::postgres::PgConnectOptions;
use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{watch, OwnedSemaphorePermit, Semaphore},
    time::{timeout_at, Instant},
};

pub(super) struct TemporaryFactory {
    options: Arc<PgConnectOptions>,
    limit: Option<u32>,
    quota: Option<Arc<Semaphore>>,
    counts: Arc<Counts>,
    closed: AtomicBool,
}

impl TemporaryFactory {
    pub(super) fn new(business: &PgPool, settings: &PgConnectionSettings) -> Self {
        Self {
            options: business.connect_options(),
            limit: settings.temporary_max_connections,
            quota: settings
                .temporary_max_connections
                .map(|limit| Arc::new(Semaphore::new(limit as usize))),
            counts: Arc::default(),
            closed: AtomicBool::new(false),
        }
    }

    pub(super) fn observe(&self) -> PgTemporaryObservation {
        PgTemporaryObservation {
            max_connections: self.limit,
            active: self.counts.active.load(Ordering::SeqCst),
            closing: self.counts.closing.load(Ordering::SeqCst),
            waiting: self.counts.waiting.load(Ordering::SeqCst),
        }
    }

    pub(super) async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(quota) = &self.quota {
            quota.close();
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

impl PgConnectionRegistry {
    /// Reserve one temporary physical session BEFORE creating its max=1 pool.
    /// Caller hooks own transaction setup/depth checks; this owner controls quota and drain.
    pub async fn temporary_pool(
        &self,
        options: PgPoolOptions,
        deadline: Instant,
    ) -> Result<PgTemporaryPool> {
        let factory = &self.inner.temporary;
        anyhow::ensure!(
            deadline > Instant::now(),
            "postgres temporary deadline exceeded"
        );
        anyhow::ensure!(
            !factory.closed.load(Ordering::SeqCst),
            "postgres temporary owner closed"
        );
        let waiting = Waiting::new(&factory.counts);
        let permit = match &factory.quota {
            Some(quota) => Some(
                timeout_at(deadline, quota.clone().acquire_owned())
                    .await
                    .map_err(|_| anyhow::anyhow!("postgres temporary quota deadline exceeded"))?
                    .map_err(|_| anyhow::anyhow!("postgres temporary owner closed"))?,
            ),
            None => None,
        };
        drop(waiting);
        // Register activity before checking shutdown: either drain observes it, or the
        // check rejects admission. There is no await before the guarded pool exists.
        let activity = Activity::new(&factory.counts);
        anyhow::ensure!(
            !factory.closed.load(Ordering::SeqCst),
            "postgres temporary owner closed"
        );
        let pool = options
            .max_connections(1)
            .min_connections(0)
            .connect_lazy_with(
                (*factory.options)
                    .clone()
                    .application_name("1flowbase:temporary"),
            );
        let (closed, receiver) = watch::channel(false);
        let owner = PgTemporaryPool {
            owned: Some(TemporaryOwned {
                pool,
                _permit: permit,
                activity,
                closed,
            }),
            closed: receiver,
            runtime: tokio::runtime::Handle::current(),
        };
        // Owner already exists: cancellation during connect/after_connect still drains the
        // pool before releasing quota. Use the actual caller hooks on fresh connections.
        let connection = timeout_at(deadline, owner.pool().acquire())
            .await
            .map_err(|_| anyhow::anyhow!("postgres temporary acquire deadline exceeded"))??;
        drop(connection);
        Ok(owner)
    }
}

/// The owner must outlive all repository clones using pool(). Closing waits for their
/// native checkouts; cancellation of close transfers cleanup rather than releasing quota.
pub struct PgTemporaryPool {
    owned: Option<TemporaryOwned>,
    closed: watch::Receiver<bool>,
    runtime: tokio::runtime::Handle,
}

struct TemporaryOwned {
    pool: PgPool,
    _permit: Option<OwnedSemaphorePermit>,
    activity: Activity,
    closed: watch::Sender<bool>,
}

impl fmt::Debug for PgTemporaryPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgTemporaryPool")
            .field("closed", &*self.closed.borrow())
            .finish_non_exhaustive()
    }
}

impl PgTemporaryPool {
    pub fn pool(&self) -> &PgPool {
        &self
            .owned
            .as_ref()
            .expect("temporary owner exposes pool only before close")
            .pool
    }

    pub async fn close(mut self) {
        self.start_close();
        loop {
            let closed = *self.closed.borrow_and_update();
            if closed {
                break;
            }
            if self.closed.changed().await.is_err() {
                break;
            }
        }
    }

    fn start_close(&mut self) {
        if let Some(mut owned) = self.owned.take() {
            owned.activity.begin_close();
            self.runtime.spawn(async move {
                let closed = owned.closed.clone();
                owned.pool.close().await;
                // Capture the whole owner, including its permit, through the native drain.
                drop(owned);
                closed.send_replace(true);
            });
        }
    }
}

impl Drop for PgTemporaryPool {
    fn drop(&mut self) {
        self.start_close();
    }
}
