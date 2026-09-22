import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const echartsMock = vi.hoisted(() => ({
  chart: {
    on: vi.fn(),
    off: vi.fn(),
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
      from?: string;
      to?: string;
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
      input?.bucket ?? 'day',
      input?.from,
      input?.to
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
import { ApplicationStatisticsPage } from '../pages/ApplicationStatisticsPage';

function monitoringReport() {
  return {
    costs: { total_cost: 1.25, cost_recorded_count: 10, cost_missing_count: 2 },
    models: [
      {
        requested_model_id: 'model-a',
        task_count: 12,
        total_tokens: 5600,
        total_cost: 1.25
      }
    ],
    users: [
      {
        user_id: 'user-a',
        name: 'Alice',
        task_count: 12,
        total_tokens: 5600,
        total_cost: 1.25
      }
    ],
    meta: {
      started_from: '2026-05-01T00:00:00Z',
      started_to: '2026-05-03T00:00:00Z',
      bucket: 'day',
      slow_run_threshold_ms: 30000
    },
    overview: {
      total_count: 12,
      running_count: 0,
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
        bucket_end: '2026-05-02T00:00:00Z',
        total_cost: 0.25,
        avg_duration_ms: 1200,
        run_count: 4,
        total_tokens: 1200,
        input_tokens: 900,
        output_tokens: 300,
        input_cache_hit_rate: 0.1176470588,
        input_cache_hit_tokens: 120
      },
      {
        bucket_start: '2026-05-02T00:00:00Z',
        bucket_end: '2026-05-03T00:00:00Z',
        total_cost: 1,
        avg_duration_ms: 3000,
        run_count: 8,
        total_tokens: 4400,
        input_tokens: 3300,
        output_tokens: 1100,
        input_cache_hit_rate: 0.1911764706,
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
        bucket_end: '2026-05-01T09:00:00Z',
        total_cost: 0.25,
        avg_duration_ms: 1200,
        run_count: 4,
        total_tokens: 1200,
        input_tokens: 900,
        output_tokens: 300,
        input_cache_hit_rate: 0.1176470588,
        input_cache_hit_tokens: 120
      },
      {
        bucket_start: '2026-05-01T09:00:00Z',
        bucket_end: '2026-05-01T10:00:00Z',
        total_cost: 1,
        avg_duration_ms: 3000,
        run_count: 8,
        total_tokens: 4400,
        input_tokens: 3300,
        output_tokens: 1100,
        input_cache_hit_rate: 0.1911764706,
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

describe('ApplicationStatisticsPage', () => {
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

  test('shows task metrics, server distributions and matching log drilldowns', async () => {
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    expect(
      await screen.findByText(
        'Succeeded 9 · Failed 2 · Cancelled 1 · Running 0'
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText('Succeeded 9 · Failed 2 · Cancelled 1 · Running 0')
    ).toBeInTheDocument();
    expect(
      screen.getByText('Costs include missing records for 2 root tasks')
    ).toBeInTheDocument();
    const userLink = screen.getByRole('link', { name: 'Alice' });
    expect(userLink).toHaveAttribute(
      'href',
      '/applications/app-1/logs?started_from=2026-05-01T00%3A00%3A00Z&started_to=2026-05-03T00%3A00%3A00Z&user_id=user-a'
    );
    expect(screen.getByRole('link', { name: 'model-a' })).toHaveAttribute(
      'href',
      expect.stringContaining('requested_model_id=model-a')
    );
    expect(
      screen.getByRole('link', { name: 'View task logs' })
    ).toHaveAttribute('href', expect.stringContaining('started_to='));
    expect(screen.queryByText('Customer API')).not.toBeInTheDocument();
    const trend = echartsMock.chart.setOption.mock.calls
      .map((call) => call[0])
      .find((option) => option.xAxis);
    expect(
      trend.series.map((series: { data: unknown }) => series.data)
    ).toEqual([
      [900, 3300],
      [300, 1100],
      [120, 780],
      [11.76, 19.12]
    ]);
    expect(trend.series[3]).toMatchObject({
      name: 'Cache hit rate',
      yAxisIndex: 1,
      connectNulls: false
    });
    expect(trend.yAxis[1]).toMatchObject({ min: 0, max: 100 });
  });

  test('leaves undefined cache rates as gaps rather than zero percent', async () => {
    const report = monitoringReport();
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue({
      ...report,
      tokens_trend: [
        {
          ...report.tokens_trend[0],
          input_tokens: 0,
          input_cache_hit_tokens: 0,
          input_cache_hit_rate: null
        }
      ]
    });
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    await screen.findByRole('link', { name: 'Alice' });
    const trend = echartsMock.chart.setOption.mock.calls
      .map((call) => call[0])
      .find((option) => option.xAxis);
    expect(trend.series[3].data).toEqual([null]);
    expect(trend.series[3].connectNulls).toBe(false);
  });

  test('changes model distribution without changing user distribution', async () => {
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    await screen.findByRole('link', { name: 'Alice' });
    fireEvent.click(screen.getAllByRole('radio', { name: 'Recorded cost' })[0]);
    const rings = echartsMock.chart.setOption.mock.calls
      .map((call) => call[0])
      .filter((option) => option.series[0]?.type === 'pie')
      .slice(-2);
    expect(rings.map((option) => option.series[0].data[0].value)).toEqual([
      1.25, 5600
    ]);
  });

  test('keeps missing cost distinct from zero and missing identities drillable', async () => {
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue({
      ...monitoringReport(),
      costs: {
        total_cost: null,
        cost_recorded_count: 0,
        cost_missing_count: 12
      },
      users: [
        {
          user_id: null,
          name: null,
          task_count: 12,
          total_tokens: 5600,
          total_cost: null
        }
      ],
      models: [
        {
          requested_model_id: null,
          task_count: 12,
          total_tokens: 5600,
          total_cost: null
        }
      ]
    });
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    expect((await screen.findAllByText('—')).length).toBeGreaterThan(0);
    expect(screen.getByRole('link', { name: 'Unknown user' })).toHaveAttribute(
      'href',
      expect.stringContaining('missing_user=true')
    );
    expect(
      screen.getByRole('link', { name: 'Unspecified model' })
    ).toHaveAttribute('href', expect.stringContaining('missing_model=true'));
  });

  test('switches distributions and trends to backend cost values', async () => {
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    await screen.findByText('Succeeded 9 · Failed 2 · Cancelled 1 · Running 0');
    const costOptions = screen.getAllByRole('radio', { name: 'Recorded cost' });
    fireEvent.click(costOptions[0]);
    fireEvent.click(costOptions[2]);
    await waitFor(() => {
      const options = echartsMock.chart.setOption.mock.calls.map(
        (call) => call[0]
      );
      expect(
        options.some(
          (option) =>
            option.xAxis && JSON.stringify(option.series[0].data) === '[0.25,1]'
        )
      ).toBe(true);
      expect(
        options.some(
          (option) =>
            option.series[0]?.type === 'pie' &&
            option.series[0].data[0].value === 1.25
        )
      ).toBe(true);
    });
  });

  test('retains every measure in tables and does not invent zero-cost ring sectors', async () => {
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue({
      ...monitoringReport(),
      models: [
        {
          requested_model_id: 'model-a',
          task_count: 12,
          total_tokens: 5600,
          total_cost: 0
        }
      ],
      users: [
        {
          user_id: 'user-a',
          name: 'Alice',
          task_count: 12,
          total_tokens: 5600,
          total_cost: null
        }
      ]
    });
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );
    await screen.findByRole('link', { name: 'Alice' });
    for (const table of screen.getAllByRole('table')) {
      expect(
        within(table).getByRole('columnheader', { name: 'Root tasks' })
      ).toBeInTheDocument();
      expect(
        within(table).getByRole('columnheader', { name: 'Tokens' })
      ).toBeInTheDocument();
      expect(
        within(table).getByRole('columnheader', { name: 'Recorded cost' })
      ).toBeInTheDocument();
    }
    fireEvent.click(screen.getAllByRole('radio', { name: 'Recorded cost' })[0]);
    fireEvent.click(screen.getAllByRole('radio', { name: 'Recorded cost' })[1]);
    expect(
      screen.getAllByText('No recorded positive values for this metric')
    ).toHaveLength(2);
    expect(screen.getByRole('link', { name: 'Alice' })).toBeInTheDocument();
    expect(
      screen.queryByRole('img', { name: 'User distribution' })
    ).not.toBeInTheDocument();
  });

  test('keeps realtime activity in the monitoring section', async () => {
    render(
      <AppProviders>
        <ApplicationMonitoringPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('Runtime activity')).toBeInTheDocument();
    expect(await screen.findByText('Slow')).toBeInTheDocument();
    expect(screen.getByText('5m failure rate')).toBeInTheDocument();
    expect(screen.queryByText('Total runs')).not.toBeInTheDocument();
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
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(
      await screen.findByText('Total amount of tokens')
    ).toBeInTheDocument();
    expect(screen.getAllByText('11.7M')).toHaveLength(1);
    expect(
      screen.getByText(
        /Input tokens: 11.3M.*Output tokens: 366.4K.*Cache-hit tokens: 7.9M/
      )
    ).toBeInTheDocument();
    expect(screen.queryByText('11,739,169')).not.toBeInTheDocument();
  });

  test('refreshes the report when time range changes', async () => {
    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(
      await screen.findByText(
        'Succeeded 9 · Failed 2 · Cancelled 1 · Running 0'
      )
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: 'past 4 weeks' }));

    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunMonitoringReport
      ).toHaveBeenLastCalledWith('app-1', {
        timeRangeDays: 28,
        bucket: 'day'
      });
    });
  });

  test('formats token trend buckets as hours for the past 24 hours range', async () => {
    runtimeApi.fetchApplicationRunMonitoringReport.mockResolvedValue(
      hourlyMonitoringReport()
    );

    render(
      <AppProviders>
        <ApplicationStatisticsPage applicationId="app-1" />
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
