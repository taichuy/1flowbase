import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { useQuery } from '@tanstack/react-query';
import {
  Button,
  DatePicker,
  Empty,
  Radio,
  Result,
  Select,
  Space,
  Table,
  Typography,
  theme
} from 'antd';
import { useState } from 'react';
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
  const [distributionMetric, setDistributionMetric] =
    useState<Metric>('task_count');
  const [trendMetric, setTrendMetric] = useState<Metric | 'avg_duration_ms'>(
    'task_count'
  );
  const input = {
    timeRangeDays,
    bucket: resolvedBucket,
    ...(custom ? range : {})
  };
  const query = useQuery({
    queryKey: applicationRunMonitoringReportQueryKey(applicationId, input),
    queryFn: () => fetchApplicationRunMonitoringReport(applicationId, input)
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
    title: string,
    rows: T[],
    identity: (row: T) => string,
    label: (row: T) => string,
    href: (row: T) => string
  ) {
    return (
      <section className="application-statistics__section" aria-label={title}>
        <Typography.Title level={5}>{title}</Typography.Title>
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
      </section>
    );
  }
  return (
    <div
      className="application-statistics"
      data-testid="application-statistics-page"
    >
      <Space wrap className="application-statistics__toolbar">
        <Radio.Group
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
            showTime
            aria-label={t('statistics.custom')}
            onChange={(dates) =>
              setRange(
                dates?.[0] && dates[1]
                  ? { from: dates[0].toISOString(), to: dates[1].toISOString() }
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
        {report && <a href={logsHref()}>{t('statistics.view_logs')}</a>}
      </Space>
      {query.isPending ? (
        <LoadingState compact />
      ) : query.isError || !report ? (
        <Result
          status="error"
          title={t('auto.monitoring_report_load_failed')}
        />
      ) : (
        <>
          <Typography.Text type="secondary">
            {t('statistics.scope')}
          </Typography.Text>
          <div className="application-statistics__metrics">
            <section>
              <span>{t('statistics.tasks')}</span>
              <strong>{formatInteger(report.overview.total_count)}</strong>
              <small>
                {t('statistics.status_summary', {
                  success: report.overview.success_count,
                  failed: report.overview.failed_count,
                  cancelled: report.overview.cancelled_count,
                  running: report.overview.running_count
                })}
              </small>
            </section>
            <section>
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
                {formatTokenCount(report.tokens.input_cache_hit_tokens_sum)}
              </small>
            </section>
            <section>
              <span>{t('statistics.cost')}</span>
              <strong>{formatCost(report.costs.total_cost)}</strong>
              <small>
                {t('statistics.cost_coverage', {
                  recorded: report.costs.cost_recorded_count,
                  missing: report.costs.cost_missing_count
                })}
              </small>
            </section>
            <section>
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
            </section>
          </div>
          <Radio.Group
            aria-label={t('statistics.distribution_metric')}
            optionType="button"
            options={metricOptions}
            value={distributionMetric}
            onChange={(event) => setDistributionMetric(event.target.value)}
          />
          <div className="application-statistics__distributions">
            {distribution(
              t('statistics.models'),
              report.models,
              (row) => row.requested_model_id ?? '__missing_model',
              (row) => row.requested_model_id ?? t('statistics.unknown_model'),
              (row) =>
                logsHref(
                  row.requested_model_id === null
                    ? { missing_model: true }
                    : { requested_model_id: row.requested_model_id }
                )
            )}
            {distribution(
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
          <section className="application-statistics__section">
            <Space wrap>
              <Typography.Title level={5}>
                {t('statistics.trend')}
              </Typography.Title>
              <Radio.Group
                aria-label={t('statistics.trend_metric')}
                optionType="button"
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
            </Space>
            {report.tokens_trend.length ? (
              <ApplicationMonitoringChart
                ariaLabel={t('statistics.trend')}
                onDataClick={(index) => {
                  const point = report.tokens_trend[index];
                  if (point)
                    navigate(
                      logsHref(statisticsBucketFilters(report.meta, point))
                    );
                }}
                option={{
                  color: [token.colorPrimary],
                  tooltip: { trigger: 'axis' },
                  grid: { left: 64, right: 24, top: 32, bottom: 40 },
                  xAxis: {
                    type: 'category',
                    data: report.tokens_trend.map((point) =>
                      formatTrendBucket(point.bucket_start, report.meta.bucket)
                    )
                  },
                  yAxis: {
                    type: 'value',
                    name:
                      trendMetric === 'avg_duration_ms'
                        ? 'ms'
                        : trendMetric === 'total_cost'
                          ? '$'
                          : ''
                  },
                  series: [
                    {
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
          </section>
        </>
      )}
    </div>
  );
}
