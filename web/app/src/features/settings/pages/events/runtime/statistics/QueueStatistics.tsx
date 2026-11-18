import type { EventRuntimeQueueSnapshot } from '@1flowbase/api-client';
import { EChart, type EChartOption } from '@1flowbase/charts';
import InfoCircleOutlined from '@ant-design/icons/es/icons/InfoCircleOutlined';
import { theme, Tooltip } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useRuntimeLabels } from '../runtime-labels';
import {
  appendQueueSample,
  queueStatistics,
  type QueueSample
} from './queue-statistics';
import './queue-statistics.css';

export function QueueStatistics({
  queues,
  capturedAt
}: {
  queues: EventRuntimeQueueSnapshot[];
  capturedAt: number;
}) {
  const { t } = useRuntimeLabels();
  const { token } = theme.useToken();
  const stats = useMemo(() => queueStatistics(queues), [queues]);
  const [samples, setSamples] = useState<QueueSample[]>(() => [
    {
      captured_at: capturedAt,
      buffered: stats.buffered
    }
  ]);
  useEffect(() => {
    setSamples((previous) =>
      appendQueueSample(previous, {
        captured_at: capturedAt,
        buffered: stats.buffered
      })
    );
  }, [capturedAt, stats.buffered]);

  const categories = useMemo(
    () => [t('stats_open_queues'), t('stats_closed_queues')],
    [t]
  );
  const colors = [token.colorPrimary, token.colorWarning];
  const trend = useMemo<EChartOption>(
    () => ({
      animation: false,
      color: [token.colorPrimary, token.colorWarning],
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
        name: t('stats_delivery_unit'),
        minInterval: 1,
        splitNumber: 3,
        nameTextStyle: { color: token.colorTextSecondary, fontSize: 11 },
        axisLabel: { color: token.colorTextSecondary, fontSize: 11 },
        splitLine: {
          lineStyle: { color: token.colorBorderSecondary, type: 'dashed' }
        }
      },
      series: [
        {
          name: t('stats_buffered'),
          type: 'line',
          symbol: 'circle',
          symbolSize: samples.length === 1 ? 6 : 3,
          showSymbol: samples.length === 1,
          lineStyle: { width: 2 },
          areaStyle: { opacity: 0.14 },
          data: samples.map((point) => point.buffered)
        }
      ]
    }),
    [samples, t, token]
  );
  const composition = useMemo<EChartOption>(
    () => ({
      animation: false,
      color: [token.colorPrimary, token.colorWarning],
      tooltip: {
        trigger: 'item',
        confine: true,
        backgroundColor: token.colorBgElevated,
        borderColor: token.colorBorderSecondary,
        textStyle: { color: token.colorText }
      },
      series: [
        {
          name: t('stats_queue_state'),
          type: 'pie',
          radius: ['67%', '88%'],
          center: ['50%', '50%'],
          label: { show: false },
          labelLine: { show: false },
          avoidLabelOverlap: true,
          stillShowZeroSum: false,
          showEmptyCircle: true,
          emptyCircleStyle: { color: token.colorFillSecondary },
          data: stats.queue_count
            ? [
                {
                  name: categories[0],
                  value: stats.open
                },
                { name: categories[1], value: stats.closed }
              ]
            : []
        }
      ]
    }),
    [categories, stats.open, stats.closed, stats.queue_count, t, token]
  );

  const metrics = [
    {
      label: t('stats_queue_count'),
      value: stats.queue_count.toLocaleString(),
      hint: t('stats_queue_count_hint')
    },
    {
      label: t('stats_buffered'),
      value: stats.buffered.toLocaleString(),
      hint: t('stats_buffered_hint', {
        in_flight: stats.in_flight,
        accepted: stats.accepted,
        rejected: stats.admission_rejected
      })
    },
    {
      label: t('stats_consumer_count'),
      value: stats.consumer_count.toLocaleString(),
      hint: t('stats_consumer_count_hint')
    },
    {
      label: t('stats_consumed'),
      value: stats.consumed.toLocaleString(),
      hint: t('stats_consumed_hint')
    }
  ];
  const parts = [stats.open, stats.closed];

  return (
    <section
      className="event-queue-statistics"
      aria-label={t('stats_overview')}
    >
      <div className="event-queue-metrics">
        {metrics.map((metric) => (
          <div className="event-queue-metric" key={metric.label}>
            <div className="event-queue-metric-label">
              {metric.label}
              <Tooltip title={metric.hint}>
                <button
                  type="button"
                  className="event-queue-info"
                  aria-label={metric.label}
                >
                  <InfoCircleOutlined />
                </button>
              </Tooltip>
            </div>
            <div className="event-queue-metric-value">{metric.value}</div>
          </div>
        ))}
      </div>
      <div className="event-queue-charts">
        <section className="event-queue-trend">
          <header>
            <h3>{t('stats_queue_trend')}</h3>
            <Tooltip title={t('stats_trend_hint')}>
              <span className="event-queue-period">
                {t('stats_current_visit')}
              </span>
            </Tooltip>
          </header>
          <EChart
            className="event-queue-trend-chart"
            ariaLabel={t('stats_queue_trend')}
            option={trend}
            tooltipValueUnit={t('stats_delivery_unit')}
          />
        </section>
        <section className="event-queue-composition">
          <header>
            <h3>{t('stats_queue_state')}</h3>
          </header>
          <div className="event-queue-composition-content">
            <div className="event-queue-donut">
              <EChart
                className="event-queue-donut-chart"
                ariaLabel={t('stats_queue_state')}
                option={composition}
              />
              <div className="event-queue-donut-label">
                <span>{t('stats_queue_count')}</span>
                <strong>{stats.queue_count.toLocaleString()}</strong>
              </div>
            </div>
            <dl className="event-queue-legend">
              {categories.map((name, index) => (
                <div key={name}>
                  <dt>
                    <i style={{ backgroundColor: colors[index] }} />
                    {name}
                  </dt>
                  <dd>
                    <strong>{parts[index].toLocaleString()}</strong>
                    <span>
                      {stats.queue_count
                        ? `${((parts[index] / stats.queue_count) * 100).toFixed(1)}%`
                        : '—'}
                    </span>
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        </section>
      </div>
    </section>
  );
}
