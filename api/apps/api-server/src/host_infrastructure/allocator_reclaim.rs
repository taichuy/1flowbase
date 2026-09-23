//! Return free glibc arena pages after large, temporary long-conversation buffers.
//! `MALLOC_ARENA_MAX` bounds arena count but does not return idle pages in them.

use std::{fs, time::Duration};

use tokio::{task::JoinHandle, time::MissedTickBehavior};

const RSS_TRIGGER_KIB: u64 = 1024 * 1024;
const RECLAIM_INTERVAL: Duration = Duration::from_secs(60);

unsafe extern "C" {
    fn malloc_trim(pad: usize) -> std::ffi::c_int;
}

pub fn spawn_allocator_reclaimer() -> JoinHandle<()> {
    tokio::spawn(async {
        let mut tick = tokio::time::interval_at(
            tokio::time::Instant::now() + RECLAIM_INTERVAL,
            RECLAIM_INTERVAL,
        );
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if let Err(error) = tokio::task::spawn_blocking(reclaim_if_needed).await {
                tracing::warn!(%error, "allocator reclaim worker failed");
            }
        }
    })
}

fn reclaim_if_needed() {
    let Some(before_kib) = rss_kib() else {
        return;
    };
    if before_kib < RSS_TRIGGER_KIB {
        return;
    }
    // glibc synchronizes its arenas; this only releases pages already free.
    let _ = unsafe { malloc_trim(0) };
    if let Some(after_kib) = rss_kib() {
        if after_kib < before_kib {
            tracing::info!(before_kib, after_kib, "idle allocator pages reclaimed");
        }
    }
}

fn rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
