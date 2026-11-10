import type { EventSourceRuntimeSnapshot } from '@1flowbase/api-client';
import { EChart, type EChartOption } from '@1flowbase/charts';
import InfoCircleOutlined from '@ant-design/icons/es/icons/InfoCircleOutlined';
import { Progress, theme, Tooltip } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useRuntimeLabels } from '../runtime-labels';
import {
  appendObservationSample,
  formatObservationBytes,
  observationStatistics,
  type ObservationSample
} from './observation-statistics';
import './observation-statistics.css';

export function ObservationStatistics({
  views,
  capturedAt,
  admissionRejected
}: {
  views: EventSourceRuntimeSnapshot[];
  capturedAt: number;
  admissionRejected: number;
}) {
  const { t, scopeLabel } = useRuntimeLabels();
  const { token } = theme.useToken();
  const stats = useMemo(() => observationStatistics(views), [views]);
  const [samples, setSamples] = useState<ObservationSample[]>(() => [
    {
      captured_at: capturedAt,
      observation_bytes: stats.observation_bytes,
      source_metadata_bytes: stats.source_metadata_bytes
    }
  ]);
  useEffect(() => {
    setSamples((previous) =>
      appendObservationSample(previous, {
        captured_at: capturedAt,
        observation_bytes: stats.observation_bytes,
        source_metadata_bytes: stats.source_metadata_bytes
      })
    );
  }, [capturedAt, stats.observation_bytes, stats.source_metadata_bytes]);

  const categories = useMemo(
    () => [
      t('observation_stats_records_and_accounting'),
      t('observation_stats_source_metadata')
    ],
    [t]
  );
  const trend = useMemo<EChartOption>(
    () => ({
      animation: false,
      color: [token.colorPrimary, token.colorInfo],
      textStyle: { fontFamily: token.fontFamily },
      tooltip: {
        trigger: 'axis',
        confine: true,
        backgroundColor: token.colorBgElevated,
        borderColor: token.colorBorderSecondary,
        textStyle: { color: token.colorText }
      },
      legend: {
        top: 0,
        right: 0,
        icon: 'roundRect',
        itemWidth: 10,
        itemHeight: 7,
        textStyle: { color: token.colorTextSecondary, fontSize: 11 }
      },
      grid: { top: 36, right: 12, bottom: 24, left: 48 },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: samples.map((point) =>
          new Date(point.captured_at).toLocaleTimeString([], { hour12: false })
        ),
        axisTick: { show: false },
        axisLine: { lineStyle: { color: token.colorBorderSecondary } },
        axisLabel: {
          color: token.colorTextSecondary,
          fontSize: 11,
          hideOverlap: true
        }
      },
      yAxis: {
        type: 'value',
        min: 0,
        name: 'KiB',
        splitNumber: 3,
        nameTextStyle: { color: token.colorTextSecondary, fontSize: 11 },
        axisLabel: { color: token.colorTextSecondary, fontSize: 11 },
        splitLine: {
          lineStyle: { color: token.colorBorderSecondary, type: 'dashed' }
        }
      },
      series: categories.map((name, index) => ({
        name,
        type: 'line',
        stack: 'observation',
        symbol: 'circle',
        symbolSize: samples.length === 1 ? 6 : 3,
        showSymbol: samples.length === 1,
        lineStyle: { width: 2 },
        areaStyle: { opacity: index ? 0.12 : 0.16 },
        data: samples.map((point) =>
          Number(
            (
              (index
                ? point.source_metadata_bytes
                : point.observation_bytes - point.source_metadata_bytes) / 1024
            ).toFixed(2)
          )
        )
      }))
    }),
    [samples, categories, token]
  );
  const cleanupNames = [
    t('observation_stats_expired_records'),
    t('observation_stats_evicted_records'),
    t('observation_stats_oversize_records')
  ];
  const cleanupColors = [
    token.colorPrimary,
    token.colorWarning,
    token.colorError
  ];
  const cleanup: EChartOption = {
    animation: false,
    color: cleanupColors,
    tooltip: {
      trigger: 'axis',
      confine: true,
      axisPointer: { type: 'shadow' },
      backgroundColor: token.colorBgElevated,
      borderColor: token.colorBorderSecondary,
      textStyle: { color: token.colorText }
    },
    legend: {
      top: 0,
      right: 0,
      icon: 'roundRect',
      itemWidth: 8,
      itemHeight: 7,
      textStyle: { color: token.colorTextSecondary, fontSize: 11 }
    },
    grid: { top: 34, right: 28, bottom: 24, left: 142 },
    xAxis: {
      type: 'value',
      minInterval: 1,
      min: 0,
      axisLabel: { color: token.colorTextSecondary, fontSize: 11 },
      splitLine: {
        lineStyle: { color: token.colorBorderSecondary, type: 'dashed' }
      }
    },
    yAxis: {
      type: 'category',
      inverse: true,
      data: views.map((view) => {
        const loss = view.observation_loss;
        return `${scopeLabel(view.scope)} · ${t('observation_stats_record_count', { count: view.activity.length })}\n${loss.accounting_available && loss.window_started_at_unix_ms !== null ? t('observation_stats_count_since', { time: new Date(loss.window_started_at_unix_ms).toLocaleTimeString([], { hour12: false }) }) : t('observation_stats_no_accounting')}`;
      }),
      axisTick: { show: false },
      axisLine: { show: false },
      axisLabel: {
        color: token.colorTextSecondary,
        fontSize: 11,
        lineHeight: 14,
        interval: 0
      }
    },
    series: (['expired', 'evicted', 'oversize_dropped'] as const).map(
      (key, index) => ({
        name: cleanupNames[index],
        type: 'bar',
        stack: 'cleanup',
        barMaxWidth: 16,
        data: views.map((view) =>
          view.observation_loss.accounting_available
            ? {
                value: view.observation_loss[key],
                label: {
                  show: index === 0 || view.observation_loss[key] > 0,
                  position: 'right',
                  color: token.colorTextSecondary,
                  fontSize: 11
                }
              }
            : null
        )
      })
    )
  };
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
    <section
      className="event-observation-statistics"
      aria-label={t('observation_stats_overview')}
    >
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
      <div className="event-observation-charts">
        <section className="event-observation-trend">
          <header>
            <h3>{t('observation_stats_space_trend')}</h3>
            <Tooltip title={t('observation_stats_trend_hint')}>
              <span className="event-observation-period">
                {t('stats_current_visit')}
              </span>
            </Tooltip>
          </header>
          <EChart
            className="event-observation-trend-chart"
            ariaLabel={t('observation_stats_space_trend')}
            option={trend}
            tooltipValueUnit="KiB"
          />
        </section>
        <section className="event-observation-composition">
          <header>
            <h3>{t('observation_stats_cleanup_title')}</h3>
            <span className="event-observation-period">
              {t('observation_stats_coverage', {
                available: stats.accounting_scopes,
                total: stats.visible_scopes,
                duration: duration(stats.window_duration_ms)
              })}
            </span>
          </header>
          <EChart
            className="event-observation-cleanup-chart"
            ariaLabel={t('observation_stats_cleanup_title')}
            option={cleanup}
            tooltipValueUnit={t('observation_stats_records_unit')}
          />
        </section>
      </div>
    </section>
  );
}
