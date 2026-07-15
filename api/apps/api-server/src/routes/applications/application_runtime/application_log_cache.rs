use control_plane::ports::CacheStore;
use serde::{de::DeserializeOwned, Serialize};
use time::Duration;
use tracing::warn;
use uuid::Uuid;

use super::{
    ApplicationRunsQuery, FlowRunSummaryPageResponse, APPLICATION_RUN_LOG_DEFAULT_TIME_RANGE_DAYS,
};

fn time_range_segment(query: &ApplicationRunsQuery) -> String {
    let days = query
        .time_range_days
        .filter(|days| *days > 0)
        .unwrap_or(APPLICATION_RUN_LOG_DEFAULT_TIME_RANGE_DAYS);

    format!("days-{days}")
}

pub(super) fn summary_page_cache_key(
    workspace_id: Uuid,
    application_id: Uuid,
    query: &ApplicationRunsQuery,
    page: i64,
    page_size: i64,
    sort_by: &str,
    sort_order: &str,
) -> String {
    format!(
        "application-logs:summary-page:v2:workspace:{workspace_id}:application:{application_id}:range:{}:sort:{sort_by}:{sort_order}:page:{page}:size:{page_size}",
        time_range_segment(query)
    )
}

pub(super) fn summary_page_cache_ttl(page: i64) -> Duration {
    if page <= 1 {
        Duration::minutes(5)
    } else {
        Duration::minutes(15)
    }
}

fn is_terminal_status_str(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "cancelled")
}

pub(super) fn summary_page_cacheable(response: &FlowRunSummaryPageResponse) -> bool {
    response
        .items
        .iter()
        .all(|item| is_terminal_status_str(&item.status))
}

pub(super) async fn read<T>(cache: &dyn CacheStore, key: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    let cached = match cache.get_json(key).await {
        Ok(cached) => cached,
        Err(error) => {
            warn!(cache_key = key, error = %error, "failed to read application log cache");
            return None;
        }
    };

    let value = cached?;

    match serde_json::from_value(value) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(cache_key = key, error = %error, "failed to decode application log cache");
            None
        }
    }
}

pub(super) async fn write<T>(cache: &dyn CacheStore, key: &str, value: &T, ttl: Duration)
where
    T: Serialize,
{
    let value = match serde_json::to_value(value) {
        Ok(value) => value,
        Err(error) => {
            warn!(cache_key = key, error = %error, "failed to encode application log cache");
            return;
        }
    };

    if let Err(error) = cache.set_json(key, value, Some(ttl)).await {
        warn!(cache_key = key, error = %error, "failed to write application log cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_page_cache_ttl_keeps_first_page_fresher_than_older_pages() {
        assert_eq!(summary_page_cache_ttl(1), Duration::minutes(5));
        assert_eq!(summary_page_cache_ttl(2), Duration::minutes(15));
    }
}
