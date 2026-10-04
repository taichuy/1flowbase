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
import { useEChartResourceScope } from './lifecycle';
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
  /** Trusted, stable component identities allow data updates without resetting interaction. */
  readonly replaceMerge?: readonly string[];
  readonly style?: CSSProperties;
  readonly tooltipValueUnit?: string;
  readonly yAxisValueUnit?: string;
  readonly yAxisValueFormatters?: readonly ((value: number) => string)[];
  readonly seriesValueFormatters?: readonly ((value: number) => string)[];
}

export function EChart({
  ariaLabel,
  className,
  option,
  replaceMerge,
  style,
  onDataClick,
  tooltipValueUnit,
  yAxisValueUnit,
  yAxisValueFormatters,
  seriesValueFormatters
}: EChartProps) {
  const resourceScope = useEChartResourceScope();
  const mountRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ReturnType<typeof echarts.init> | null>(null);

  const retained = useRef<{ dispose(): void } | null>(null);
  const applied = useRef<{
    option: string;
    replaceMerge: string;
    yAxisValueUnit?: string;
    tooltipValueUnit?: string;
    yAxisValueFormatters?: EChartProps['yAxisValueFormatters'];
    seriesValueFormatters?: EChartProps['seriesValueFormatters'];
  } | null>(null);
  assertSafeEChartOption(option);
  const optionSignature = JSON.stringify(option);
  const mergeSignature = JSON.stringify(replaceMerge ?? null);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return undefined;

    let chart = chartRef.current;
    if (!chart) {
      chart = echarts.init(mount);
      chartRef.current = chart;
      applied.current = null;
      const owned = chart;
      let released = false;
      let unregister: (() => void) | undefined;
      const resource = {
        dispose() {
          if (released) return;
          released = true;
          unregister?.();
          if (chartRef.current === owned) {
            chartRef.current = null;
            retained.current = null;
            applied.current = null;
          }
          owned.dispose();
        }
      };
      unregister = resourceScope?.register(resource);
      retained.current = resource;
    }
    const activeChart = chart;
    const animation = activeChart.getZr().animation;
    activeChart.getZr().wakeUp();
    const resize = () => {
      const width = mount.clientWidth;
      const height = mount.clientHeight;
      if (
        width > 0 &&
        height > 0 &&
        (activeChart.getWidth() !== width || activeChart.getHeight() !== height)
      ) {
        activeChart.resize();
      }
    };
    const resizeObserver =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(resize);
    resizeObserver?.observe(mount);
    // A hidden retained canvas can have a new allocation on reveal.
    resize();
    const resource = retained.current!;
    return () => {
      resizeObserver?.disconnect();
      if (!resourceScope) {
        resource.dispose();
      } else {
        animation.stop();
        // Activity leaves DOM connected. A removed chart must not accumulate
        // in its surviving page owner (e.g. switching to an empty report).
        queueMicrotask(() => {
          if (!mount.isConnected) resource.dispose();
        });
      }
    };
  }, [resourceScope]);

  useEffect(() => {
    const previous = applied.current;
    if (
      previous?.option === optionSignature &&
      previous.replaceMerge === mergeSignature &&
      previous.tooltipValueUnit === tooltipValueUnit &&
      previous.yAxisValueUnit === yAxisValueUnit &&
      sameFormatters(previous.yAxisValueFormatters, yAxisValueFormatters) &&
      sameFormatters(previous.seriesValueFormatters, seriesValueFormatters)
    )
      return;
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
      ...(yAxisValueFormatters && Array.isArray(option.yAxis)
        ? {
            yAxis: option.yAxis.map((axis, index) => {
              const formatter = yAxisValueFormatters[index];
              if (!formatter) return axis;
              const axisOption = axis as Record<string, EChartValue>;
              return {
                ...axisOption,
                axisLabel: {
                  ...(axisOption.axisLabel as Record<string, EChartValue>),
                  formatter
                }
              };
            })
          }
        : {}),
      ...(seriesValueFormatters && Array.isArray(option.series)
        ? {
            series: option.series.map((series, index) => {
              const formatter = seriesValueFormatters[index];
              if (!formatter) return series;
              const seriesOption = series as Record<string, EChartValue>;
              return {
                ...seriesOption,
                tooltip: {
                  ...(seriesOption.tooltip as Record<string, EChartValue>),
                  valueFormatter: (value: unknown) =>
                    value === null || value === undefined
                      ? '-'
                      : formatter(Number(value))
                }
              };
            })
          }
        : {}),
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
      notMerge: replaceMerge === undefined,
      ...(replaceMerge ? { replaceMerge: [...replaceMerge] } : {}),
      lazyUpdate: true
    });
    applied.current = {
      option: optionSignature,
      replaceMerge: mergeSignature,
      tooltipValueUnit,
      yAxisValueUnit,
      yAxisValueFormatters,
      seriesValueFormatters
    };
  }, [
    option,
    optionSignature,
    replaceMerge,
    mergeSignature,
    tooltipValueUnit,
    yAxisValueUnit,
    yAxisValueFormatters,
    seriesValueFormatters
  ]);

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

function sameFormatters(
  left: readonly ((value: number) => string)[] | undefined,
  right: readonly ((value: number) => string)[] | undefined
): boolean {
  return (
    left === right ||
    Boolean(
      left &&
      right &&
      left.length === right.length &&
      left.every((formatter, index) => formatter === right[index])
    )
  );
}
