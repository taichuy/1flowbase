use std::{collections::BTreeMap, time::Duration};

use anyhow::{bail, Context, Result};
use storage_ephemeral::ReadThroughCachePolicy;

pub(super) fn from_env(map: &BTreeMap<String, String>) -> Result<Option<ReadThroughCachePolicy>> {
    const BUDGET: &str = "API_DOCS_CACHE_MAX_WEIGHT_BYTES";
    const IDLE: &str = "API_DOCS_CACHE_IDLE_SECONDS";
    match (map.get(BUDGET), map.get(IDLE)) {
        (None, None) => Ok(None),
        (Some(budget), Some(idle)) => {
            let budget = budget
                .trim()
                .parse::<u64>()
                .with_context(|| format!("invalid {BUDGET}"))?;
            let idle = idle
                .trim()
                .parse::<u64>()
                .with_context(|| format!("invalid {IDLE}"))?;
            ReadThroughCachePolicy::new(budget, Duration::from_secs(idle))
                .context("invalid API_DOCS_CACHE policy")
                .map(Some)
        }
        _ => bail!("{BUDGET} and {IDLE} must be configured together"),
    }
}

#[cfg(test)]
#[path = "../_tests/host_boot/docs_cache_config_tests.rs"]
mod tests;
