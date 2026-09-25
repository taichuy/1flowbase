import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const echartsMock = vi.hoisted(() => ({
  chart: {
    dispose: vi.fn(),
    resize: vi.fn(),
    setOption: vi.fn()
  },
  init: vi.fn()
}));

const systemRuntimeApi = vi.hoisted(() => ({
  settingsSystemRuntimeQueryKey: ['settings', 'system-runtime'],
  settingsSystemRuntimeProcessesQueryKey: [
    'settings',
    'system-runtime',
    'processes'
  ],
  fetchSettingsSystemRuntimeProfile: vi.fn(),
  fetchSettingsSystemRuntimeProcesses: vi.fn(),
  terminateSettingsSystemRuntimeProcess: vi.fn()
}));

vi.mock('echarts/core', () => ({
  init: echartsMock.init,
  use: vi.fn()
}));
vi.mock('echarts/charts', () => ({
  BarChart: {},
  FunnelChart: {},
  GaugeChart: {},
  LineChart: {},
  PieChart: {},
  RadarChart: {}
}));
vi.mock('echarts/components', () => ({
  GridComponent: {},
  LegendComponent: {},
  RadarComponent: {},
  TitleComponent: {},
  TooltipComponent: {}
}));
vi.mock('echarts/renderers', () => ({
  CanvasRenderer: {}
}));
vi.mock('../../api/system-runtime', () => systemRuntimeApi);

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { SystemRuntimePanel } from '../../components/SystemRuntimePanel';

function runtimeMetrics(
  cpuUsagePercent: number | null,
  capturedAt: string,
  relatedProcessBytes: number,
  relatedProcessCount: number
) {
  return {
    captured_at_unix_milliseconds: Date.parse(capturedAt),
    sample_interval_milliseconds: 2000,
    cpu: {
      availability: 'available',
      scope_kind: 'cgroup',
      usage_percent: cpuUsagePercent,
      logical_count: 8,
      limit_cores: 2
    },
    memory: {
      availability: 'available',
      scope_kind: 'cgroup',
      total_bytes: 4_294_967_296,
      available_bytes: 3_221_225_472,
      used_bytes: 1_073_741_824,
      process_bytes: 268_435_456,
      related_process_bytes: relatedProcessBytes,
      related_process_count: relatedProcessCount,
      cgroup_composition: {
        anonymous_bytes: 536_870_912,
        file_bytes: 268_435_456,
        kernel_bytes: 67_108_864,
        shared_memory_bytes: 16_777_216
      }
    },
    storage: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      mount_point: '/',
      file_system: 'overlay',
      total_bytes: 68_719_476_736,
      available_bytes: 51_539_607_552,
      used_bytes: 17_179_869_184
    },
    network: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      received_bytes_per_second: 2048,
      transmitted_bytes_per_second: 1024
    },
    disk_io: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      read_bytes_per_second: 4096,
      written_bytes_per_second: 8192
    }
  };
}

function warmingRuntimeProfile() {
  const profile = runtimeProfile();
  profile.runtime_targets[0]!.metrics.cpu = {
    ...profile.runtime_targets[0]!.metrics.cpu,
    availability: 'warming_up',
    usage_percent: null
  };
  return profile;
}

function runtimeProfile(sampleIndex = 0) {
  const capturedAt = `2026-07-17T10:00:${String(sampleIndex).padStart(2, '0')}Z`;
  return {
    provider_install_root: '/opt/1flowbase/plugins',
    host_extension_dropin_root: '/opt/1flowbase/plugins/host-extension/dropins',
    related_process_memory_complete: true,
    locale_meta: {
      requested_locale: null,
      resolved_locale: 'zh_Hans',
      source: 'fallback',
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    },
    topology: { relationship: 'same_host' },
    services: {
      api_server: {
        reachable: true,
        service: 'api-server',
        status: 'ok',
        version: '0.2.6',
        host_fingerprint: 'host-1'
      },
      plugin_runner: {
        reachable: true,
        service: 'runtime-extension-host',
        status: 'ok',
        version: '0.2.6',
        host_fingerprint: 'host-1'
      }
    },
    hosts: [
      {
        host_fingerprint: 'host-1',
        platform: {
          os: 'linux',
          arch: 'amd64',
          libc: 'musl',
          rust_target_triple: 'x86_64-unknown-linux-musl'
        },
        cpu: { logical_count: 8 },
        related_process_bytes: 805_306_368,
        related_process_count: 5,
        memory: {
          total_bytes: 4_294_967_296,
          total_gb: 4,
          available_bytes: 3_221_225_472,
          available_gb: 3,
          process_bytes: 268_435_456,
          process_gb: 0.25
        },
        services: ['api-server', 'runtime-extension-host']
      }
    ],
    runtime_targets: [
      {
        target_id: 'api-server',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: runtimeMetrics(12.5 + sampleIndex, capturedAt, 335_544_320, 2)
      },
      {
        target_id: 'runtime-extension-host',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: runtimeMetrics(37.5 + sampleIndex, capturedAt, 469_762_048, 3)
      }
    ]
  };
}

function runtimeProcessList() {
  return {
    process_total: 3,
    processes: [
      {
        pid: 1442117,
        parent_pid: 1,
        name: 'api-server',
        command: './target/debug/api-server',
        user: 'taichuy',
        status: 'sleeping',
        cpu_usage_percent: 0.85,
        memory_bytes: 33_554_432,
        memory_usage_percent: 3.2,
        start_time_unix_seconds: 1_758_160_251,
        terminable: true,
        backend_process: true
      },
      {
        pid: 1619155,
        parent_pid: 1,
        name: 'MainThread',
        command: 'node dsh --profile web',
        user: 'taichuy',
        status: 'running',
        cpu_usage_percent: 0.57,
        memory_bytes: 25_165_824,
        memory_usage_percent: 2.84,
        start_time_unix_seconds: 1_758_217_368,
        terminable: true,
        backend_process: false
      },
      {
        pid: 669,
        parent_pid: 1,
        name: 'systemd-journald',
        command: '/usr/lib/systemd/systemd-journald',
        user: 'root',
        status: 'sleeping',
        cpu_usage_percent: 0.03,
        memory_bytes: 14_155_776,
        memory_usage_percent: 1.35,
        start_time_unix_seconds: 1_756_994_875,
        terminable: false,
        backend_process: false
      }
    ]
  };
}

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const view = render(
    <QueryClientProvider client={queryClient}>
      <App>
        <SystemRuntimePanel />
      </App>
    </QueryClientProvider>
  );
  return { ...view, queryClient };
}

describe('SystemRuntimePanel', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible'
    });
    echartsMock.init.mockReset();
    echartsMock.init.mockReturnValue(echartsMock.chart);
    echartsMock.chart.setOption.mockReset();
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockReset();
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue(
      runtimeProfile()
    );
    systemRuntimeApi.fetchSettingsSystemRuntimeProcesses.mockReset();
    systemRuntimeApi.fetchSettingsSystemRuntimeProcesses.mockResolvedValue(
      runtimeProcessList()
    );
    systemRuntimeApi.terminateSettingsSystemRuntimeProcess.mockReset();
    systemRuntimeApi.terminateSettingsSystemRuntimeProcess.mockResolvedValue({
      pid: 1442117,
      outcome: 'signalled'
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test('AC-1993-004 uses the shared loading state for the initial runtime request', () => {
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockImplementation(
      () => new Promise(() => undefined)
    );

    renderPanel();

    expect(
      screen.getByRole('status', {
        name: 'thinking'
      })
    ).toBeVisible();
  });

  test('ac_001 keeps only the resource monitor section and fills the settings viewport', async () => {
    renderPanel();

    expect(await screen.findByText('资源监控')).toBeInTheDocument();
    expect(screen.queryByText('运行概览')).not.toBeInTheDocument();
    expect(screen.queryByText('运行环境')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '运行目标' })
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('settings-section-surface')).toHaveClass(
      'settings-section-surface--fill'
    );
    expect(
      screen.getByRole('img', { name: '运行资源实时曲线' })
    ).toBeInTheDocument();
  });

  test('ac_010 explains cgroup memory composition without treating it as process RSS', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    expect(screen.getByText('内存构成')).toBeInTheDocument();
    expect(screen.getByText('匿名 512 MB')).toBeInTheDocument();
    expect(screen.getByText('文件 256 MB')).toBeInTheDocument();
    expect(screen.getByText('内核 64.0 MB')).toBeInTheDocument();
    expect(screen.getByText('共享 16.0 MB')).toBeInTheDocument();
  });

  test('ac_011 shows storage usage as used over total bytes beside the mount point', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    expect(
      screen.getByText('/ · 16.0 GB / 64.0 GB')
    ).toBeInTheDocument();
  });

  test('ac_003 polls every two seconds and pauses while hidden', async () => {
    vi.useFakeTimers();
    let sampleIndex = 0;
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockImplementation(
      async () => runtimeProfile(sampleIndex++)
    );
    renderPanel();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalledTimes(2);

    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'hidden'
    });
    fireEvent(document, new Event('visibilitychange'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalledTimes(2);
  });

  test('AC-1993-005 keeps available runtime content visible during a background refresh', async () => {
    let resolveRefresh: ((profile: ReturnType<typeof runtimeProfile>) => void) |
      undefined;
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile
      .mockResolvedValueOnce(runtimeProfile())
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveRefresh = resolve;
          })
      );
    const { queryClient } = renderPanel();

    expect(await screen.findByText('12.5%')).toBeInTheDocument();

    act(() => {
      void queryClient.invalidateQueries({
        queryKey: systemRuntimeApi.settingsSystemRuntimeQueryKey
      });
    });
    await waitFor(() => {
      expect(
        systemRuntimeApi.fetchSettingsSystemRuntimeProfile
      ).toHaveBeenCalledTimes(2);
    });

    expect(screen.getByText('12.5%')).toBeVisible();
    expect(
      screen.queryByRole('status', {
        name: 'thinking'
      })
    ).not.toBeInTheDocument();

    await act(async () => {
      resolveRefresh?.(runtimeProfile(1));
    });
  });

  test('ac_004 monitors the single api-server target without a target selector', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    expect(screen.getByText('12.5%')).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '运行目标' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('37.5%')).not.toBeInTheDocument();
  });

  test('shows zero during CPU warm-up and replaces it with the first sampled value', async () => {
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile
      .mockResolvedValueOnce(warmingRuntimeProfile())
      .mockResolvedValue(runtimeProfile());
    const { queryClient } = renderPanel();

    await screen.findByText('资源监控');
    expect(screen.getByText('0%')).toBeInTheDocument();
    expect(screen.queryByText('采样中')).not.toBeInTheDocument();

    await act(async () => {
      await queryClient.invalidateQueries({
        queryKey: systemRuntimeApi.settingsSystemRuntimeQueryKey
      });
    });
    expect(screen.getByText('12.5%')).toBeInTheDocument();
  });

  test('stops polling after three consecutive collection failures', async () => {
    vi.useFakeTimers();
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockRejectedValue(
      new Error('runtime unavailable')
    );
    renderPanel();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    for (let interval = 0; interval < 5; interval += 1) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2000);
      });
    }

    expect(
      systemRuntimeApi.fetchSettingsSystemRuntimeProfile
    ).toHaveBeenCalledTimes(3);
  });

  test('updates the live chart without recreating its canvas on every sample', async () => {
    vi.useFakeTimers();
    let sampleIndex = 0;
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockImplementation(
      async () => runtimeProfile(sampleIndex++)
    );
    renderPanel();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });

    expect(echartsMock.init).toHaveBeenCalledTimes(1);
    expect(echartsMock.chart.setOption.mock.calls.length).toBeGreaterThan(1);
  });

  test('ac_005 plots throughput charts in KB/s', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find(
          (candidate) =>
            Array.isArray(candidate?.series) &&
            candidate.series[0]?.data?.length === 1
        ) as
        | {
            yAxis?: { name?: string; max?: number };
            series?: Array<{ data?: unknown[] }>;
          }
        | undefined;

      expect(option?.yAxis?.name).toBe('KB/s');
      expect(option?.yAxis).not.toHaveProperty('max');
      expect(option?.series?.[0]?.data).toEqual([2]);
      expect(option?.series?.[1]?.data).toEqual([1]);
    });

    fireEvent.click(screen.getByText('CPU'));
    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find((candidate) => candidate?.yAxis?.name === '%') as
        | { yAxis?: { max?: number } }
        | undefined;

      expect(option?.yAxis?.max).toBe(100);
    });
  });

  test('ac_011 separates environment memory from the related process memory trend', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    expect(screen.getByText('环境内存')).toBeInTheDocument();
    fireEvent.click(screen.getByText('进程内存'));
    expect(screen.getByText('API Server · 2 个进程')).toBeInTheDocument();
    expect(
      screen.queryByText('Runtime Extension Host · 3 个进程')
    ).not.toBeInTheDocument();
    expect(screen.queryByText('同宿主合计 · 5 个进程')).not.toBeInTheDocument();

    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find(
          (candidate) =>
            Array.isArray(candidate?.series) &&
            candidate.series[0]?.name === 'API Server 进程树'
        ) as
        | {
            yAxis?: { name?: string };
            tooltip?: { valueFormatter?: (value: number) => string };
            series?: Array<{
              name?: string;
              data?: unknown[];
            }>;
          }
        | undefined;

      expect(option?.yAxis?.name).toBe('MB');
      expect(option?.tooltip?.valueFormatter?.(320)).toBe('320 MB');
      expect(option?.series?.map((series) => series.name)).toEqual([
        'API Server 进程树',
        'API Server 根进程 RSS'
      ]);
      expect(option?.series?.[0]?.data).toEqual([320]);
      expect(option?.series?.[1]?.data).toEqual([256]);
    });
  });

  test('ac_013 lists observable processes and filters by name', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    fireEvent.click(screen.getByRole('tab', { name: '进程' }));
    const processRegion = within(screen.getByRole('region', { name: '进程' }));

    expect(await processRegion.findByText('api-server')).toBeInTheDocument();
    expect(processRegion.getByText('MainThread')).toBeInTheDocument();
    expect(processRegion.getByText('32.0 MB')).toBeInTheDocument();
    expect(
      processRegion.getByText('./target/debug/api-server')
    ).toBeInTheDocument();

    fireEvent.change(
      processRegion.getByRole('textbox', { name: '请输入进程名' }),
      {
        target: { value: 'mainthread' }
      }
    );

    expect(processRegion.getByText('MainThread')).toBeInTheDocument();
    expect(processRegion.queryByText('api-server')).not.toBeInTheDocument();
  });

  test('ac_014 terminates only the processes the current user may signal', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    fireEvent.click(screen.getByRole('tab', { name: '进程' }));
    const processRegion = within(screen.getByRole('region', { name: '进程' }));
    await processRegion.findByText('api-server');

    const terminateButtons = processRegion.getAllByRole('button', {
      name: '结束'
    });
    expect(terminateButtons).toHaveLength(3);
    expect(terminateButtons[0]).toBeEnabled();
    expect(terminateButtons[2]).toBeDisabled();

    fireEvent.click(terminateButtons[0]);

    await waitFor(() => {
      expect(
        systemRuntimeApi.terminateSettingsSystemRuntimeProcess
      ).toHaveBeenCalledWith(1442117);
    });
  });

  test('ac_015 renders the backend process tree inside the resource monitor', async () => {
    renderPanel();

    await screen.findByText('资源监控');

    expect(
      screen.queryByRole('tab', { name: '进程树' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('MainThread')).not.toBeInTheDocument();
    expect(screen.queryByText('systemd-journald')).not.toBeInTheDocument();

    const treePanel = await waitFor(() => {
      const element = document.querySelector(
        '.system-runtime-panel__process-tree-panel'
      );
      expect(element).not.toBeNull();
      return element as HTMLElement;
    });
    expect(treePanel).toHaveTextContent('api-server');
    expect(treePanel).not.toHaveTextContent('1442117');
    expect(treePanel).not.toHaveTextContent('CPU');

    const detailPanel = document.querySelector(
      '.system-runtime-panel__process-detail-panel'
    );
    expect(detailPanel).not.toBeNull();
    const detail = within(detailPanel as HTMLElement);
    expect(detail.getByText('api-server')).toBeInTheDocument();
    expect(detail.getByText('1442117')).toBeInTheDocument();
    expect(
      detail.getByText('./target/debug/api-server')
    ).toBeInTheDocument();
    expect(detail.getByText('32.0 MB')).toBeInTheDocument();
    expect(
      detail.getByRole('button', { name: /结\s*束/u })
    ).toBeInTheDocument();
  });
});
