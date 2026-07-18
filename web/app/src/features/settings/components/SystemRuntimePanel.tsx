import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

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
type RuntimeHost = SettingsSystemRuntimeProfile['hosts'][number];

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

function pointFromMetrics(
  metrics: RuntimeMetrics,
  host: RuntimeHost | undefined,
  relatedProcessMemoryComplete: boolean
): RuntimeMetricPoint {
  return {
    capturedAt: metrics.captured_at_unix_milliseconds,
    cpuUsagePercent: metrics.cpu.usage_percent,
    environmentMemoryUsagePercent: usagePercent(
      metrics.memory.used_bytes,
      metrics.memory.total_bytes
    ),
    hostRelatedProcessBytes:
      relatedProcessMemoryComplete && host ? host.related_process_bytes : null,
    hostRelatedProcessCount:
      relatedProcessMemoryComplete && host ? host.related_process_count : null,
    targetRelatedProcessBytes: metrics.memory.related_process_bytes,
    targetRelatedProcessCount: metrics.memory.related_process_count,
    rootProcessBytes: metrics.memory.process_bytes,
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
  detail: ReactNode;
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
        const host = profile.hosts.find(
          (entry) => entry.host_fingerprint === target.host_fingerprint
        );
        const point = pointFromMetrics(
          target.metrics,
          host,
          profile.related_process_memory_complete
        );
        const existing = current[target.target_id] ?? [];
        const latest = existing.at(-1);
        if (
          latest?.capturedAt === point.capturedAt &&
          latest.hostRelatedProcessBytes === point.hostRelatedProcessBytes &&
          latest.hostRelatedProcessCount === point.hostRelatedProcessCount
        ) {
          return;
        }
        const cutoff = point.capturedAt - HISTORY_WINDOW_MILLISECONDS;
        const historyBeforePoint =
          latest?.capturedAt === point.capturedAt
            ? existing.slice(0, -1)
            : existing;
        next[target.target_id] = [...historyBeforePoint, point]
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
      <SettingsSectionSurface heightMode="fill">
        <Flex justify="center" className="system-runtime-panel__loading">
          <Spin />
        </Flex>
      </SettingsSectionSurface>
    );
  }

  if (runtimeQuery.isError && !profile) {
    return (
      <SettingsSectionSurface heightMode="fill">
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
      <SettingsSectionSurface heightMode="fill">
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
  const selectedHost = profile.hosts.find(
    (host) => host.host_fingerprint === selectedTarget?.host_fingerprint
  );
  const selectedHostProcessSummaries = selectedHost
    ? profile.runtime_targets.flatMap((target) =>
        target.reachable &&
        target.metrics &&
        target.host_fingerprint === selectedHost.host_fingerprint
          ? [
              {
                targetId: target.target_id,
                relatedProcessCount: target.metrics.memory.related_process_count
              }
            ]
          : []
      )
    : [];
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
    <SettingsSectionSurface heightMode="fill">
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
          aria-labelledby="runtime-environment-title"
        >
          <Flex align="center" justify="space-between" gap={12} wrap="wrap">
            <Flex align="center" gap={8}>
              <GlobalOutlined className="system-runtime-panel__section-icon" />
              <Typography.Title level={5} id="runtime-environment-title">
                {i18nText('settings', 'auto.runtime_environment')}
              </Typography.Title>
            </Flex>
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
          <Descriptions
            className="system-runtime-panel__environment-details"
            size="small"
            layout="vertical"
            column={{ xs: 1, sm: 1, lg: 3 }}
            items={[
              {
                key: 'related-process-memory',
                label: i18nText('settings', 'auto.related_process_memory'),
                children:
                  metrics && selectedHost ? (
                    <div className="system-runtime-panel__process-memory">
                      <span className="system-runtime-panel__process-memory-summary">
                        <Typography.Text strong>
                          {formatBytes(selectedHost.related_process_bytes)}
                        </Typography.Text>
                        <Typography.Text type="secondary">
                          {i18nText('settings', 'auto.process_count', {
                            value1: selectedHost.related_process_count
                          })}
                        </Typography.Text>
                      </span>
                      <Typography.Text type="secondary">
                        {i18nText('settings', 'auto.current_target_memory')}{' '}
                        {formatBytes(metrics.memory.related_process_bytes)} ·{' '}
                        {i18nText('settings', 'auto.process_count', {
                          value1: metrics.memory.related_process_count
                        })}
                      </Typography.Text>
                      <Typography.Text type="secondary">
                        {i18nText('settings', 'auto.root_process_rss')}{' '}
                        {formatBytes(metrics.memory.process_bytes)}
                      </Typography.Text>
                      {!profile.related_process_memory_complete ? (
                        <Typography.Text type="warning">
                          {i18nText(
                            'settings',
                            'auto.related_process_memory_partial'
                          )}
                        </Typography.Text>
                      ) : null}
                    </div>
                  ) : (
                    '—'
                  )
              },
              {
                key: 'plugin-root',
                label: i18nText('settings', 'auto.plugin_install_path'),
                children: (
                  <Typography.Text code>
                    {profile.provider_install_root}
                  </Typography.Text>
                )
              },
              {
                key: 'host-extension-root',
                label: i18nText('settings', 'auto.host_extension_path'),
                children: (
                  <Typography.Text code>
                    {profile.host_extension_dropin_root}
                  </Typography.Text>
                )
              }
            ]}
          />
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
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.last_two_minutes')}
            </Typography.Text>
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
                  detail={
                    <>
                      <span>
                        {formatBytes(metrics.memory.used_bytes)} /{' '}
                        {formatBytes(metrics.memory.total_bytes)}
                      </span>
                      {metrics.memory.scope_kind === 'cgroup' &&
                      metrics.memory.cgroup_composition ? (
                        <span className="system-runtime-panel__memory-composition">
                          <span className="system-runtime-panel__memory-composition-label">
                            {i18nText('settings', 'auto.memory_composition')}
                          </span>
                          {metrics.memory.cgroup_composition.anonymous_bytes !==
                          null ? (
                            <span>
                              {i18nText('settings', 'auto.anonymous_memory')}{' '}
                              {formatBytes(
                                metrics.memory.cgroup_composition
                                  .anonymous_bytes
                              )}
                            </span>
                          ) : null}
                          {metrics.memory.cgroup_composition.file_bytes !==
                          null ? (
                            <span>
                              {i18nText('settings', 'auto.file_memory')}{' '}
                              {formatBytes(
                                metrics.memory.cgroup_composition.file_bytes
                              )}
                            </span>
                          ) : null}
                          {metrics.memory.cgroup_composition.kernel_bytes !==
                          null ? (
                            <span>
                              {i18nText('settings', 'auto.kernel_memory')}{' '}
                              {formatBytes(
                                metrics.memory.cgroup_composition.kernel_bytes
                              )}
                            </span>
                          ) : null}
                          {metrics.memory.cgroup_composition
                            .shared_memory_bytes !== null ? (
                            <span>
                              {i18nText('settings', 'auto.shared_memory')}{' '}
                              {formatBytes(
                                metrics.memory.cgroup_composition
                                  .shared_memory_bytes
                              )}
                            </span>
                          ) : null}
                        </span>
                      ) : null}
                    </>
                  }
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
                <div className="system-runtime-panel__chart-toolbar">
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
                        label: i18nText('settings', 'auto.environment_memory'),
                        value: 'environment_memory'
                      },
                      {
                        label: i18nText('settings', 'auto.process_memory'),
                        value: 'process_memory'
                      }
                    ]}
                  />
                  {metricKind === 'process_memory' ? (
                    <div
                      aria-label={i18nText(
                        'settings',
                        'auto.related_process_memory'
                      )}
                      className="system-runtime-panel__process-summary"
                      role="group"
                    >
                      {selectedHostProcessSummaries.map((summary) => (
                        <Tag
                          className="system-runtime-panel__process-summary-tag"
                          key={summary.targetId}
                        >
                          {i18nText(
                            'settings',
                            'auto.runtime_target_process_count',
                            {
                              value1: serviceLabel(summary.targetId),
                              value2: summary.relatedProcessCount
                            }
                          )}
                        </Tag>
                      ))}
                      {profile.related_process_memory_complete &&
                      selectedHost ? (
                        <Tag className="system-runtime-panel__process-summary-tag system-runtime-panel__process-summary-tag--total">
                          {i18nText(
                            'settings',
                            'auto.same_host_process_total',
                            {
                              value1: selectedHost.related_process_count
                            }
                          )}
                        </Tag>
                      ) : null}
                    </div>
                  ) : null}
                </div>
                <RuntimeMetricsChart
                  kind={metricKind}
                  points={points}
                  targetLabel={serviceLabel(selectedTargetId)}
                />
              </div>
            </>
          ) : (
            <Empty description={i18nText('settings', 'auto.unavailable')} />
          )}
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
