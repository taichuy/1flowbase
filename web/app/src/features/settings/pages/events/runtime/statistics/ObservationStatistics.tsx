import type { EventSourceRuntimeSnapshot } from '@1flowbase/api-client';
import { EChart } from '@1flowbase/charts';
import { theme, Tooltip } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useRuntimeLabels } from '../runtime-labels';
import {
  appendObservationSample,
  observationStatistics,
  type ObservationSample
} from './observation-statistics';
import { ObservationMetrics } from './ObservationMetrics';
import {
  observationTrendOption,
  observationCleanupOption
} from './observation-chart-options';
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
  const trend = useMemo(
    () => observationTrendOption(samples, categories, token),
    [samples, categories, token]
  );
  const cleanup = observationCleanupOption(
    views,
    views.map((view) => {
      const loss = view.observation_loss;
      return `${scopeLabel(view.scope)} · ${t('observation_stats_record_count', { count: view.activity.length })}\n${loss.accounting_available && loss.window_started_at_unix_ms !== null ? t('observation_stats_count_since', { time: new Date(loss.window_started_at_unix_ms).toLocaleTimeString([], { hour12: false }) }) : t('observation_stats_no_accounting')}`;
    }),
    [
      t('observation_stats_expired_records'),
      t('observation_stats_evicted_records'),
      t('observation_stats_oversize_records')
    ],
    token
  );
  const duration = (ms: number) =>
    ms >= 60000
      ? t('observation_stats_minutes', {
          count: Number((ms / 60000).toFixed(1))
        })
      : t('observation_stats_seconds', {
          count: Number((ms / 1000).toFixed(1))
        });
  return (
    <section
      className="event-observation-statistics"
      aria-label={t('observation_stats_overview')}
    >
      <ObservationMetrics stats={stats} admissionRejected={admissionRejected} />
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
