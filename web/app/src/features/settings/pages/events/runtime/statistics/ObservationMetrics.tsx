import InfoCircleOutlined from '@ant-design/icons/es/icons/InfoCircleOutlined';
import { Progress, theme, Tooltip } from 'antd';
import { useRuntimeLabels } from '../runtime-labels';
import {
  formatObservationBytes,
  observationStatistics
} from './observation-statistics';

export function ObservationMetrics({
  stats,
  admissionRejected
}: {
  stats: ReturnType<typeof observationStatistics>;
  admissionRejected: number;
}) {
  const { t } = useRuntimeLabels();
  const { token } = theme.useToken();
  const duration = (ms: number) =>
    ms >= 60000
      ? t('observation_stats_minutes', {
          count: Number((ms / 60000).toFixed(1))
        })
      : t('observation_stats_seconds', {
          count: Number((ms / 1000).toFixed(1))
        });
  const hasAccounting = stats.accounting_scopes > 0;
  const partial =
    hasAccounting && stats.accounting_scopes < stats.visible_scopes;
  const coverage = partial
    ? t('observation_stats_partial')
    : t('observation_stats_window', {
        duration: duration(stats.window_duration_ms)
      });
  const metrics = [
    {
      label: t('observation_stats_retained_records'),
      value: stats.retained_records.toLocaleString(),
      note: t('observation_stats_retention', {
        duration: duration(stats.observation_ttl_ms)
      }),
      hint: t('observation_stats_retention_hint', {
        minutes: stats.observation_ttl_ms / 60000
      })
    },
    {
      label: t('observation_stats_space_used'),
      value: formatObservationBytes(stats.observation_bytes),
      note: t('observation_stats_limit', {
        max: formatObservationBytes(stats.observation_max_bytes)
      }),
      hint: t('observation_stats_budget_hint', {
        max: formatObservationBytes(stats.observation_max_bytes)
      }),
      budget: true
    },
    {
      label: t('observation_stats_expired_records'),
      value: hasAccounting ? stats.known_expired.toLocaleString() : '—',
      note: hasAccounting ? coverage : t('observation_stats_no_accounting'),
      hint: t('observation_stats_known_counts')
    },
    {
      label: t('observation_stats_evicted_records'),
      value: hasAccounting ? stats.known_evicted.toLocaleString() : '—',
      note: hasAccounting ? coverage : t('observation_stats_no_accounting'),
      hint: t('observation_stats_evicted_hint')
    },
    {
      label: t('observation_stats_oversize_records'),
      value: hasAccounting
        ? stats.known_oversize_dropped.toLocaleString()
        : '—',
      note: hasAccounting ? coverage : t('observation_stats_no_accounting'),
      hint: t('observation_stats_oversize_hint')
    },
    {
      label: t('observation_stats_rejected'),
      value: admissionRejected.toLocaleString(),
      note: t('observation_stats_workspace_window'),
      hint: t('observation_stats_rejected_hint')
    }
  ];
  const budgetPercent = stats.observation_max_bytes
    ? (stats.observation_bytes / stats.observation_max_bytes) * 100
    : null;

  return (
    <div className="event-observation-metrics">
      {metrics.map((metric) => (
        <div className="event-observation-metric" key={metric.label}>
          <div className="event-observation-metric-label">
            {metric.label}
            <Tooltip title={metric.hint}>
              <button
                type="button"
                className="event-observation-info"
                aria-label={metric.label}
              >
                <InfoCircleOutlined />
              </button>
            </Tooltip>
          </div>
          <div className="event-observation-metric-value">{metric.value}</div>
          <div className="event-observation-metric-note">
            {metric.note}
            {metric.budget && budgetPercent !== null
              ? ` · ${budgetPercent.toFixed(1)}%`
              : ''}
          </div>
          {metric.budget && budgetPercent !== null && (
            <Progress
              percent={Math.min(100, budgetPercent)}
              showInfo={false}
              size="small"
              strokeColor={token.colorPrimary}
            />
          )}
        </div>
      ))}
    </div>
  );
}
