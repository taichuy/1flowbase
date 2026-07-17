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

export type RuntimeMetricKind =
  | 'network'
  | 'disk_io'
  | 'cpu'
  | 'environment_memory'
  | 'process_memory';

export interface RuntimeMetricPoint {
  capturedAt: number;
  cpuUsagePercent: number | null;
  environmentMemoryUsagePercent: number | null;
  hostRelatedProcessBytes: number | null;
  hostRelatedProcessCount: number | null;
  targetRelatedProcessBytes: number;
  targetRelatedProcessCount: number;
  rootProcessBytes: number;
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

function kilobytesPerSecond(value: number | null) {
  return value === null ? null : Number((value / 1024).toFixed(2));
}

function megabytes(value: number | null) {
  return value === null ? null : Number((value / 1024 / 1024).toFixed(2));
}

function processMemoryTooltip(value: unknown, processCount: number | null) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return '—';
  }
  const memory = `${value.toLocaleString(undefined, {
    maximumFractionDigits: 2
  })} MB`;
  return processCount === null
    ? memory
    : `${memory} · ${i18nText('settings', 'auto.process_count', {
        value1: processCount
      })}`;
}

function seriesFor(
  kind: RuntimeMetricKind,
  points: RuntimeMetricPoint[],
  targetLabel: string
) {
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
  if (kind === 'environment_memory') {
    return [
      {
        name: i18nText('settings', 'auto.memory_usage'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        data: points.map((point) => point.environmentMemoryUsagePercent)
      }
    ];
  }
  if (kind === 'process_memory') {
    return [
      {
        name: i18nText('settings', 'auto.host_related_process_total'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        tooltip: {
          valueFormatter: (value: unknown, dataIndex: number) =>
            processMemoryTooltip(
              value,
              points[dataIndex]?.hostRelatedProcessCount ?? null
            )
        },
        data: points.map((point) => megabytes(point.hostRelatedProcessBytes))
      },
      {
        name: i18nText('settings', 'auto.runtime_target_process_tree', {
          value1: targetLabel
        }),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        tooltip: {
          valueFormatter: (value: unknown, dataIndex: number) =>
            processMemoryTooltip(
              value,
              points[dataIndex]?.targetRelatedProcessCount ?? null
            )
        },
        data: points.map((point) => megabytes(point.targetRelatedProcessBytes))
      },
      {
        name: i18nText('settings', 'auto.runtime_target_root_process_rss', {
          value1: targetLabel
        }),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        lineStyle: { type: 'dashed' as const },
        tooltip: {
          valueFormatter: (value: unknown, dataIndex: number) =>
            processMemoryTooltip(value, points[dataIndex] ? 1 : null)
        },
        data: points.map((point) => megabytes(point.rootProcessBytes))
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
        data: points.map((point) =>
          kilobytesPerSecond(point.diskReadBytesPerSecond)
        )
      },
      {
        name: i18nText('settings', 'auto.written_rate'),
        type: 'line' as const,
        smooth: true,
        showSymbol: false,
        connectNulls: false,
        lineStyle: { type: 'dashed' as const },
        data: points.map((point) =>
          kilobytesPerSecond(point.diskWrittenBytesPerSecond)
        )
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
      data: points.map((point) =>
        kilobytesPerSecond(point.networkReceivedBytesPerSecond)
      )
    },
    {
      name: i18nText('settings', 'auto.transmitted_rate'),
      type: 'line' as const,
      smooth: true,
      showSymbol: false,
      connectNulls: false,
      lineStyle: { type: 'dashed' as const },
      data: points.map((point) =>
        kilobytesPerSecond(point.networkTransmittedBytesPerSecond)
      )
    }
  ];
}

export function RuntimeMetricsChart({
  kind,
  points,
  targetLabel
}: {
  kind: RuntimeMetricKind;
  points: RuntimeMetricPoint[];
  targetLabel: string;
}) {
  const chartRef = useRef<HTMLDivElement>(null);
  const chartInstanceRef = useRef<ReturnType<typeof echarts.init> | null>(null);
  const option = useMemo<echarts.EChartsCoreOption>(() => {
    const percentage = kind === 'cpu' || kind === 'environment_memory';
    return {
      animation: false,
      color: ['#00ab73', '#4f7cff', '#7b8982'],
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
        name: percentage ? '%' : kind === 'process_memory' ? 'MB' : 'KB/s',
        min: 0,
        max: percentage ? 100 : undefined,
        nameTextStyle: { color: '#7b8982', fontSize: 11 },
        axisLabel: { color: '#7b8982', fontSize: 11 },
        splitLine: { lineStyle: { color: '#e8edea' } }
      },
      series: seriesFor(kind, points, targetLabel)
    };
  }, [kind, points, targetLabel]);

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
