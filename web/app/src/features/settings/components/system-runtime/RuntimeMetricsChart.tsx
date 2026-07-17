import { useEffect, useMemo, useRef } from 'react';

import { LineChart } from 'echarts/charts';
import {
  GridComponent,
  LegendComponent,
  TooltipComponent
} from 'echarts/components';
import * as echarts from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';

import { i18nText } from '../../../../shared/i18n/text';

echarts.use([
  LineChart,
  GridComponent,
  LegendComponent,
  TooltipComponent,
  CanvasRenderer
]);

export type RuntimeMetricKind = 'network' | 'disk_io' | 'cpu' | 'memory';

export interface RuntimeMetricPoint {
  capturedAt: number;
  cpuUsagePercent: number | null;
  memoryUsagePercent: number | null;
  networkReceivedBytesPerSecond: number | null;
  networkTransmittedBytesPerSecond: number | null;
  diskReadBytesPerSecond: number | null;
  diskWrittenBytesPerSecond: number | null;
}

function timeLabel(timestamp: number) {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false
  });
}

function seriesFor(kind: RuntimeMetricKind, points: RuntimeMetricPoint[]) {
  if (kind === 'cpu') {
    return [
      {
        name: i18nText('settings', 'auto.cpu_usage'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        data: points.map((point) => point.cpuUsagePercent)
      }
    ];
  }
  if (kind === 'memory') {
    return [
      {
        name: i18nText('settings', 'auto.memory_usage'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        data: points.map((point) => point.memoryUsagePercent)
      }
    ];
  }
  if (kind === 'disk_io') {
    return [
      {
        name: i18nText('settings', 'auto.read_rate'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        data: points.map((point) => point.diskReadBytesPerSecond)
      },
      {
        name: i18nText('settings', 'auto.written_rate'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        lineStyle: { type: 'dashed' as const },
        data: points.map((point) => point.diskWrittenBytesPerSecond)
      }
    ];
  }
  return [
    {
      name: i18nText('settings', 'auto.received_rate'),
      type: 'line' as const,
      smooth: true,
      showSymbol: false,
      connectNulls: false,
      data: points.map((point) => point.networkReceivedBytesPerSecond)
    },
    {
      name: i18nText('settings', 'auto.transmitted_rate'),
      type: 'line' as const,
      smooth: true,
      showSymbol: false,
      connectNulls: false,
      lineStyle: { type: 'dashed' as const },
      data: points.map((point) => point.networkTransmittedBytesPerSecond)
    }
  ];
}

export function RuntimeMetricsChart({
  kind,
  points
}: {
  kind: RuntimeMetricKind;
  points: RuntimeMetricPoint[];
}) {
  const chartRef = useRef<HTMLDivElement>(null);
  const chartInstanceRef = useRef<ReturnType<typeof echarts.init> | null>(null);
  const option = useMemo<echarts.EChartsCoreOption>(() => {
    const percentage = kind === 'cpu' || kind === 'memory';
    return {
      animation: false,
      color: ['#00ab73', '#7b8982'],
      tooltip: { trigger: 'axis' },
      legend: {
        top: 0,
        left: 0,
        itemWidth: 18,
        itemHeight: 3,
        textStyle: { color: '#55645d', fontSize: 12 }
      },
      grid: { top: 44, right: 20, bottom: 28, left: 56 },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        data: points.map((point) => timeLabel(point.capturedAt)),
        axisLine: { lineStyle: { color: '#d5ddd8' } },
        axisTick: { show: false },
        axisLabel: { color: '#7b8982', fontSize: 11 }
      },
      yAxis: {
        type: 'value',
        name: percentage ? '%' : 'B/s',
        min: 0,
        max: percentage ? 100 : undefined,
        nameTextStyle: { color: '#7b8982', fontSize: 11 },
        axisLabel: { color: '#7b8982', fontSize: 11 },
        splitLine: { lineStyle: { color: '#e8edea' } }
      },
      series: seriesFor(kind, points)
    };
  }, [kind, points]);

  useEffect(() => {
    if (!chartRef.current) {
      return;
    }
    const chart = echarts.init(chartRef.current);
    chartInstanceRef.current = chart;
    const resizeObserver =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => chart.resize());
    resizeObserver?.observe(chartRef.current);
    return () => {
      resizeObserver?.disconnect();
      chartInstanceRef.current = null;
      chart.dispose();
    };
  }, []);

  useEffect(() => {
    chartInstanceRef.current?.setOption(option, {
      notMerge: true,
      lazyUpdate: true
    });
  }, [option]);

  return (
    <div
      ref={chartRef}
      aria-label={i18nText('settings', 'auto.runtime_metrics_chart')}
      className="system-runtime-panel__chart"
      role="img"
    />
  );
}
