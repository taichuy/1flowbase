import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import DatabaseOutlined from '@ant-design/icons/es/icons/DatabaseOutlined';
import DollarOutlined from '@ant-design/icons/es/icons/DollarOutlined';
import ClockCircleOutlined from '@ant-design/icons/es/icons/ClockCircleOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Card,
  DatePicker,
  Empty,
  Radio,
  Result,
  Select,
  Space,
  Spin,
  Table,
  Typography,
  theme
} from 'antd';
import { useState } from 'react';
import dayjs from 'dayjs';
import { useTranslation } from 'react-i18next';
import { formatTokenCount } from '../../../../shared/i18n/format';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';
import {
  applicationRunMonitoringReportQueryKey,
  fetchApplicationRunMonitoringReport,
  type ApplicationRunMonitoringBucket
} from '../../api/runtime';
import {
  formatDuration,
  formatInteger,
  formatTrendBucket,
  getMonitoringBucket,
  monitoringTimeRangeOptions,
  type MonitoringTimeRange
} from '../../lib/application-monitoring-format';
import {
  statisticsBucketFilters,
  statisticsLogsHref
} from '../../lib/statistics-log-filters';
import { ApplicationMonitoringChart } from '../monitoring/ApplicationMonitoringChart';
import './application-task-statistics.css';

type Metric = 'task_count' | 'total_tokens' | 'total_cost';
type Usage = {
  task_count: number;
  total_tokens: number;
  total_cost: number | null;
};
const formatCost = (value: number | null) =>
  value === null
    ? '—'
    : `${value.toLocaleString(undefined, { maximumFractionDigits: 6 })} $`;

export function ApplicationTaskStatistics({
  applicationId
}: {
  applicationId: string;
}) {
  const { t } = useTranslation('applications');
  const { token } = theme.useToken();
  const [timeRangeDays, setTimeRangeDays] = useState<MonitoringTimeRange>(7);
  const [custom, setCustom] = useState(false);
  const [range, setRange] = useState<{ from?: string; to?: string }>({});
  const [bucket, setBucket] = useState<ApplicationRunMonitoringBucket | 'auto'>(
    'auto'
  );
  const rangeDays =
    custom && range.from && range.to
      ? (Date.parse(range.to) - Date.parse(range.from)) / 86_400_000
      : timeRangeDays;
  const resolvedBucket =
    bucket === 'auto' ? getMonitoringBucket(rangeDays) : bucket;
  const [distributionMetrics, setDistributionMetrics] = useState<
    Record<'models' | 'users', Metric>
  >({ models: 'total_tokens', users: 'total_tokens' });
  const [trendMetric, setTrendMetric] = useState<Metric | 'avg_duration_ms'>(
    'total_tokens'
  );
  const input = {
    timeRangeDays,
    bucket: resolvedBucket,
    ...(custom ? range : {})
  };
  const query = useQuery({
    queryKey: applicationRunMonitoringReportQueryKey(applicationId, input),
    queryFn: () => fetchApplicationRunMonitoringReport(applicationId, input),
    placeholderData: (previousData, previousQuery) =>
      previousQuery?.queryKey[1] === applicationId ? previousData : undefined
  });
  const report = query.data;
  const metricOptions = [
    { value: 'task_count', label: t('statistics.tasks') },
    { value: 'total_tokens', label: 'Tokens' },
    { value: 'total_cost', label: t('statistics.cost') }
  ];
  const valueLabel = (metric: Metric, value: number | null) =>
    metric === 'total_cost'
      ? formatCost(value)
      : metric === 'total_tokens'
        ? formatTokenCount(value ?? 0)
        : formatInteger(value ?? 0);
  const logsHref = (filters = {}) =>
    statisticsLogsHref(applicationId, {
      started_from: report?.meta.started_from ?? undefined,
      started_to: report?.meta.started_to ?? undefined,
      ...filters
    });
  const navigate = (href: string) => {
    window.location.assign(href);
  };
  function distribution<T extends Usage>(
    dimension: 'models' | 'users',
    title: string,
    rows: T[],
    identity: (row: T) => string,
    label: (row: T) => string,
    href: (row: T) => string
  ) {
    const distributionMetric = distributionMetrics[dimension];
    return (
      <Card
        className="application-statistics__section"
        aria-label={title}
        title={
          <div className="application-statistics__card-heading">
            <span>{title}</span>
            <Radio.Group
              className="application-statistics__metric-controls"
              aria-label={`${title} ${t('statistics.distribution_metric')}`}
              optionType="button"
              buttonStyle="solid"
              size="small"
              options={metricOptions}
              value={distributionMetric}
              onChange={(event) =>
                setDistributionMetrics((previous) => ({
                  ...previous,
                  [dimension]: event.target.value
                }))
              }
            />
          </div>
        }
        styles={{
          header: { paddingBlock: 16 },
          title: { whiteSpace: 'normal' }
        }}
      >
        {rows.length ? (
          <div className="application-statistics__distribution-body">
            <div className="application-statistics__distribution-chart">
              {rows.some((row) => (row[distributionMetric] ?? 0) > 0) ? (
                <ApplicationMonitoringChart
                  ariaLabel={title}
                  onDataClick={(index) => {
                    if (rows[index]) navigate(href(rows[index]));
                  }}
                  option={{
                    color: [
                      token.colorPrimary,
                      token.colorInfo,
                      token.colorWarning,
                      token.colorSuccess,
                      token.colorTextSecondary
                    ],
                    tooltip: { trigger: 'item' },
                    series: [
                      {
                        type: 'pie',
                        stillShowZeroSum: false,
                        radius: ['48%', '72%'],
                        label: { show: false },
                        data: rows.map((row) => ({
                          name: label(row),
                          value: row[distributionMetric]
                        }))
                      }
                    ]
                  }}
                />
              ) : (
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={t('statistics.no_metric_data')}
                />
              )}
            </div>
            <div className="application-statistics__distribution-table">
              <Table<T>
                scroll={{ x: 420 }}
                size="small"
                rowKey={identity}
                dataSource={rows}
                pagination={{ pageSize: 8, hideOnSinglePage: true }}
                columns={[
                  {
                    title,
                    key: 'dimension',
                    render: (_, row) => <a href={href(row)}>{label(row)}</a>
                  },
                  ...metricOptions.map((metric) => ({
                    title: metric.label,
                    dataIndex: metric.value,
                    align: 'right' as const,
                    render: (value: number | null) =>
                      valueLabel(metric.value as Metric, value)
                  }))
                ]}
              />
            </div>
          </div>
        ) : (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
        )}
      </Card>
    );
  }
  const filters = (
    <Card className="application-statistics__filters">
      <Space wrap className="application-statistics__toolbar">
        <Radio.Group
          className="application-statistics__metric-controls"
          optionType="button"
          options={[
            ...monitoringTimeRangeOptions(),
            { value: 'custom', label: t('statistics.custom') }
          ]}
          value={custom ? 'custom' : timeRangeDays}
          onChange={(event) => {
            const value = event.target.value;
            setCustom(value === 'custom');
            if (value !== 'custom') {
              setTimeRangeDays(value);
            }
          }}
        />
        {custom && (
          <DatePicker.RangePicker
            style={{ maxWidth: '100%' }}
            value={
              range.from && range.to
                ? [dayjs(range.from), dayjs(range.to)]
                : null
            }
            showTime
            aria-label={t('statistics.custom')}
            onChange={(dates) =>
              setRange(
                dates?.[0] && dates[1]
                  ? {
                      from: dates[0].toISOString(),
                      to: dates[1].toISOString()
                    }
                  : {}
              )
            }
          />
        )}
        <Select
          aria-label={t('statistics.bucket')}
          value={bucket}
          onChange={setBucket}
          options={[
            { value: 'auto', label: t('statistics.auto_bucket') },
            { value: 'hour', label: t('statistics.hour') },
            { value: 'day', label: t('statistics.day') },
            { value: 'week', label: t('statistics.week') },
            { value: 'month', label: t('statistics.month') }
          ]}
        />
        <Button
          aria-label={t('auto.refresh_monitoring_report')}
          icon={<ReloadOutlined />}
          loading={query.isFetching}
          onClick={() => void query.refetch()}
        />
        {report && (
          <Button type="link" href={logsHref()} disabled={query.isFetching}>
            {t('statistics.view_logs')}
          </Button>
        )}
      </Space>
    </Card>
  );
  return (
    <div
      className="application-statistics"
      data-testid="application-statistics-page"
    >
      <Typography.Text type="secondary">
        {t('statistics.scope')}
      </Typography.Text>
      {filters}
      {query.isPending ? (
        <LoadingState compact />
      ) : !report ? (
        <Result
          status="error"
          title={t('auto.monitoring_report_load_failed')}
        />
      ) : (
        <Spin
          spinning={query.isFetching}
          description={t('statistics.refreshing')}
        >
          <div
            className="application-statistics__data"
            aria-busy={query.isFetching}
            inert={query.isFetching}
          >
            {query.isError && (
              <Alert
                type="error"
                title={t('auto.monitoring_report_load_failed')}
                showIcon
              />
            )}
            <div className="application-statistics__metrics">
              <Card className="application-statistics__metric application-statistics__metric--tasks">
                <div className="application-statistics__metric-content">
                  <FileTextOutlined className="application-statistics__metric-icon" />
                  <div className="application-statistics__metric-value">
                    <span>{t('statistics.tasks')}</span>
                    <strong>
                      {formatInteger(report.overview.total_count)}
                    </strong>
                    <small>
                      {t('statistics.status_summary', {
                        success: report.overview.success_count,
                        failed: report.overview.failed_count,
                        cancelled: report.overview.cancelled_count,
                        running: report.overview.running_count
                      })}
                    </small>
                  </div>
                </div>
              </Card>
              <Card className="application-statistics__metric application-statistics__metric--tokens">
                <div className="application-statistics__metric-content">
                  <DatabaseOutlined className="application-statistics__metric-icon" />
                  <div className="application-statistics__metric-value">
                    <span>{t('auto.total_tokens_amount')}</span>
                    <strong>
                      {formatTokenCount(report.tokens.total_tokens_sum)}
                    </strong>
                    <small>
                      {t('auto.input_tokens')}:{' '}
                      {formatTokenCount(report.tokens.input_tokens_sum)} ·{' '}
                      {t('auto.output_tokens')}:{' '}
                      {formatTokenCount(report.tokens.output_tokens_sum)} ·{' '}
                      {t('auto.input_cache_hit_tokens')}:{' '}
                      {formatTokenCount(
                        report.tokens.input_cache_hit_tokens_sum
                      )}
                    </small>
                  </div>
                </div>
              </Card>
              <Card className="application-statistics__metric application-statistics__metric--cost">
                <div className="application-statistics__metric-content">
                  <DollarOutlined className="application-statistics__metric-icon" />
                  <div className="application-statistics__metric-value">
                    <span>{t('statistics.cost')}</span>
                    <strong>{formatCost(report.costs.total_cost)}</strong>
                    <small>
                      {t('statistics.cost_coverage', {
                        recorded: report.costs.cost_recorded_count,
                        missing: report.costs.cost_missing_count
                      })}
                    </small>
                  </div>
                </div>
              </Card>
              <Card className="application-statistics__metric application-statistics__metric--duration">
                <div className="application-statistics__metric-content">
                  <ClockCircleOutlined className="application-statistics__metric-icon" />
                  <div className="application-statistics__metric-value">
                    <span>{t('auto.average_duration')}</span>
                    <strong>
                      {report.duration.duration_recorded_count
                        ? formatDuration(report.duration.avg_duration_ms)
                        : '—'}
                    </strong>
                    <small>
                      P95:{' '}
                      {report.duration.duration_recorded_count
                        ? formatDuration(report.duration.p95_duration_ms)
                        : '—'}
                    </small>
                  </div>
                </div>
              </Card>
            </div>
            <div className="application-statistics__distributions">
              {distribution(
                'models',
                t('statistics.models'),
                report.models,
                (row) => row.requested_model_id ?? '__missing_model',
                (row) =>
                  row.requested_model_id ?? t('statistics.unknown_model'),
                (row) =>
                  logsHref(
                    row.requested_model_id === null
                      ? { missing_model: true }
                      : { requested_model_id: row.requested_model_id }
                  )
              )}
              {distribution(
                'users',
                t('statistics.users'),
                report.users,
                (row) => row.user_id ?? '__missing_user',
                (row) => row.name ?? t('statistics.unknown_user'),
                (row) =>
                  logsHref(
                    row.user_id === null
                      ? { missing_user: true }
                      : { user_id: row.user_id }
                  )
              )}
            </div>
            <Card
              className="application-statistics__section application-statistics__trend"
              title={
                <div className="application-statistics__card-heading">
                  <span>
                    {trendMetric === 'total_tokens'
                      ? t('statistics.token_trend')
                      : t('statistics.trend')}
                  </span>
                  <Radio.Group
                    className="application-statistics__metric-controls"
                    aria-label={t('statistics.trend_metric')}
                    optionType="button"
                    buttonStyle="solid"
                    size="small"
                    options={[
                      ...metricOptions,
                      {
                        value: 'avg_duration_ms',
                        label: t('auto.average_duration')
                      }
                    ]}
                    value={trendMetric}
                    onChange={(event) => setTrendMetric(event.target.value)}
                  />
                </div>
              }
              styles={{
                header: { paddingBlock: 16 },
                title: { whiteSpace: 'normal' }
              }}
            >
              {report.tokens_trend.length ? (
                <ApplicationMonitoringChart
                  ariaLabel={t('statistics.token_trend')}
                  onDataClick={(index) => {
                    const point = report.tokens_trend[index];
                    if (point)
                      navigate(
                        logsHref(statisticsBucketFilters(report.meta, point))
                      );
                  }}
                  option={{
                    color: [
                      token.blue,
                      token.colorSuccess,
                      token.cyan,
                      token.purple
                    ],
                    tooltip: { trigger: 'axis' },
                    legend: { type: 'scroll', top: 0 },
                    grid: {
                      left: 64,
                      right: trendMetric === 'total_tokens' ? 64 : 24,
                      top: 56,
                      bottom: 48
                    },
                    xAxis: {
                      type: 'category',
                      data: report.tokens_trend.map((point) =>
                        formatTrendBucket(
                          point.bucket_start,
                          report.meta.bucket
                        )
                      )
                    },
                    yAxis:
                      trendMetric === 'total_tokens'
                        ? [
                            {
                              type: 'value',
                              name: 'Token'
                            },
                            {
                              type: 'value',
                              name: '%',
                              min: 0,
                              max: 100,
                              splitLine: { show: false }
                            }
                          ]
                        : {
                            type: 'value',
                            name:
                              trendMetric === 'avg_duration_ms'
                                ? 'ms'
                                : trendMetric === 'total_cost'
                                  ? '$'
                                  : ''
                          },
                    series:
                      trendMetric === 'total_tokens'
                        ? [
                            ...[
                              {
                                name: t('auto.input_tokens'),
                                field: 'input_tokens',
                                color: token.blue
                              },
                              {
                                name: t('auto.output_tokens'),
                                field: 'output_tokens',
                                color: token.colorSuccess
                              },
                              {
                                name: t('auto.input_cache_hit_tokens'),
                                field: 'input_cache_hit_tokens',
                                color: token.cyan
                              }
                            ].map(({ name, field, color }) => ({
                              name,
                              type: 'line',
                              showSymbol: report.tokens_trend.length < 32,
                              symbolSize: 6,
                              connectNulls: false,
                              lineStyle: { width: 2, color },
                              itemStyle: { color },
                              areaStyle: { opacity: 0.08, color },
                              data: report.tokens_trend.map(
                                (point) =>
                                  point[
                                    field as
                                      | 'input_tokens'
                                      | 'output_tokens'
                                      | 'input_cache_hit_tokens'
                                  ]
                              )
                            })),
                            {
                              name: t('auto.input_cache_hit_rate'),
                              type: 'line',
                              yAxisIndex: 1,
                              showSymbol: report.tokens_trend.length < 32,
                              symbolSize: 6,
                              connectNulls: false,
                              lineStyle: {
                                width: 2,
                                color: token.purple,
                                type: 'dashed'
                              },
                              itemStyle: { color: token.purple },
                              data: report.tokens_trend.map((point) =>
                                point.input_cache_hit_rate === null
                                  ? null
                                  : Number(
                                      (
                                        point.input_cache_hit_rate * 100
                                      ).toFixed(2)
                                    )
                              )
                            }
                          ]
                        : [
                            {
                              name:
                                trendMetric === 'avg_duration_ms'
                                  ? t('auto.average_duration')
                                  : metricOptions.find(
                                      (metric) => metric.value === trendMetric
                                    )?.label,
                              type: 'line',
                              showSymbol: true,
                              connectNulls: false,
                              data: report.tokens_trend.map((point) =>
                                trendMetric === 'task_count'
                                  ? point.run_count
                                  : point[trendMetric]
                              )
                            }
                          ]
                  }}
                />
              ) : (
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
              )}
            </Card>
          </div>
        </Spin>
      )}
    </div>
  );
}
