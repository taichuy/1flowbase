import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
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
  LineChart: {}
}));
vi.mock('echarts/components', () => ({
  GridComponent: {},
  LegendComponent: {},
  TooltipComponent: {}
}));
vi.mock('echarts/renderers', () => ({
  CanvasRenderer: {}
}));
vi.mock('../../api/system-runtime', () => systemRuntimeApi);

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { SystemRuntimePanel } from '../../components/SystemRuntimePanel';

function runtimeMetrics(cpuUsagePercent: number, capturedAt: string) {
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
      process_bytes: 268_435_456
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

function runtimeProfile(sampleIndex = 0) {
  const capturedAt = `2026-07-17T10:00:${String(sampleIndex).padStart(2, '0')}Z`;
  return {
    provider_install_root: '/opt/1flowbase/plugins',
    host_extension_dropin_root: '/opt/1flowbase/plugins/host-extension/dropins',
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
        metrics: runtimeMetrics(12.5 + sampleIndex, capturedAt)
      },
      {
        target_id: 'plugin-runner',
        reachable: true,
        host_fingerprint: 'host-1',
        metrics: runtimeMetrics(37.5 + sampleIndex, capturedAt)
      }
    ]
  };
}

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <SystemRuntimePanel />
    </QueryClientProvider>
  );
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
    expect(within(environmentSection!).queryByText('当前语言')).toBeNull();
    expect(within(environmentSection!).queryByText('回退语言')).toBeNull();
    expect(within(environmentSection!).queryByText('支持语言')).toBeNull();
    expect(
      within(environmentSection!).getByText('进程内存')
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
});
