use std::sync::Mutex;

use anyhow::{anyhow, Result};
use plugin_framework::RuntimeTarget;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use time::OffsetDateTime;

use crate::{detect_host_fingerprint, RuntimeMetricSampler, RuntimeMetricsSnapshot};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePlatform {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target: String,
}

impl RuntimePlatform {
    pub fn from_target(target: &RuntimeTarget) -> Self {
        Self {
            os: target.os.clone(),
            arch: target.arch.clone(),
            libc: target.libc.clone(),
            rust_target: target.rust_target_triple.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCpu {
    pub logical_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMemory {
    pub total_bytes: u64,
    pub total_gb: f64,
    pub available_bytes: u64,
    pub available_gb: f64,
    pub process_bytes: u64,
    pub process_gb: f64,
}

impl RuntimeMemory {
    pub fn from_bytes(total_bytes: u64, available_bytes: u64, process_bytes: u64) -> Self {
        Self {
            total_bytes,
            total_gb: bytes_to_gb(total_bytes),
            available_bytes,
            available_gb: bytes_to_gb(available_bytes),
            process_bytes,
            process_gb: bytes_to_gb(process_bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeProfile {
    pub host_fingerprint: String,
    pub platform: RuntimePlatform,
    pub cpu: RuntimeCpu,
    pub memory: RuntimeMemory,
    pub uptime_seconds: u64,
    pub started_at: OffsetDateTime,
    pub captured_at: OffsetDateTime,
    pub service: String,
    pub service_version: String,
    pub service_status: String,
    pub metrics: RuntimeMetricsSnapshot,
}

#[derive(Debug)]
pub struct RuntimeProfileCollector {
    host_fingerprint: String,
    platform: RuntimePlatform,
    service: String,
    service_version: String,
    process_start: OffsetDateTime,
    service_status: String,
    sampler: Mutex<RuntimeMetricSampler>,
}

impl RuntimeProfileCollector {
    pub fn new(
        service: impl Into<String>,
        service_version: impl Into<String>,
        process_start: OffsetDateTime,
        service_status: impl Into<String>,
    ) -> Result<Self> {
        let target = RuntimeTarget::current_host()?;
        Ok(Self {
            host_fingerprint: detect_host_fingerprint()?,
            platform: RuntimePlatform::from_target(&target),
            service: service.into(),
            service_version: service_version.into(),
            process_start,
            service_status: service_status.into(),
            sampler: Mutex::new(RuntimeMetricSampler::new()),
        })
    }

    pub fn collect(&self) -> Result<RuntimeProfile> {
        let metrics = self
            .sampler
            .lock()
            .map_err(|_| anyhow!("runtime metric sampler lock poisoned"))?
            .collect();
        Ok(RuntimeProfile {
            host_fingerprint: self.host_fingerprint.clone(),
            platform: self.platform.clone(),
            cpu: RuntimeCpu {
                logical_count: metrics.cpu.logical_count,
            },
            memory: RuntimeMemory::from_bytes(
                metrics.memory.total_bytes,
                metrics.memory.available_bytes,
                metrics.memory.process_bytes,
            ),
            uptime_seconds: System::uptime(),
            started_at: self.process_start,
            captured_at: metrics.captured_at,
            service: self.service.clone(),
            service_version: self.service_version.clone(),
            service_status: self.service_status.clone(),
            metrics,
        })
    }
}

pub fn bytes_to_gb(bytes: u64) -> f64 {
    ((bytes as f64 / 1024_f64.powi(3)) * 100.0).round() / 100.0
}
