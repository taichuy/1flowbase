use super::*;
use std::process::{Child, Command, Stdio};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn observation() -> (u64, u64) {
    let stat = fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<_> = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    let ticks = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    let rss_kib = fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .map(|v| v.split_whitespace().next().unwrap().parse().unwrap())
        })
        .unwrap();
    (ticks, rss_kib)
}

// Real sampler + sysinfo + OS children; the old Some(primary PIDs) policy is
// exercised as a controlled negative rather than an abstract collection model.
#[test]
fn process_metadata_churn_releases_exited_commands_and_preserves_primary_observation() {
    for old_policy in [true, false] {
        let mut sampler = RuntimeMetricSampler::new();
        let mut dead = Vec::new();
        let mut samples = Vec::new();
        let mut retained_payload = Vec::new();
        let mut retained_dead = Vec::new();
        let (cpu_start, _) = observation();
        let started = Instant::now();
        for round in 0..12 {
            let payload = format!("gateway-metadata-churn-{round}-{}", "x".repeat(32_768));
            let child = OwnedChild(
                Command::new("sh")
                    .args(["-c", "read -r line", &payload])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("owned waiting child"),
            );
            let pid = sysinfo::Pid::from_u32(child.0.id());
            sampler.previous_sample_at = None;
            sampler.collect();
            let primary_before = sampler.process_snapshot();
            if old_policy {
                let pids: Vec<_> = sampler.system.processes().keys().copied().collect();
                sampler.metadata_system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&pids),
                    true,
                    ProcessRefreshKind::nothing()
                        .without_tasks()
                        .with_user(UpdateKind::OnlyIfNotSet)
                        .with_cmd(UpdateKind::OnlyIfNotSet),
                );
            } else {
                assert!(sampler.ensure_process_metadata());
                assert!(!sampler.ensure_process_metadata());
            }
            assert!(sampler
                .metadata_system
                .process(pid)
                .unwrap()
                .cmd()
                .iter()
                .any(|v| v == payload.as_str()));
            // Optional metadata must leave the primary System's observations intact.
            assert_eq!(
                primary_before.processes.len(),
                sampler.process_snapshot().processes.len()
            );
            drop(child);
            dead.push(pid);
            sampler.previous_sample_at = None;
            sampler.collect();
            if old_policy {
                let pids: Vec<_> = sampler.system.processes().keys().copied().collect();
                sampler.metadata_system.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&pids),
                    true,
                    ProcessRefreshKind::nothing()
                        .without_tasks()
                        .with_cmd(UpdateKind::OnlyIfNotSet),
                );
            } else {
                sampler.ensure_process_metadata();
            }
            let count = dead
                .iter()
                .filter(|pid| sampler.metadata_system.process(**pid).is_some())
                .count();
            let bytes: usize = dead
                .iter()
                .filter_map(|pid| sampler.metadata_system.process(*pid))
                .flat_map(|p| p.cmd())
                .map(|v| v.as_encoded_bytes().len())
                .sum();
            retained_dead.push(count);
            retained_payload.push(bytes);
            let (ticks, rss) = observation();
            samples.push((
                round,
                sampler.metadata_system.processes().len(),
                rss,
                ticks - cpu_start,
            ));
            assert_eq!(count, if old_policy { round + 1 } else { 0 });
            assert_eq!(bytes == 0, !old_policy);
        }
        println!("METADATA_CHURN old_policy={old_policy} samples_round_len_rssKiB_cpuTicks={samples:?} retained_dead={retained_dead:?} retained_cmd_bytes={retained_payload:?} wall_seconds={:.6} clock_ticks_per_second=host_getconf_CLK_TCK", started.elapsed().as_secs_f64());
    }
}
