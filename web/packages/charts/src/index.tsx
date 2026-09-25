import { useEffect, useRef } from 'react';
import type { CSSProperties } from 'react';

import {
  BarChart,
  FunnelChart,
  GaugeChart,
  LineChart,
  PieChart,
  RadarChart
} from 'echarts/charts';
import {
  GridComponent,
  LegendComponent,
  RadarComponent,
  TitleComponent,
  TooltipComponent
} from 'echarts/components';
import * as echarts from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';

import { assertSafeEChartOption } from './safe-option';
import type { EChartOption, EChartValue } from './safe-option';

echarts.use([
  BarChart,
  FunnelChart,
  GaugeChart,
  LineChart,
  PieChart,
  RadarChart,
  GridComponent,
  LegendComponent,
  RadarComponent,
  TitleComponent,
  TooltipComponent,
  CanvasRenderer
]);

export type { EChartOption, EChartValue };

export interface EChartProps {
  readonly onDataClick?: (dataIndex: number) => void;
  readonly ariaLabel?: string;
  readonly className?: string;
  readonly option: EChartOption;
  readonly style?: CSSProperties;
  readonly tooltipValueUnit?: string;
  readonly yAxisValueUnit?: string;
}

export function EChart({
  ariaLabel,
  className,
  option,
  style,
  onDataClick,
  tooltipValueUnit,
  yAxisValueUnit
}: EChartProps) {
  const mountRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ReturnType<typeof echarts.init> | null>(null);

  assertSafeEChartOption(option);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return undefined;

    const chart = echarts.init(mount);
    chartRef.current = chart;
    const resizeObserver =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => chart.resize());
    resizeObserver?.observe(mount);

    return () => {
      resizeObserver?.disconnect();
      chartRef.current = null;
      chart.dispose();
    };
  }, []);

  useEffect(() => {
    const yAxis =
      option.yAxis &&
      typeof option.yAxis === 'object' &&
      !Array.isArray(option.yAxis)
        ? (option.yAxis as {
            readonly axisLabel?: Record<string, unknown>;
            readonly [key: string]: unknown;
          })
        : undefined;
    const safeOption = {
      ...option,
      ...(yAxisValueUnit && yAxis
        ? {
            yAxis: {
              ...yAxis,
              axisLabel: {
                ...yAxis.axisLabel,
                formatter: (value: number) => `${value} ${yAxisValueUnit}`
              }
            }
          }
        : {}),
      tooltip:
        option.tooltip && typeof option.tooltip === 'object'
          ? {
              ...option.tooltip,
              renderMode: 'richText',
              ...(tooltipValueUnit
                ? {
                    valueFormatter: (value: unknown) =>
                      `${value} ${tooltipValueUnit}`
                  }
                : {})
            }
          : option.tooltip
    };
    chartRef.current?.setOption(safeOption, {
      notMerge: true,
      lazyUpdate: true
    });
  }, [option, tooltipValueUnit, yAxisValueUnit]);

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart || !onDataClick) return;
    const handleClick = (event: { dataIndex?: number }) => {
      if (typeof event.dataIndex === 'number') onDataClick(event.dataIndex);
    };
    chart.on('click', handleClick);
    return () => {
      if (chartRef.current === chart) chart.off('click', handleClick);
    };
  }, [onDataClick]);

  return (
    <div
      ref={mountRef}
      aria-label={ariaLabel}
      className={className}
      role="img"
      style={style}
    />
  );
}
