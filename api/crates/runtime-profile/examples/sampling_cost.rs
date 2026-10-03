//! Measure the real sampler, including warm-up, without starting an API server.
//! Run the built binary under `/usr/bin/time` to include all worker-thread CPU.
//! Arguments: [metrics|mixed|idle] [samples=10] [interval_ms=2000].
use runtime_profile::{RuntimeProcessSampler, RuntimeSampleSource};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("mixed");
    anyhow::ensure!(matches!(mode, "metrics" | "mixed" | "idle"), "unknown mode");
    let samples: usize = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(10);
    let interval_ms: u64 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(2000);
    anyhow::ensure!(
        samples > 0 && interval_ms >= 1000,
        "need samples > 0 and interval >= 1000 ms"
    );
    sysinfo::set_open_files_limit(128);
    let source = Arc::new(RuntimeSampleSource::new(Duration::from_secs(1)));
    let processes = RuntimeProcessSampler::with_sample_source(source.clone());
    let mut elapsed = Vec::with_capacity(samples);
    let mut process_count = 0;
    let started = Instant::now();
    for index in 0..samples {
        let at = Instant::now();
        if mode != "idle" {
            let snapshot = source.collect()?;
            assert!(snapshot.metrics.memory.process_bytes > 0);
            if index > 0 {
                assert!(snapshot.metrics.cpu.usage_percent.is_some());
            }
            if mode == "mixed" {
                let snapshot = processes.collect();
                process_count = snapshot.total;
                let own = snapshot
                    .processes
                    .iter()
                    .find(|p| p.pid == std::process::id());
                if let Some(own) = own {
                    assert!(own.command.is_some() && !own.terminable);
                } else {
                    // The API intentionally returns only the 256 largest
                    // processes; a small standalone probe may fall below them.
                    assert!(snapshot.total > snapshot.processes.len());
                }
            }
        }
        elapsed.push(at.elapsed().as_secs_f64() * 1000.0);
        thread::sleep(Duration::from_millis(interval_ms).saturating_sub(at.elapsed()));
    }
    let first_ms = elapsed.remove(0);
    elapsed.sort_by(f64::total_cmp);
    let median_ms = elapsed.get(elapsed.len() / 2).copied().unwrap_or(first_ms);
    let max_ms = elapsed.last().copied().unwrap_or(first_ms);
    println!("mode={mode} samples={samples} interval_ms={interval_ms} processes={process_count} first_ms={first_ms:.3} steady_median_ms={median_ms:.3} steady_max_ms={max_ms:.3} elapsed_s={:.3}", started.elapsed().as_secs_f64());
    Ok(())
}
