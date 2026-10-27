use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

mod coordination;
mod temporary;

pub use coordination::{
    PgAdvisoryLockKey, PgCoordinationLane, PgCoordinationLease, PgSessionStatus,
};
pub use temporary::PgTemporaryPool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgPoolSettings {
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl PgPoolSettings {
    pub fn with_max_connections(max_connections: u32) -> Self {
        Self {
            max_connections,
            acquire_timeout: Duration::from_secs(5),
            // Match SQLx defaults, while allowing an explicit None to disable recycling.
            idle_timeout: Some(Duration::from_secs(600)),
            max_lifetime: Some(Duration::from_secs(1800)),
        }
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.max_connections > 0,
            "postgres business limit must be positive"
        );
        anyhow::ensure!(
            !self.acquire_timeout.is_zero(),
            "postgres acquire timeout must be positive"
        );
        for duration in [
            Some(self.acquire_timeout),
            self.idle_timeout,
            self.max_lifetime,
        ]
        .into_iter()
        .flatten()
        {
            anyhow::ensure!(
                !duration.is_zero() && std::time::Instant::now().checked_add(duration).is_some(),
                "postgres pool duration must be positive and representable"
            );
        }
        Ok(())
    }

    pub(super) fn options(&self) -> PgPoolOptions {
        PgPoolOptions::new()
            .min_connections(0)
            .max_connections(self.max_connections)
            .acquire_timeout(self.acquire_timeout)
            .acquire_time_level("debug".parse().expect("static SQLx log level"))
            .idle_timeout(self.idle_timeout)
            .max_lifetime(self.max_lifetime)
    }
}

/// An allocation for this serving instance, including explicitly reserved other clients.
/// Other instances, rolling deployments and offline maintenance need their own allocations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgInstanceConnectionBudget {
    pub max_connections: u32,
    pub reserved_connections: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgConnectionSettings {
    pub business: PgPoolSettings,
    /// None inherits business P, preserving the old managed mutation resource scale.
    pub managed_max_connections: Option<u32>,
    /// None preserves the old detached/raw-session concurrency, with accounting but no new cap.
    pub dispatch_max_connections: Option<u32>,
    pub template_install_max_connections: Option<u32>,
    pub temporary_max_connections: Option<u32>,
    pub instance_budget: Option<PgInstanceConnectionBudget>,
    pub health_check_interval: Duration,
}

impl PgConnectionSettings {
    pub fn compatible(business: PgPoolSettings) -> Self {
        Self {
            business,
            managed_max_connections: None,
            dispatch_max_connections: None,
            template_install_max_connections: None,
            temporary_max_connections: None,
            instance_budget: None,
            health_check_interval: Duration::from_secs(1),
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.business.validate()?;
        anyhow::ensure!(
            !self.health_check_interval.is_zero(),
            "postgres health interval must be positive"
        );
        anyhow::ensure!(
            std::time::Instant::now()
                .checked_add(self.health_check_interval)
                .is_some(),
            "postgres health interval must be representable"
        );
        for limit in [
            self.managed_max_connections,
            self.dispatch_max_connections,
            self.template_install_max_connections,
            self.temporary_max_connections,
        ]
        .into_iter()
        .flatten()
        {
            anyhow::ensure!(limit > 0, "postgres lane limit must be positive");
        }
        // Even without a full instance allocation, reject overflow of known allocations.
        let mut known = self.business.max_connections;
        for limit in [
            Some(self.managed_limit()),
            self.dispatch_max_connections,
            self.template_install_max_connections,
            self.temporary_max_connections,
        ]
        .into_iter()
        .flatten()
        {
            known = known
                .checked_add(limit)
                .ok_or_else(|| anyhow::anyhow!("postgres connection budget overflow"))?;
        }
        if let Some(budget) = &self.instance_budget {
            anyhow::ensure!(
                budget.max_connections > 0,
                "postgres instance allocation must be positive"
            );
            anyhow::ensure!(self.managed_max_connections.is_some() && self.dispatch_max_connections.is_some() && self.template_install_max_connections.is_some() && self.temporary_max_connections.is_some(), "postgres full budget requires explicit managed, dispatch, template-install and temporary allocations");
            let total = known
                .checked_add(budget.reserved_connections)
                .ok_or_else(|| anyhow::anyhow!("postgres connection budget overflow"))?;
            anyhow::ensure!(
                total <= budget.max_connections,
                "postgres connection allocations exceed instance budget"
            );
        }
        Ok(())
    }

    pub fn managed_limit(&self) -> u32 {
        self.managed_max_connections
            .unwrap_or(self.business.max_connections)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgPoolObservation {
    /// None means compatibility mode, not zero allocation or a deployment-wide bound.
    pub max_connections: Option<u32>,
    /// Local SQLx/client ownership, including establishment. Discarded remote backends
    /// can outlive this count; use pg_stat_activity/pg_locks for actual server observations.
    pub size: u32,
    pub idle: usize,
    pub checked_out: u32,
}

fn observe_pool(pool: &PgPool, max_connections: Option<u32>) -> PgPoolObservation {
    let size = pool.size();
    let idle = pool.num_idle();
    PgPoolObservation {
        max_connections,
        size,
        idle,
        checked_out: size.saturating_sub(idle as u32),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgLaneObservation {
    /// Uncapped lanes report owned session establishment/close here and have no idle pool.
    pub pool: PgPoolObservation,
    /// Locally owned acquisition attempts or held sessions, not remote PG backend counts.
    pub active: usize,
    /// Local socket cleanup; its completion is not a server termination acknowledgement.
    pub closing: usize,
    pub local_queue: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgTemporaryObservation {
    pub max_connections: Option<u32>,
    /// Pool owners/reservations, including establishment; not a count of successful queries.
    pub active: usize,
    pub closing: usize,
    pub waiting: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgConnectionObservation {
    pub business: PgPoolObservation,
    pub managed: PgLaneObservation,
    pub dispatch: PgLaneObservation,
    pub template_install: PgLaneObservation,
    pub temporary: PgTemporaryObservation,
    pub local_key_cells: usize,
    /// Validated local allocation; not an absolute server backend cap during network
    /// partitions or delayed reclamation of permanently discarded client sessions.
    pub instance_budget: Option<PgInstanceConnectionBudget>,
}

/// One process-local owner per store family. Clones and scoped stores share its factories.
#[derive(Clone)]
pub struct PgConnectionRegistry {
    inner: std::sync::Arc<coordination::RegistryInner>,
}

impl PgConnectionRegistry {
    pub fn new(business: PgPool, settings: PgConnectionSettings) -> Result<Self> {
        settings.validate()?;
        anyhow::ensure!(
            business.options().get_max_connections() == settings.business.max_connections,
            "postgres business pool does not match connection allocation"
        );
        Ok(Self {
            inner: std::sync::Arc::new(coordination::RegistryInner::new(business, settings)),
        })
    }

    pub(super) fn from_pool(pool: PgPool) -> Self {
        let options = pool.options();
        let settings = PgConnectionSettings::compatible(PgPoolSettings {
            max_connections: options.get_max_connections(),
            acquire_timeout: options.get_acquire_timeout(),
            idle_timeout: options.get_idle_timeout(),
            max_lifetime: options.get_max_lifetime(),
        });
        Self {
            inner: std::sync::Arc::new(coordination::RegistryInner::new(pool, settings)),
        }
    }

    pub fn settings(&self) -> &PgConnectionSettings {
        &self.inner.settings
    }

    /// Snapshots can race with SQLx maintenance; checked_out includes native acquisition/return.
    pub fn observe(&self) -> PgConnectionObservation {
        self.inner.observe()
    }

    /// Stop admission and drain all this owner's dedicated resources, then the business pool.
    /// Retained Host leases and temporary owners must first finish or be released by their owner.
    pub async fn close(&self) {
        self.inner.close().await;
    }
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    connect_with_max_connections(database_url, 5).await
}

pub async fn connect_with_max_connections(
    database_url: &str,
    max_connections: u32,
) -> Result<PgPool> {
    connect_with_pool_settings(
        database_url,
        PgPoolSettings::with_max_connections(max_connections),
    )
    .await
}

pub async fn connect_with_pool_settings(
    database_url: &str,
    settings: PgPoolSettings,
) -> Result<PgPool> {
    settings.validate()?;
    let pool = settings.options().connect(database_url).await?;

    Ok(pool)
}
