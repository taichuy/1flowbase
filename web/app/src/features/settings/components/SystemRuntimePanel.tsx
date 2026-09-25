import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode
} from 'react';

import DashboardOutlined from '@ant-design/icons/es/icons/DashboardOutlined';
import ExclamationCircleOutlined from '@ant-design/icons/es/icons/ExclamationCircleOutlined';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Badge,
  Button,
  Descriptions,
  Empty,
  Flex,
  Input,
  Progress,
  Segmented,
  Splitter,
  Table,
  Tabs,
  Tag,
  Tree,
  Typography,
  type TableColumnsType
} from 'antd';

import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import {
  fetchSettingsSystemRuntimeProcesses,
  fetchSettingsSystemRuntimeProfile,
  settingsSystemRuntimeProcessesQueryKey,
  settingsSystemRuntimeQueryKey,
  terminateSettingsSystemRuntimeProcess
} from '../api/system-runtime';
import type {
  SettingsSystemRuntimeProcessList,
  SettingsSystemRuntimeProfile
} from '../api/system-runtime';
import {
  persistProcessTreeSplitRatio,
  readProcessTreeSplitRatio
} from '../lib/process-tree-split-ratio';
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
const PRIMARY_SERVICE_TARGET_ID = 'api-server';
const PROCESS_TABLE_PAGE_SIZE = 50;

function useNarrowProcessTreeLayout() {
  const [narrow, setNarrow] = useState(
    () =>
      typeof window !== 'undefined' &&
      window.matchMedia('(max-width: 767px)').matches
  );
  useEffect(() => {
    const media = window.matchMedia('(max-width: 767px)');
    const update = () => setNarrow(media.matches);
    media.addEventListener('change', update);
    update();
    return () => media.removeEventListener('change', update);
  }, []);
  return narrow;
}

type RuntimeTarget = SettingsSystemRuntimeProfile['runtime_targets'][number];
type RuntimeMetrics = NonNullable<RuntimeTarget['metrics']>;
type RuntimeProcess = SettingsSystemRuntimeProcessList['processes'][number];
type SystemRuntimeTab = 'resources' | 'processes';

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

function serviceLabel(targetId: string) {
  if (targetId === PRIMARY_SERVICE_TARGET_ID) {
    return 'API Server';
  }
  if (targetId === 'runtime-extension-host') {
    return 'Runtime Extension Host';
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

function formatPercent(value: number | null | undefined) {
  return value === null || value === undefined || !Number.isFinite(value)
    ? '—'
    : `${value.toFixed(2)}%`;
}

function formatMegabytes(value: number) {
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatStartedAt(seconds: number) {
  const date = new Date(seconds * 1000);
  const pad = (value: number) => String(value).padStart(2, '0');
  return [
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  ].join(' ');
}

function processStatusLabel(status: string) {
  switch (status) {
    case 'running':
      return i18nText('settings', 'auto.running');
    case 'sleeping':
      return i18nText('settings', 'auto.process_status_sleeping');
    case 'stopped':
      return i18nText('settings', 'auto.process_status_stopped');
    case 'zombie':
      return i18nText('settings', 'auto.process_status_zombie');
    case 'idle':
      return i18nText('settings', 'auto.process_status_idle');
    default:
      return status;
  }
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
    environmentMemoryUsagePercent: usagePercent(
      metrics.memory.used_bytes,
      metrics.memory.total_bytes
    ),
    targetRelatedProcessBytes: metrics.memory.related_process_bytes,
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
      ? availability === 'warming_up'
        ? '0%'
        : (availabilityText(availability) ?? '—')
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
        railColor="#e8edea"
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

function processColumns(
  onTerminate: (pid: number) => void,
  terminating: boolean
): TableColumnsType<RuntimeProcess> {
  return [
    {
      title: i18nText('settings', 'auto.process_column_pid'),
      dataIndex: 'pid',
      key: 'pid',
      width: 84,
      sorter: (left, right) => left.pid - right.pid
    },
    {
      title: i18nText('settings', 'auto.process_column_name'),
      dataIndex: 'name',
      key: 'name',
      width: 130,
      ellipsis: true
    },
    {
      title: i18nText('settings', 'auto.process_column_command'),
      dataIndex: 'command',
      key: 'command',
      width: 260,
      ellipsis: true,
      render: (command: string | null) => command ?? '—'
    },
    {
      title: i18nText('settings', 'auto.process_column_cpu'),
      dataIndex: 'cpu_usage_percent',
      key: 'cpu_usage_percent',
      width: 110,
      defaultSortOrder: 'descend',
      sorter: (left, right) => left.cpu_usage_percent - right.cpu_usage_percent,
      render: (value: number) => formatPercent(value)
    },
    {
      title: i18nText('settings', 'auto.memory'),
      dataIndex: 'memory_bytes',
      key: 'memory_bytes',
      width: 110,
      sorter: (left, right) => left.memory_bytes - right.memory_bytes,
      render: (value: number) => formatBytes(value)
    },
    {
      title: i18nText('settings', 'auto.process_column_user'),
      dataIndex: 'user',
      key: 'user',
      width: 100,
      ellipsis: true,
      render: (user: string | null) => user ?? '—'
    },
    {
      title: i18nText('settings', 'auto.process_column_started_at'),
      dataIndex: 'start_time_unix_seconds',
      key: 'start_time_unix_seconds',
      width: 156,
      sorter: (left, right) =>
        left.start_time_unix_seconds - right.start_time_unix_seconds,
      render: (seconds: number) => formatStartedAt(seconds)
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'action',
      width: 84,
      render: (_value: unknown, process: RuntimeProcess) => (
        <Button
          danger
          type="link"
          size="small"
          disabled={!process.terminable || terminating}
          onClick={() => onTerminate(process.pid)}
        >
          {i18nText('settings', 'auto.process_terminate')}
        </Button>
      )
    }
  ];
}

interface BackendProcessTreeNode {
  key: string;
  title: ReactNode;
  children: BackendProcessTreeNode[];
}

function buildBackendProcessTree(
  processes: RuntimeProcess[]
): BackendProcessTreeNode[] {
  const nodes = new Map<number, BackendProcessTreeNode>();
  processes.forEach((process) => {
    nodes.set(process.pid, {
      key: `process-${process.pid}`,
      title: (
        <span className="system-runtime-panel__process-tree-title">
          <Typography.Text strong>{process.name}</Typography.Text>
          <Typography.Text type="secondary">
            c:{formatPercent(process.cpu_usage_percent)} · m:
            {formatMegabytes(process.memory_bytes)}
          </Typography.Text>
        </span>
      ),
      children: []
    });
  });

  const roots: BackendProcessTreeNode[] = [];
  processes.forEach((process) => {
    const node = nodes.get(process.pid);
    if (!node) {
      return;
    }
    const parent =
      process.parent_pid === null ? undefined : nodes.get(process.parent_pid);
    if (parent) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  });
  return roots;
}

function collectProcessTreeKeys(nodes: BackendProcessTreeNode[]): string[] {
  return nodes.flatMap((node) => [
    node.key,
    ...collectProcessTreeKeys(node.children)
  ]);
}

export function SystemRuntimePanel() {
  const pageVisible = usePageVisibility();
  const narrowProcessTreeLayout = useNarrowProcessTreeLayout();
  const [processTreeSplitRatio, setProcessTreeSplitRatio] = useState(
    readProcessTreeSplitRatio
  );
  const saveProcessTreeSplitRatio = useCallback((sizes: number[]) => {
    const ratio = persistProcessTreeSplitRatio(sizes);
    if (ratio !== null) {
      setProcessTreeSplitRatio(ratio);
    }
  }, []);
  const queryClient = useQueryClient();
  const { message } = App.useApp();
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
  const processesQuery = useQuery({
    queryKey: settingsSystemRuntimeProcessesQueryKey,
    queryFn: fetchSettingsSystemRuntimeProcesses,
    enabled: pageVisible && !pollingStopped,
    retry: false,
    refetchInterval:
      pageVisible && !pollingStopped ? POLL_INTERVAL_MILLISECONDS : false,
    refetchIntervalInBackground: false
  });
  const [activeTab, setActiveTab] = useState<SystemRuntimeTab>('resources');
  const [metricKind, setMetricKind] = useState<RuntimeMetricKind>('network');
  const [history, setHistory] = useState<RuntimeMetricPoint[]>([]);
  const [processQuery, setProcessQuery] = useState('');

  const monitoredTarget = useMemo(() => {
    if (!profile) {
      return undefined;
    }
    return (
      profile.runtime_targets.find(
        (target) =>
          target.target_id === PRIMARY_SERVICE_TARGET_ID && target.reachable
      ) ??
      profile.runtime_targets.find(
        (target) => target.target_id === PRIMARY_SERVICE_TARGET_ID
      ) ??
      profile.runtime_targets.find((target) => target.reachable) ??
      profile.runtime_targets[0]
    );
  }, [profile]);

  useEffect(() => {
    if (!monitoredTarget?.reachable || !monitoredTarget.metrics) {
      return;
    }
    const point = pointFromMetrics(monitoredTarget.metrics);
    setHistory((current) => {
      if (current.at(-1)?.capturedAt === point.capturedAt) {
        return current;
      }
      const cutoff = point.capturedAt - HISTORY_WINDOW_MILLISECONDS;
      return [...current, point]
        .filter((entry) => entry.capturedAt >= cutoff)
        .slice(-MAX_HISTORY_POINTS);
    });
  }, [monitoredTarget]);

  const visibleProcesses = useMemo(() => {
    const processes = processesQuery.data?.processes ?? [];
    const query = processQuery.trim().toLowerCase();
    if (!query) {
      return processes;
    }
    return processes.filter((process) =>
      [
        String(process.pid),
        process.name,
        process.command ?? '',
        process.user ?? ''
      ]
        .join(' ')
        .toLowerCase()
        .includes(query)
    );
  }, [processesQuery.data, processQuery]);

  const terminateProcess = useMutation({
    mutationFn: (pid: number) => terminateSettingsSystemRuntimeProcess(pid),
    onSuccess: (result) => {
      if (result.outcome === 'signalled') {
        message.success(
          i18nText('settings', 'auto.process_terminate_signalled')
        );
      } else if (result.outcome === 'forbidden') {
        message.warning(
          i18nText('settings', 'auto.process_terminate_forbidden')
        );
      } else if (result.outcome === 'not_observable') {
        message.info(
          i18nText('settings', 'auto.process_terminate_not_observable')
        );
      } else {
        message.error(i18nText('settings', 'auto.process_terminate_failed'));
      }
      void queryClient.invalidateQueries({
        queryKey: settingsSystemRuntimeQueryKey
      });
    },
    onError: (error) => {
      message.error(
        error instanceof Error
          ? error.message
          : i18nText('settings', 'auto.process_terminate_failed')
      );
    }
  });
  const handleTerminate = useCallback(
    (pid: number) => {
      terminateProcess.mutate(pid);
    },
    [terminateProcess]
  );
  const columns = useMemo(
    () => processColumns(handleTerminate, terminateProcess.isPending),
    [handleTerminate, terminateProcess.isPending]
  );

  const backendProcesses = useMemo(
    () =>
      (processesQuery.data?.processes ?? []).filter(
        (process) => process.backend_process
      ),
    [processesQuery.data]
  );
  const backendProcessIds = backendProcesses
    .map((process) => process.pid)
    .join(',');
  const backendTreeData = useMemo(
    () => buildBackendProcessTree(backendProcesses),
    [backendProcesses]
  );
  const [expandedTreeKeys, setExpandedTreeKeys] = useState<string[]>([]);
  const [selectedProcessId, setSelectedProcessId] = useState<number | null>(
    null
  );
  useEffect(() => {
    const keys = collectProcessTreeKeys(
      buildBackendProcessTree(backendProcesses)
    );
    setExpandedTreeKeys((current) =>
      Array.from(new Set([...current, ...keys]))
    );
    setSelectedProcessId((current) => {
      if (
        current !== null &&
        backendProcesses.some((process) => process.pid === current)
      ) {
        return current;
      }
      const root =
        backendProcesses.find(
          (process) =>
            !backendProcesses.some(
              (candidate) => candidate.pid === process.parent_pid
            )
        ) ?? backendProcesses[0];
      return root ? root.pid : null;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backendProcessIds]);
  const selectedProcess =
    selectedProcessId === null
      ? null
      : (backendProcesses.find(
          (process) => process.pid === selectedProcessId
        ) ?? null);

  if (runtimeQuery.isLoading) {
    return (
      <SettingsSectionSurface heightMode="fill">
        <LoadingState compact />
      </SettingsSectionSurface>
    );
  }

  if (runtimeQuery.isError && !profile) {
    return (
      <SettingsSectionSurface heightMode="fill">
        <Alert
          type="error"
          showIcon
          title={i18nText(
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

  const metrics = monitoredTarget?.metrics ?? null;
  const points = history;
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

  const processTreeSection = (
    <section
      className="system-runtime-panel__section"
      aria-label={i18nText('settings', 'auto.process_tree')}
    >
      {backendTreeData.length > 0 ? (
        <Splitter
          key={narrowProcessTreeLayout ? 'vertical' : 'horizontal'}
          className="system-runtime-panel__process-tree-layout"
          orientation={narrowProcessTreeLayout ? 'vertical' : 'horizontal'}
          onResizeEnd={saveProcessTreeSplitRatio}
        >
          <Splitter.Panel
            className="system-runtime-panel__process-tree-panel"
            defaultSize={`${processTreeSplitRatio}%`}
            min="20%"
          >
            <Tree
              className="system-runtime-panel__process-tree"
              selectedKeys={
                selectedProcessId === null
                  ? []
                  : [`process-${selectedProcessId}`]
              }
              expandedKeys={expandedTreeKeys}
              onExpand={(keys) => setExpandedTreeKeys(keys.map(String))}
              onSelect={(keys) => {
                const key = keys[0];
                if (typeof key === 'string' && key.startsWith('process-')) {
                  setSelectedProcessId(Number(key.slice('process-'.length)));
                }
              }}
              treeData={backendTreeData}
            />
          </Splitter.Panel>
          <Splitter.Panel
            className="system-runtime-panel__process-detail-panel"
            defaultSize={`${100 - processTreeSplitRatio}%`}
            min="20%"
          >
            {selectedProcess ? (
              <>
                <Descriptions
                  size="small"
                  column={1}
                  className="system-runtime-panel__process-detail"
                  items={[
                    {
                      key: 'pid',
                      label: i18nText('settings', 'auto.process_column_pid'),
                      children: selectedProcess.pid
                    },
                    {
                      key: 'parent_pid',
                      label: i18nText(
                        'settings',
                        'auto.process_detail_parent_pid'
                      ),
                      children:
                        selectedProcess.parent_pid === null
                          ? '—'
                          : selectedProcess.parent_pid
                    },
                    {
                      key: 'name',
                      label: i18nText('settings', 'auto.process_column_name'),
                      children: selectedProcess.name
                    },
                    {
                      key: 'command',
                      label: i18nText(
                        'settings',
                        'auto.process_column_command'
                      ),
                      children: (
                        <Typography.Text code>
                          {selectedProcess.command ?? '—'}
                        </Typography.Text>
                      )
                    },
                    {
                      key: 'user',
                      label: i18nText('settings', 'auto.process_column_user'),
                      children: selectedProcess.user ?? '—'
                    },
                    {
                      key: 'status',
                      label: i18nText('settings', 'auto.status'),
                      children: processStatusLabel(selectedProcess.status)
                    },
                    {
                      key: 'started_at',
                      label: i18nText(
                        'settings',
                        'auto.process_column_started_at'
                      ),
                      children: formatStartedAt(
                        selectedProcess.start_time_unix_seconds
                      )
                    },
                    {
                      key: 'cpu',
                      label: i18nText('settings', 'auto.process_column_cpu'),
                      children: formatPercent(selectedProcess.cpu_usage_percent)
                    },
                    {
                      key: 'memory',
                      label: i18nText('settings', 'auto.memory'),
                      children: formatBytes(selectedProcess.memory_bytes)
                    }
                  ]}
                />
                {selectedProcess.terminable ? (
                  <Button
                    danger
                    loading={terminateProcess.isPending}
                    onClick={() => handleTerminate(selectedProcess.pid)}
                  >
                    {i18nText('settings', 'auto.process_terminate')}
                  </Button>
                ) : (
                  <Typography.Text type="secondary">
                    {i18nText('settings', 'auto.process_detail_not_terminable')}
                  </Typography.Text>
                )}
              </>
            ) : (
              <Empty
                description={i18nText('settings', 'auto.process_detail_empty')}
              />
            )}
          </Splitter.Panel>
        </Splitter>
      ) : (
        <Empty description={i18nText('settings', 'auto.process_tree_empty')} />
      )}
    </section>
  );

  const resourcePane = (
    <section
      className="system-runtime-panel__section"
      aria-label={i18nText('settings', 'auto.resource_monitoring')}
    >
      <Flex align="center" justify="space-between" gap={12} wrap="wrap">
        <Flex align="center" gap={8}>
          <DashboardOutlined className="system-runtime-panel__section-icon" />
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
          title={i18nText(
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
                            metrics.memory.cgroup_composition.anonymous_bytes
                          )}
                        </span>
                      ) : null}
                      {metrics.memory.cgroup_composition.file_bytes !== null ? (
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
                      {metrics.memory.cgroup_composition.shared_memory_bytes !==
                      null ? (
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
              detail={`${metrics.storage.mount_point ?? '—'} · ${formatBytes(metrics.storage.used_bytes)} / ${formatBytes(metrics.storage.total_bytes)}`}
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
                  ↓ {formatRate(metrics.network.received_bytes_per_second)} · ↑{' '}
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
        </>
      ) : (
        <Empty description={i18nText('settings', 'auto.unavailable')} />
      )}

      {processTreeSection}

      {metrics ? (
        <>
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
              {metricKind === 'process_memory' && monitoredTarget ? (
                <div
                  aria-label={i18nText(
                    'settings',
                    'auto.related_process_memory'
                  )}
                  className="system-runtime-panel__process-summary"
                  role="group"
                >
                  <Tag className="system-runtime-panel__process-summary-tag">
                    {i18nText('settings', 'auto.runtime_target_process_count', {
                      value1: serviceLabel(monitoredTarget.target_id),
                      value2: metrics.memory.related_process_count
                    })}
                  </Tag>
                </div>
              ) : null}
            </div>
            <RuntimeMetricsChart
              kind={metricKind}
              points={points}
              targetLabel={serviceLabel(
                monitoredTarget?.target_id ?? PRIMARY_SERVICE_TARGET_ID
              )}
            />
          </div>
        </>
      ) : null}
    </section>
  );

  const processPane = (
    <section
      className="system-runtime-panel__section"
      aria-label={i18nText('settings', 'auto.process_monitoring')}
    >
      <Flex align="center" justify="flex-end" gap={12} wrap="wrap">
        <Input
          allowClear
          aria-label={i18nText('settings', 'auto.process_search_placeholder')}
          className="system-runtime-panel__process-search"
          placeholder={i18nText('settings', 'auto.process_search_placeholder')}
          value={processQuery}
          onChange={(event) => setProcessQuery(event.target.value)}
        />
      </Flex>
      <Table<RuntimeProcess>
        columns={columns}
        dataSource={visibleProcesses}
        rowKey="pid"
        size="small"
        pagination={{
          pageSize: PROCESS_TABLE_PAGE_SIZE,
          showSizeChanger: false,
          hideOnSinglePage: true
        }}
        scroll={{ x: 1034, y: 420 }}
        locale={{
          emptyText: i18nText('settings', 'auto.process_empty')
        }}
      />
    </section>
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <div className="system-runtime-panel">
        <Tabs
          activeKey={activeTab}
          onChange={(key) => setActiveTab(key as SystemRuntimeTab)}
          items={[
            {
              key: 'resources',
              label: i18nText('settings', 'auto.resource_monitoring'),
              children: resourcePane
            },
            {
              key: 'processes',
              label: i18nText('settings', 'auto.process_monitoring'),
              children: processPane
            }
          ]}
        />

        {profile.topology.relationship === 'runner_unreachable' ? (
          <Alert
            type="warning"
            showIcon
            icon={<ExclamationCircleOutlined />}
            title={i18nText('settings', 'auto.runner_is_unreachable')}
          />
        ) : null}
      </div>
    </SettingsSectionSurface>
  );
}
