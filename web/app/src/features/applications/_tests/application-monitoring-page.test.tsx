import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const echartsMock = vi.hoisted(() => ({
  chart: {
    dispose: vi.fn(),
    resize: vi.fn(),
    setOption: vi.fn()
  },
  init: vi.fn()
}));

const runtimeApi = vi.hoisted(() => ({
  applicationRunMonitoringReportQueryKey: (
    applicationId: string,
    input?: {
      timeRangeDays?: number | null;
      bucket?: 'hour' | 'day' | 'week' | 'month';
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'monitoring',
      'run-metrics',
      input?.timeRangeDays ?? 7,
      input?.bucket ?? 'day'
    ] as const,
  applicationRuntimeActivityQueryKey: (applicationId: string) =>
    [
      'applications',
      applicationId,
      'runtime',
      'monitoring',
      'runtime-activity'
    ] as const,
  fetchApplicationRuntimeActivity: vi.fn(),
  fetchApplicationRunMonitoringReport: vi.fn()
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
vi.mock('../api/runtime', () => runtimeApi);

import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../state/auth-store';
import { ApplicationMonitoringPage } from '../pages/ApplicationMonitoringPage';

function monitoringReport() {
  return {
    meta: {
      started_from: '2026-05-01T00:00:00Z',
      started_to: null,
      bucket: 'day',
      slow_run_threshold_ms: 30000
    },
    overview: {
      total_count: 12,
      success_count: 9,
      failed_count: 2,
      cancelled_count: 1,
      success_rate: 0.75,
      failed_rate: 0.1667,
      running_count_included: false
    },
    duration: {
      duration_recorded_count: 12,
      avg_duration_ms: 2400,
      p50_duration_ms: 1800,
      p95_duration_ms: 32000,
      slow_run_rate: 0.08
    },
    tokens: {
      total_tokens_sum: 5600,
      input_tokens_sum: 4200,
      output_tokens_sum: 1400,
      input_cache_hit_tokens_sum: 900,
      avg_tokens_per_run: 466.7,
      token_recorded_count: 12
    },
    tokens_comparison: {
      previous_total_tokens_sum: 0,
      previous_run_count: 0,
      previous_avg_tokens_per_run: 0,
      token_change_rate: 5600,
      run_count_change_rate: 0.3333,
      avg_tokens_per_run_change_rate: 0.1667,
      traffic_effect: 1.3333,
      cost_per_run_effect: 1.1667
    },
    tool_callbacks: {
      total_tool_callback_count: 7,
      avg_tool_callback_count: 0.58,
      runs_with_tool_callback: 3
    },
    nodes: {
      avg_unique_node_count: 4.2,
      max_unique_node_count: 8
    },
    concurrency: {
      peak_concurrency: 3
    },
    tokens_trend: [
      {
        bucket_start: '2026-05-01T00:00:00Z',
        run_count: 4,
        total_tokens: 1200,
        input_tokens: 900,
        output_tokens: 300,
        input_cache_hit_tokens: 120
      },
      {
        bucket_start: '2026-05-02T00:00:00Z',
        run_count: 8,
        total_tokens: 4400,
        input_tokens: 3300,
        output_tokens: 1100,
        input_cache_hit_tokens: 780
      }
    ],
    protocols: [
      {
        protocol: 'default',
        request_count: 7,
        success_rate: 0.85,
        avg_duration_ms: 1800,
        total_tokens: 2600
      },
      {
        protocol: 'openai-responses-v1',
        request_count: 5,
        success_rate: 0.6,
        avg_duration_ms: 3200,
        total_tokens: 3000
      }
    ],
    sources: [
      {
        invocation_source: 'debug',
        request_count: 4,
        success_rate: 0.85,
        total_tokens: 1600
      },
      {
        invocation_source: 'agent_flow_api',
        request_count: 3,
        success_rate: 0.6,
        total_tokens: 1800
      },
      {
        invocation_source: 'workflow_http',
        request_count: 3,
        success_rate: 1,
        total_tokens: 1200
      },
      {
        invocation_source: 'workflow_schedule',
        request_count: 2,
        success_rate: 1,
        total_tokens: 1000
      },
      {
        invocation_source: 'assistant',
        request_count: 1,
        success_rate: 1,
        total_tokens: 600
      }
    ],
    authorized_accounts: [
      {
        authorized_account: 'root',
        request_count: 7,
        total_tokens: 2600,
        avg_duration_ms: 1800,
        failed_count: 0
      }
    ],
    external_users: [
      {
        external_user: 'customer-1',
        request_count: 5,
        total_tokens: 3000,
        avg_duration_ms: 3200,
        failed_count: 2
      }
    ],
    api_keys: [
      {
        api_key_id: 'key-1',
        api_key_name_snapshot: 'Customer API',
        request_count: 5,
        total_tokens: 3000,
        avg_duration_ms: 3200,
        failed_count: 2
      }
    ],
    external_conversations: [
      {
        external_conversation_id: 'conversation-1',
        request_count: 5,
        total_tokens: 3000,
        avg_duration_ms: 3200,
        failed_count: 2
      }
    ],
    slowest_runs: [
      {
        flow_run_id: 'run-2',
        title: '最慢运行',
        status: 'failed',
        started_at: '2026-05-02T10:00:00Z',
        finished_at: '2026-05-02T10:00:40Z',
        duration_ms: 40000,
        total_tokens: 3000
      }
    ],
    high_token_runs: [
      {
        flow_run_id: 'run-2',
        title: '最慢运行',
        status: 'failed',
        started_at: '2026-05-02T10:00:00Z',
        finished_at: '2026-05-02T10:00:40Z',
        duration_ms: 40000,
        total_tokens: 3000
      }
    ]
  };
}

function hourlyMonitoringReport() {
  return {
    ...monitoringReport(),
    meta: {
      ...monitoringReport().meta,
      bucket: 'hour' as const
    },
    tokens_trend: [
      {
        bucket_start: '2026-05-01T08:00:00Z',
        run_count: 4,
        total_tokens: 1200,
        input_tokens: 900,
        output_tokens: 300,
        input_cache_hit_tokens: 120
      },
      {
        bucket_start: '2026-05-01T09:00:00Z',
        run_count: 8,
        total_tokens: 4400,
        input_tokens: 3300,
        output_tokens: 1100,
        input_cache_hit_tokens: 780
      }
    ]
  };
}

function runtimeActivity() {
  return {
    meta: {
      application_id: 'app-1',
      scope: 'current_instance',
      storage: 'memory',
      instance_started_at: '2026-05-30T00:00:00Z',
      snapshot_at: '2026-05-30T00:01:00Z'
    },
    active: {
      total: 6,
      http_requests: 1,
      sse_connections: 2,
      websocket_connections: 0,
      application_executions: 1,
      tool_calls: 1,
      model_requests: 1,
      waiting: null
    },
    peaks: {
      process_peak_concurrency: 9,
      recent_peak_concurrency: 6
    },
    rolling_minute: {
      completed: 20,
      failed: 2,
      cancelled: 1,
      disconnected: 3
    },
    windows: {
      one_minute: {
        window_seconds: 60,
        completed: 20,
        failed: 2,
        cancelled: 1,
        disconnected: 3,
        peak_concurrency: 6,
        failure_rate: 0.087,
        disconnect_rate: 0.12,
        throughput_per_minute: 20
      },
      five_minutes: {
        window_seconds: 300,
        completed: 80,
        failed: 3,
        cancelled: 1,
        disconnected: 4,
        peak_concurrency: 6,
        failure_rate: 0.036,
        disconnect_rate: 0.046,
        throughput_per_minute: 16
      },
      fifteen_minutes: {
        window_seconds: 900,
        completed: 210,
        failed: 4,
        cancelled: 1,
        disconnected: 5,
        peak_concurrency: 9,
        failure_rate: 0.019,
        disconnect_rate: 0.023,
        throughput_per_minute: 14
      }
    },
    health: {
      state: 'slow',
      failure_rate_1m: 0.087,
      failure_rate_5m: 0.036,
      failure_rate_15m: 0.019,
      disconnect_rate_5m: 0.046,
      slow_ratio: 1,
      active_pressure: 1,
      throughput_5m_per_minute: 16,
      throughput_15m_per_minute: 14,
      throughput_trend: 'rising',
      failure_trend: 0.017
    },
    age_distribution: {
      under_5s: 3,
      from_5s_to_30s: 2,
      from_30s_to_120s: 1,
      over_120s: 0
    },
    long_connection_age_distribution: {
      under_5s: 1,
      from_5s_to_30s: 1,
      from_30s_to_120s: 0,
      over_120s: 0
    },
    pressure: {
      slow_active_executions: 1,
      execution_slots_used: null,
      execution_slots_limit: null
    },
    resources: {
      process_rss_bytes: null
    }
  };
}

describe('ApplicationMonitoringPage', () => {
  beforeEach(async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'en_US');
    await appI18n.changeLanguage('en_US');
    resetAuthStore();
    vi.clearAllMocks();
    echartsMock.init.mockReturnValue(echartsMock.chart);
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue(
      monitoringReport()
    );
    runtimeApi.fetchApplicationRuntimeActivity.mockResolvedValue(
      runtimeActivity()
    );
  });

  test('renders backend aggregated monitoring report and charts', async () => {
    render(
      <AppProviders>
        <ApplicationMonitoringPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('12')).toBeInTheDocument();
    expect(screen.getByText('75.0%')).toBeInTheDocument();
    expect(screen.queryByText('运行中数未包含')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Running statistical caliber' })
    ).toBeInTheDocument();
    expect(screen.getByText('openai-responses-v1')).toBeInTheDocument();
    expect(screen.getByText('Runtime activity')).toBeInTheDocument();
    expect(screen.getByText('Slow')).toBeInTheDocument();
    expect(screen.getByText('5m failure rate')).toBeInTheDocument();
    expect(screen.getByText('New tokens')).toBeInTheDocument();
    expect(screen.getAllByText('5.6K').length).toBeGreaterThan(0);
    expect(screen.getByText('Input tokens')).toBeInTheDocument();
    expect(screen.getByText('4.2K')).toBeInTheDocument();
    expect(screen.getByText('Output tokens')).toBeInTheDocument();
    expect(screen.getByText('1.4K')).toBeInTheDocument();
    expect(screen.getByText('Cache-hit tokens')).toBeInTheDocument();
    expect(screen.getByText('900')).toBeInTheDocument();
    expect(screen.queryByText('+560,000.0%')).not.toBeInTheDocument();
    const runtimeActivityPosition = screen
      .getByText('Runtime activity')
      .compareDocumentPosition(
        screen.getByRole('radio', { name: 'past 7 days' })
      );
    expect(runtimeActivityPosition & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(
      screen.getByRole('button', { name: 'Runtime activity' })
    ).toBeInTheDocument();
    expect(
      screen.queryByText('Current service instance realtime data')
    ).not.toBeInTheDocument();
    expect(screen.getByText('SSE')).toBeInTheDocument();
    expect(screen.getByText('Model requests')).toBeInTheDocument();
    expect(screen.getByText('customer-1')).toBeInTheDocument();
    expect(screen.getByText('Customer API')).toBeInTheDocument();
    expect(screen.getByText('Assistant')).toBeInTheDocument();
    expect(screen.getAllByText('最慢运行').length).toBeGreaterThan(0);
    expect(echartsMock.chart.setOption).toHaveBeenCalled();
    const tokenTrendOption = echartsMock.chart.setOption.mock.calls[0]?.[0];
    expect(tokenTrendOption.series).toHaveLength(4);
    expect(tokenTrendOption.series[0]).toMatchObject({
      name: 'total tokens',
      type: 'line',
      data: [1200, 4400]
    });
    expect(tokenTrendOption.series[0]).not.toHaveProperty('stack');
    expect(tokenTrendOption.series[1]).toMatchObject({
      name: 'Input tokens',
      type: 'line',
      data: [900, 3300]
    });
    expect(tokenTrendOption.series[1]).not.toHaveProperty('stack');
    expect(tokenTrendOption.series[2]).toMatchObject({
      name: 'Output tokens',
      type: 'line',
      data: [300, 1100]
    });
    expect(tokenTrendOption.series[2]).not.toHaveProperty('stack');
    expect(tokenTrendOption.series[3]).toMatchObject({
      name: 'Cache-hit tokens',
      type: 'line',
      data: [120, 780]
    });
    expect(tokenTrendOption.series[3]).not.toHaveProperty('stack');
  });

  test('formats token metric cards with K M B suffixes', async () => {
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue({
      ...monitoringReport(),
      tokens: {
        ...monitoringReport().tokens,
        total_tokens_sum: 11_739_169,
        input_tokens_sum: 11_290_226,
        output_tokens_sum: 366_440,
        input_cache_hit_tokens_sum: 7_874_262
      },
      tokens_comparison: {
        ...monitoringReport().tokens_comparison,
        previous_total_tokens_sum: 0,
        token_change_rate: 11_739_169
      }
    });

    render(
      <AppProviders>
        <ApplicationMonitoringPage applicationId="app-1" />
      </AppProviders>
    );

    expect(
      await screen.findByText('Total amount of tokens')
    ).toBeInTheDocument();
    expect(screen.getAllByText('11.7M')).toHaveLength(2);
    expect(screen.getByText('Input tokens')).toBeInTheDocument();
    expect(screen.getByText('11.3M')).toBeInTheDocument();
    expect(screen.getByText('Output tokens')).toBeInTheDocument();
    expect(screen.getByText('366.4K')).toBeInTheDocument();
    expect(screen.getByText('Cache-hit tokens')).toBeInTheDocument();
    expect(screen.getByText('7.9M')).toBeInTheDocument();
    expect(screen.queryByText('11,739,169')).not.toBeInTheDocument();
  });

  test('refreshes the report when time range changes', async () => {
    render(
      <AppProviders>
        <ApplicationMonitoringPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('12')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: 'past 4 weeks' }));

    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunMonitoringReport
      ).toHaveBeenLastCalledWith('app-1', {
        timeRangeDays: 28,
        bucket: 'day'
      });
    });
    expect(runtimeApi.fetchApplicationRuntimeActivity).toHaveBeenCalledTimes(1);
  });

  test('formats token trend buckets as hours for the past 24 hours range', async () => {
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue(
      hourlyMonitoringReport()
    );

    render(
      <AppProviders>
        <ApplicationMonitoringPage applicationId="app-1" />
      </AppProviders>
    );

    fireEvent.click(
      await screen.findByRole('radio', { name: 'past 24 hours' })
    );

    await waitFor(() => {
      const option = echartsMock.chart.setOption.mock.calls
        .map((call) => call[0])
        .find((candidate) => candidate?.xAxis?.data);
      expect(option.xAxis.data).toEqual(
        expect.arrayContaining([
          expect.stringMatching(/\d{1,2}:\d{2}/),
          expect.stringMatching(/\d{1,2}:\d{2}/)
        ])
      );
    });
  });
});
