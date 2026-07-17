import { apiFetch } from './transport';

export interface ConsoleReleaseInfo {
  name: string;
  body: string;
  published_at: string;
  html_url: string;
}

export interface ConsoleReleaseUpgradeCommands {
  shell: string;
  powershell: string;
}

export interface ConsoleReleaseStatus {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_info: ConsoleReleaseInfo | null;
  contributors_url: string;
  upgrade_commands: ConsoleReleaseUpgradeCommands;
  cached: boolean;
  warning: string | null;
}

export interface ConsoleSystemRuntimeLocaleMeta {
  requested_locale: string | null;
  resolved_locale: string;
  source: string;
  fallback_locale: string;
  supported_locales: string[];
}

export interface ConsoleSystemRuntimeTopology {
  relationship: string;
}

export interface ConsoleSystemRuntimeService {
  reachable: boolean;
  service: string;
  status: string | null;
  version: string | null;
  host_fingerprint: string | null;
}

export interface ConsoleSystemRuntimeServices {
  api_server: ConsoleSystemRuntimeService;
  plugin_runner: ConsoleSystemRuntimeService;
}

export interface ConsoleSystemRuntimePlatform {
  os: string;
  arch: string;
  libc: string | null;
  rust_target_triple: string;
}

export interface ConsoleSystemRuntimeCpu {
  logical_count: number;
}

export interface ConsoleSystemRuntimeMemory {
  total_bytes: number;
  total_gb: number;
  available_bytes: number;
  available_gb: number;
  process_bytes: number;
  process_gb: number;
}

export interface ConsoleSystemRuntimeHost {
  host_fingerprint: string;
  platform: ConsoleSystemRuntimePlatform;
  cpu: ConsoleSystemRuntimeCpu;
  memory: ConsoleSystemRuntimeMemory;
  services: string[];
}

export type ConsoleSystemRuntimeMetricAvailability =
  | 'available'
  | 'warming_up'
  | 'stale'
  | 'unavailable';

export type ConsoleSystemRuntimeMetricScopeKind =
  | 'cgroup'
  | 'host'
  | 'runtime_visible';

export interface ConsoleSystemRuntimeCpuMetrics {
  availability: ConsoleSystemRuntimeMetricAvailability;
  scope_kind: ConsoleSystemRuntimeMetricScopeKind;
  usage_percent: number | null;
  logical_count: number;
  limit_cores: number;
}

export interface ConsoleSystemRuntimeMemoryMetrics {
  availability: ConsoleSystemRuntimeMetricAvailability;
  scope_kind: ConsoleSystemRuntimeMetricScopeKind;
  total_bytes: number;
  available_bytes: number;
  used_bytes: number;
  process_bytes: number;
}

export interface ConsoleSystemRuntimeStorageMetrics {
  availability: ConsoleSystemRuntimeMetricAvailability;
  scope_kind: ConsoleSystemRuntimeMetricScopeKind;
  mount_point: string | null;
  file_system: string | null;
  total_bytes: number | null;
  available_bytes: number | null;
  used_bytes: number | null;
}

export interface ConsoleSystemRuntimeNetworkMetrics {
  availability: ConsoleSystemRuntimeMetricAvailability;
  scope_kind: ConsoleSystemRuntimeMetricScopeKind;
  received_bytes_per_second: number | null;
  transmitted_bytes_per_second: number | null;
}

export interface ConsoleSystemRuntimeDiskIoMetrics {
  availability: ConsoleSystemRuntimeMetricAvailability;
  scope_kind: ConsoleSystemRuntimeMetricScopeKind;
  read_bytes_per_second: number | null;
  written_bytes_per_second: number | null;
}

export interface ConsoleSystemRuntimeMetrics {
  captured_at_unix_milliseconds: number;
  sample_interval_milliseconds: number | null;
  cpu: ConsoleSystemRuntimeCpuMetrics;
  memory: ConsoleSystemRuntimeMemoryMetrics;
  storage: ConsoleSystemRuntimeStorageMetrics;
  network: ConsoleSystemRuntimeNetworkMetrics;
  disk_io: ConsoleSystemRuntimeDiskIoMetrics;
}

export interface ConsoleSystemRuntimeTarget {
  target_id: string;
  reachable: boolean;
  host_fingerprint: string | null;
  metrics: ConsoleSystemRuntimeMetrics | null;
}

export interface ConsoleSystemRuntimeProfile {
  provider_install_root: string;
  host_extension_dropin_root: string;
  locale_meta: ConsoleSystemRuntimeLocaleMeta;
  topology: ConsoleSystemRuntimeTopology;
  services: ConsoleSystemRuntimeServices;
  hosts: ConsoleSystemRuntimeHost[];
  runtime_targets: ConsoleSystemRuntimeTarget[];
}

export function fetchConsoleSystemRuntimeProfile(baseUrl?: string) {
  return apiFetch<ConsoleSystemRuntimeProfile>({
    path: '/api/console/system/runtime-profile',
    baseUrl
  });
}

export function fetchConsoleReleaseStatus(baseUrl?: string) {
  return apiFetch<ConsoleReleaseStatus>({
    path: '/api/console/system/release-status',
    baseUrl
  });
}
