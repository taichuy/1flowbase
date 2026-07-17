import { useEffect, useMemo, useRef, useState } from 'react';

import {
  CloudServerOutlined,
  ClusterOutlined,
  DashboardOutlined,
  ExclamationCircleOutlined,
  GlobalOutlined
} from '@ant-design/icons';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Badge,
  Descriptions,
  Empty,
  Flex,
  Progress,
  Segmented,
  Select,
  Spin,
  Tag,
  Typography
} from 'antd';

import { i18nText } from '../../../shared/i18n/text';
import {
  fetchSettingsSystemRuntimeProfile,
  settingsSystemRuntimeQueryKey
} from '../api/system-runtime';
import type { SettingsSystemRuntimeProfile } from '../api/system-runtime';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import {
  RuntimeMetricsChart,
  type RuntimeMetricKind,
  type RuntimeMetricPoint
} from './system-runtime/RuntimeMetricsChart';
import './system-runtime/system-runtime-panel.css';

const POLL_INTERVAL_MILLISECONDS = 2_000;
const HISTORY_WINDOW_MILLISECONDS = 120_000;
const MAX_HISTORY_POINTS = 60;
const MAX_CONSECUTIVE_FAILURES = 3;

type RuntimeTarget = SettingsSystemRuntimeProfile['runtime_targets'][number];
type RuntimeMetrics = NonNullable<RuntimeTarget['metrics']>;

function usePageVisibility() {
  const [visible, setVisible] = useState(
    () =>
      typeof document === 'undefined' || document.visibilityState !== 'hidden'
  );
  useEffect(() => {
    const handleVisibilityChange = () => {
      setVisible(document.visibilityState !== 'hidden');
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);
  return visible;
}

function relationshipLabel(relationship: string) {
  switch (relationship) {
    case 'same_host':
      return i18nText('settings', 'auto.deployment_same_machine');
    case 'split_host':
      return i18nText('settings', 'auto.extension_deployment');
    case 'runner_unreachable':
      return i18nText('settings', 'auto.runner_is_unreachable');
    default:
      return relationship;
  }
}

function serviceLabel(targetId: string) {
  if (targetId === 'api-server') {
    return 'API Server';
  }
  if (targetId === 'plugin-runner') {
    return 'Plugin Runner';
  }
  return targetId;
}

function formatBytes(value: number | null | undefined) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return '—';
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let normalized = Math.max(0, value);
  let unitIndex = 0;
  while (normalized >= 1024 && unitIndex < units.length - 1) {
    normalized /= 1024;
    unitIndex += 1;
  }
  const digits =
    normalized >= 100 || unitIndex === 0 ? 0 : normalized >= 10 ? 1 : 2;
  return `${normalized.toFixed(digits)} ${units[unitIndex]}`;
}

function formatRate(value: number | null | undefined) {
  return value === null || value === undefined
    ? '—'
    : `${formatBytes(value)}/s`;
}

function usagePercent(
  used: number | null | undefined,
  total: number | null | undefined
) {
  if (used === null || used === undefined || !total || total <= 0) {
    return null;
  }
  return Math.min(100, Math.max(0, (used / total) * 100));
}

function availabilityText(availability: RuntimeMetrics['cpu']['availability']) {
  switch (availability) {
    case 'warming_up':
      return i18nText('settings', 'auto.runtime_sampling');
    case 'stale':
      return i18nText('settings', 'auto.runtime_sample_stale');
    case 'unavailable':
      return i18nText('settings', 'auto.unavailable');
    default:
      return null;
  }
}

function scopeLabel(scope: RuntimeMetrics['cpu']['scope_kind']) {
  switch (scope) {
    case 'cgroup':
      return i18nText('settings', 'auto.runtime_scope_cgroup');
    case 'host':
      return i18nText('settings', 'auto.runtime_scope_host');
    default:
      return i18nText('settings', 'auto.runtime_scope_visible');
  }
}

function pointFromMetrics(metrics: RuntimeMetrics): RuntimeMetricPoint {
  return {
    capturedAt: metrics.captured_at_unix_milliseconds,
    cpuUsagePercent: metrics.cpu.usage_percent,
    memoryUsagePercent: usagePercent(
      metrics.memory.used_bytes,
      metrics.memory.total_bytes
    ),
    networkReceivedBytesPerSecond: metrics.network.received_bytes_per_second,
    networkTransmittedBytesPerSecond:
      metrics.network.transmitted_bytes_per_second,
    diskReadBytesPerSecond: metrics.disk_io.read_bytes_per_second,
    diskWrittenBytesPerSecond: metrics.disk_io.written_bytes_per_second
  };
}

function MetricGauge({
  label,
  percent,
  availability,
  detail
}: {
  label: string;
  percent: number | null;
  availability: RuntimeMetrics['cpu']['availability'];
  detail: string;
}) {
  const display =
    percent === null
      ? (availabilityText(availability) ?? '—')
      : `${percent.toFixed(1)}%`;
  return (
    <div className="system-runtime-panel__metric-gauge">
      <Typography.Text className="system-runtime-panel__metric-label">
        {label}
      </Typography.Text>
      <Progress
        type="circle"
        percent={percent ?? 0}
        size={78}
        strokeColor="#00ab73"
        trailColor="#e8edea"
        strokeWidth={8}
        format={() => display}
      />
      <Typography.Text
        type="secondary"
        className="system-runtime-panel__metric-detail"
      >
        {detail}
      </Typography.Text>
    </div>
  );
}

export function SystemRuntimePanel() {
  const pageVisible = usePageVisibility();
  const consecutiveFailuresRef = useRef(0);
  const [pollingStopped, setPollingStopped] = useState(false);
  const runtimeQuery = useQuery({
    queryKey: settingsSystemRuntimeQueryKey,
    queryFn: async () => {
      try {
        const profile = await fetchSettingsSystemRuntimeProfile();
        consecutiveFailuresRef.current = 0;
        setPollingStopped(false);
        return profile;
      } catch (error) {
        consecutiveFailuresRef.current += 1;
        if (consecutiveFailuresRef.current >= MAX_CONSECUTIVE_FAILURES) {
          setPollingStopped(true);
        }
        throw error;
      }
    },
    enabled: pageVisible && !pollingStopped,
    retry: false,
    refetchInterval:
      pageVisible && !pollingStopped ? POLL_INTERVAL_MILLISECONDS : false,
    refetchIntervalInBackground: false
  });
  const profile = runtimeQuery.data;
  const [selectedTargetId, setSelectedTargetId] = useState('api-server');
  const [metricKind, setMetricKind] = useState<RuntimeMetricKind>('network');
  const [histories, setHistories] = useState<
    Record<string, RuntimeMetricPoint[]>
  >({});

  useEffect(() => {
    if (!profile) {
      return;
    }
    setHistories((current) => {
      let changed = false;
      const next = { ...current };
      profile.runtime_targets.forEach((target) => {
        if (!target.reachable || !target.metrics) {
          return;
        }
        const point = pointFromMetrics(target.metrics);
        const existing = current[target.target_id] ?? [];
        if (existing.at(-1)?.capturedAt === point.capturedAt) {
          return;
        }
        const cutoff = point.capturedAt - HISTORY_WINDOW_MILLISECONDS;
        next[target.target_id] = [...existing, point]
          .filter((entry) => entry.capturedAt >= cutoff)
          .slice(-MAX_HISTORY_POINTS);
        changed = true;
      });
      return changed ? next : current;
    });
  }, [profile]);

  const reachableTargets = useMemo(
    () => profile?.runtime_targets.filter((target) => target.reachable) ?? [],
    [profile]
  );
  useEffect(() => {
    if (
      reachableTargets.length > 0 &&
      !reachableTargets.some((target) => target.target_id === selectedTargetId)
    ) {
      setSelectedTargetId(reachableTargets[0].target_id);
    }
  }, [reachableTargets, selectedTargetId]);

  if (runtimeQuery.isLoading) {
    return (
      <SettingsSectionSurface>
        <Flex justify="center" className="system-runtime-panel__loading">
          <Spin />
        </Flex>
      </SettingsSectionSurface>
    );
  }

  if (runtimeQuery.isError && !profile) {
    return (
      <SettingsSectionSurface>
        <Alert
          type="error"
          showIcon
          message={i18nText(
            'settings',
            'auto.runtime_information_loading_failed'
          )}
          description={
            runtimeQuery.error instanceof Error
              ? runtimeQuery.error.message
              : i18nText('settings', 'auto.try_again_later')
          }
        />
      </SettingsSectionSurface>
    );
  }

  if (!profile) {
    return (
      <SettingsSectionSurface>
        <Empty description={i18nText('settings', 'auto.runtime_data_yet')} />
      </SettingsSectionSurface>
    );
  }

  const services = [
    profile.services.api_server,
    profile.services.plugin_runner
  ];
  const selectedTarget = profile.runtime_targets.find(
    (target) => target.target_id === selectedTargetId
  );
  const metrics = selectedTarget?.metrics ?? null;
  const points = histories[selectedTargetId] ?? [];
  const memoryPercent = metrics
    ? usagePercent(metrics.memory.used_bytes, metrics.memory.total_bytes)
    : null;
  const storagePercent = metrics
    ? usagePercent(metrics.storage.used_bytes, metrics.storage.total_bytes)
    : null;
  const liveStatus = !pageVisible
    ? i18nText('settings', 'auto.runtime_collection_paused')
    : pollingStopped
      ? i18nText('settings', 'auto.runtime_collection_stopped')
      : i18nText('settings', 'auto.runtime_collecting');

  return (
    <SettingsSectionSurface>
      <div className="system-runtime-panel">
        <section
          className="system-runtime-panel__section"
          aria-labelledby="runtime-overview-title"
        >
          <Flex align="center" justify="space-between" gap={12} wrap="wrap">
            <Flex align="center" gap={8}>
              <CloudServerOutlined className="system-runtime-panel__section-icon" />
              <Typography.Title level={5} id="runtime-overview-title">
                {i18nText('settings', 'auto.runtime_overview')}
              </Typography.Title>
            </Flex>
            <Tag icon={<ClusterOutlined />}>
              {relationshipLabel(profile.topology.relationship)}
            </Tag>
          </Flex>

          <div className="system-runtime-panel__service-list" role="table">
            {services.map((service) => {
              const host = profile.hosts.find(
                (entry) => entry.host_fingerprint === service.host_fingerprint
              );
              return (
                <div
                  className="system-runtime-panel__service-row"
                  key={service.service}
                  role="row"
                >
                  <div
                    className="system-runtime-panel__service-name"
                    role="cell"
                  >
                    <Typography.Text strong>
                      {serviceLabel(service.service)}
                    </Typography.Text>
                    <Badge
                      color={service.reachable ? '#19b36b' : '#fb565b'}
                      text={
                        service.reachable
                          ? i18nText('settings', 'auto.running')
                          : i18nText('settings', 'auto.not_reachable')
                      }
                    />
                  </div>
                  <div role="cell">
                    <Typography.Text type="secondary">
                      {i18nText('settings', 'auto.version')}
                    </Typography.Text>
                    <Typography.Text>{service.version ?? '—'}</Typography.Text>
                  </div>
                  <div role="cell">
                    <Typography.Text type="secondary">
                      {i18nText('settings', 'auto.platform')}
                    </Typography.Text>
                    <Typography.Text>
                      {host
                        ? `${host.platform.os}/${host.platform.arch}${host.platform.libc ? `/${host.platform.libc}` : ''}`
                        : '—'}
                    </Typography.Text>
                  </div>
                  <div role="cell">
                    <Typography.Text type="secondary">
                      CPU / {i18nText('settings', 'auto.memory')}
                    </Typography.Text>
                    <Typography.Text>
                      {host
                        ? `${host.cpu.logical_count} · ${formatBytes(host.memory.total_bytes)}`
                        : '—'}
                    </Typography.Text>
                  </div>
                  <Typography.Text
                    code
                    className="system-runtime-panel__fingerprint"
                    role="cell"
                  >
                    {service.host_fingerprint?.slice(0, 16) ?? '—'}
                  </Typography.Text>
                </div>
              );
            })}
          </div>
        </section>

        <section
          className="system-runtime-panel__section"
          aria-labelledby="runtime-monitor-title"
        >
          <Flex align="center" justify="space-between" gap={12} wrap="wrap">
            <Flex align="center" gap={8}>
              <DashboardOutlined className="system-runtime-panel__section-icon" />
              <Typography.Title level={5} id="runtime-monitor-title">
                {i18nText('settings', 'auto.resource_monitoring')}
              </Typography.Title>
              <Badge
                color={pageVisible && !pollingStopped ? '#00ab73' : '#7b8982'}
                text={liveStatus}
              />
            </Flex>
            <Flex align="center" gap={8} wrap="wrap">
              <Typography.Text type="secondary">
                {i18nText('settings', 'auto.last_two_minutes')}
              </Typography.Text>
              <Select
                aria-label={i18nText('settings', 'auto.runtime_target')}
                value={selectedTargetId}
                onChange={setSelectedTargetId}
                options={profile.runtime_targets.map((target) => ({
                  value: target.target_id,
                  label: serviceLabel(target.target_id),
                  disabled: !target.reachable
                }))}
                popupMatchSelectWidth={false}
              />
            </Flex>
          </Flex>

          {runtimeQuery.isRefetchError ? (
            <Alert
              type="warning"
              showIcon
              message={i18nText(
                'settings',
                'auto.runtime_information_loading_failed'
              )}
            />
          ) : null}

          {metrics ? (
            <>
              <div className="system-runtime-panel__metric-strip">
                <MetricGauge
                  label={i18nText('settings', 'auto.cpu_usage')}
                  percent={metrics.cpu.usage_percent}
                  availability={metrics.cpu.availability}
                  detail={`${metrics.cpu.limit_cores.toFixed(1)} vCPU · ${scopeLabel(metrics.cpu.scope_kind)}`}
                />
                <MetricGauge
                  label={i18nText('settings', 'auto.memory_usage')}
                  percent={memoryPercent}
                  availability={metrics.memory.availability}
                  detail={`${formatBytes(metrics.memory.used_bytes)} / ${formatBytes(metrics.memory.total_bytes)}`}
                />
                <MetricGauge
                  label={i18nText('settings', 'auto.storage_usage')}
                  percent={storagePercent}
                  availability={metrics.storage.availability}
                  detail={`${metrics.storage.mount_point ?? '—'} · ${formatBytes(metrics.storage.total_bytes)}`}
                />
                <div className="system-runtime-panel__throughput">
                  <Typography.Text className="system-runtime-panel__metric-label">
                    {i18nText('settings', 'auto.current_throughput')}
                  </Typography.Text>
                  <div>
                    <Typography.Text type="secondary">
                      {i18nText('settings', 'auto.network_traffic')}
                    </Typography.Text>
                    <Typography.Text>
                      ↓ {formatRate(metrics.network.received_bytes_per_second)}{' '}
                      · ↑{' '}
                      {formatRate(metrics.network.transmitted_bytes_per_second)}
                    </Typography.Text>
                  </div>
                  <div>
                    <Typography.Text type="secondary">
                      {i18nText('settings', 'auto.disk_io')}
                    </Typography.Text>
                    <Typography.Text>
                      R {formatRate(metrics.disk_io.read_bytes_per_second)} · W{' '}
                      {formatRate(metrics.disk_io.written_bytes_per_second)}
                    </Typography.Text>
                  </div>
                  <Tag>{scopeLabel(metrics.network.scope_kind)}</Tag>
                </div>
              </div>

              <div className="system-runtime-panel__chart-panel">
                <Segmented<RuntimeMetricKind>
                  aria-label={i18nText('settings', 'auto.runtime_metric')}
                  value={metricKind}
                  onChange={setMetricKind}
                  options={[
                    {
                      label: i18nText('settings', 'auto.network_traffic'),
                      value: 'network'
                    },
                    {
                      label: i18nText('settings', 'auto.disk_io'),
                      value: 'disk_io'
                    },
                    { label: 'CPU', value: 'cpu' },
                    {
                      label: i18nText('settings', 'auto.memory'),
                      value: 'memory'
                    }
                  ]}
                />
                <RuntimeMetricsChart kind={metricKind} points={points} />
              </div>
            </>
          ) : (
            <Empty description={i18nText('settings', 'auto.unavailable')} />
          )}
        </section>

        <section
          className="system-runtime-panel__section"
          aria-labelledby="runtime-environment-title"
        >
          <Flex align="center" gap={8}>
            <GlobalOutlined className="system-runtime-panel__section-icon" />
            <Typography.Title level={5} id="runtime-environment-title">
              {i18nText('settings', 'auto.runtime_environment')}
            </Typography.Title>
          </Flex>
          <Descriptions
            size="small"
            column={{ xs: 1, sm: 2, lg: 4 }}
            items={[
              {
                key: 'current-locale',
                label: i18nText('settings', 'auto.current_language'),
                children: profile.locale_meta.resolved_locale
              },
              {
                key: 'fallback-locale',
                label: i18nText('settings', 'auto.fallback_language'),
                children: profile.locale_meta.fallback_locale
              },
              {
                key: 'supported-locales',
                label: i18nText('settings', 'auto.supported_languages'),
                children: profile.locale_meta.supported_locales.join(', ')
              },
              {
                key: 'process-memory',
                label: i18nText('settings', 'auto.process_memory'),
                children: metrics
                  ? formatBytes(metrics.memory.process_bytes)
                  : '—'
              },
              {
                key: 'plugin-root',
                label: i18nText('settings', 'auto.plugin_install_path'),
                children: (
                  <Typography.Text code>
                    {profile.provider_install_root}
                  </Typography.Text>
                ),
                span: 2
              },
              {
                key: 'host-extension-root',
                label: i18nText('settings', 'auto.host_extension_path'),
                children: (
                  <Typography.Text code>
                    {profile.host_extension_dropin_root}
                  </Typography.Text>
                ),
                span: 2
              }
            ]}
          />
        </section>

        {profile.topology.relationship === 'runner_unreachable' ? (
          <Alert
            type="warning"
            showIcon
            icon={<ExclamationCircleOutlined />}
            message={i18nText('settings', 'auto.runner_is_unreachable')}
          />
        ) : null}
      </div>
    </SettingsSectionSurface>
  );
}
