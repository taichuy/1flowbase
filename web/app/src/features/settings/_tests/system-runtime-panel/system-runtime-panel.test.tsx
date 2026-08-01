import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
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
  fetchSettingsSystemRuntimeProfile: vi.fn()
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
        service: 'plugin-runner',
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
        services: ['api-server', 'plugin-runner']
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
        target_id: 'plugin-runner',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: runtimeMetrics(37.5 + sampleIndex, capturedAt, 469_762_048, 3)
      }
    ]
  };
}

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const rendered = render(
    <QueryClientProvider client={queryClient}>
      <SystemRuntimePanel />
    </QueryClientProvider>
  );
  return { ...rendered, queryClient };
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
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test('ac_001 fills the settings viewport so the surface body owns scrolling', async () => {
    renderPanel();

    expect(await screen.findByText('运行概览')).toBeInTheDocument();
    expect(screen.getByText('资源监控')).toBeInTheDocument();
    const overviewSection = screen.getByText('运行概览').closest('section');
    const environmentSection = screen.getByText('运行环境').closest('section');
    const monitorSection = screen.getByText('资源监控').closest('section');

    expect(overviewSection).not.toBeNull();
    expect(environmentSection).not.toBeNull();
    expect(monitorSection).not.toBeNull();
    expect(
      overviewSection!.compareDocumentPosition(environmentSection!) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
    expect(
      environmentSection!.compareDocumentPosition(monitorSection!) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
    expect(
      within(environmentSection!).getByRole('combobox', { name: '运行目标' })
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).queryByText('当前语言')
    ).not.toBeInTheDocument();
    expect(
      within(environmentSection!).queryByText('回退语言')
    ).not.toBeInTheDocument();
    expect(
      within(environmentSection!).queryByText('支持语言')
    ).not.toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('相关进程内存')
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('插件安装路径')
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('宿主扩展路径')
    ).toBeInTheDocument();
    expect(
      within(monitorSection!).queryByRole('combobox', { name: '运行目标' })
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('settings-section-surface')).toHaveClass(
      'settings-section-surface--fill'
    );
    expect(
      screen.getByRole('combobox', { name: '运行目标' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('img', { name: '运行资源实时曲线' })
    ).toBeInTheDocument();
  });

  test('ac_009 shows the host total with the selected target process breakdown', async () => {
    renderPanel();

    const environmentSection = (await screen.findByText('运行环境')).closest(
      'section'
    );
    expect(environmentSection).not.toBeNull();
    expect(within(environmentSection!).getByText('768 MB')).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('5 个进程')
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('当前目标 320 MB · 2 个进程')
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('根进程 RSS 256 MB')
    ).toBeInTheDocument();

    fireEvent.mouseDown(
      within(environmentSection!).getByRole('combobox', {
        name: '运行目标'
      })
    );
    fireEvent.click(
      await screen.findByRole('option', { name: 'Plugin Runner' })
    );

    expect(within(environmentSection!).getByText('768 MB')).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('当前目标 448 MB · 3 个进程')
    ).toBeInTheDocument();
    expect(
      within(environmentSection!).getByText('根进程 RSS 256 MB')
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

  test('shows related process memory as partial when a runtime target is unreachable', async () => {
    const profile = runtimeProfile();
    profile.related_process_memory_complete = false;
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue(
      profile
    );

    renderPanel();

    expect(await screen.findByText('部分运行目标不可用')).toBeInTheDocument();
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

  test('ac_004 switches the live metrics target without merging same-host services', async () => {
    renderPanel();

    await screen.findByText('资源监控');
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '运行目标' }));
    fireEvent.click(
      await screen.findByRole('option', { name: 'Plugin Runner' })
    );
    expect(screen.getByText('37.5%')).toBeInTheDocument();
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
    expect(screen.getByText('Plugin Runner · 3 个进程')).toBeInTheDocument();
    expect(screen.getByText('同宿主合计 · 5 个进程')).toBeInTheDocument();

    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find(
          (candidate) =>
            Array.isArray(candidate?.series) &&
            candidate.series[0]?.name === '同宿主相关进程合计'
        ) as
        | {
            yAxis?: { name?: string };
            series?: Array<{
              name?: string;
              data?: unknown[];
            }>;
          }
        | undefined;

      expect(option?.yAxis?.name).toBe('MB');
      expect(option?.series?.map((series) => series.name)).toEqual([
        '同宿主相关进程合计',
        'API Server 进程树',
        'API Server 根进程 RSS'
      ]);
      expect(option?.series?.[0]?.data).toEqual([768]);
      expect(option?.series?.[1]?.data).toEqual([320]);
      expect(option?.series?.[2]?.data).toEqual([256]);
    });

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '运行目标' }));
    fireEvent.click(
      await screen.findByRole('option', { name: 'Plugin Runner' })
    );
    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find(
          (candidate) =>
            Array.isArray(candidate?.series) &&
            candidate.series[1]?.name === 'Plugin Runner 进程树'
        ) as { series?: Array<{ name?: string }> } | undefined;

      expect(option?.series?.map((series) => series.name)).toEqual([
        '同宿主相关进程合计',
        'Plugin Runner 进程树',
        'Plugin Runner 根进程 RSS'
      ]);
    });
  });

  test('ac_012 leaves a gap in the host process total when collection is incomplete', async () => {
    const profile = runtimeProfile();
    profile.related_process_memory_complete = false;
    systemRuntimeApi.fetchSettingsSystemRuntimeProfile.mockResolvedValue(
      profile
    );
    renderPanel();

    await screen.findByText('资源监控');
    fireEvent.click(screen.getByText('进程内存'));

    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .reverse()
        .find(
          (candidate) =>
            Array.isArray(candidate?.series) &&
            candidate.series[0]?.name === '同宿主相关进程合计'
        ) as { series?: Array<{ data?: unknown[] }> } | undefined;

      expect(option?.series?.[0]?.data).toEqual([null]);
      expect(option?.series?.[1]?.data).toEqual([320]);
      expect(option?.series?.[2]?.data).toEqual([256]);
    });
  });
});
