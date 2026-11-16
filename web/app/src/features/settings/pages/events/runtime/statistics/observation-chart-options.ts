import type { EventSourceRuntimeSnapshot } from '@1flowbase/api-client';
import type { EChartOption } from '@1flowbase/charts';
import type { theme } from 'antd';
import type { ObservationSample } from './observation-statistics';

type ObservationChartTheme = ReturnType<typeof theme.useToken>['token'];

export function observationTrendOption(
  samples: ObservationSample[],
  categories: string[],
  token: ObservationChartTheme
): EChartOption {
  return {
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
  };
}

export function observationCleanupOption(
  views: EventSourceRuntimeSnapshot[],
  labels: string[],
  cleanupNames: string[],
  token: ObservationChartTheme
): EChartOption {
  return {
    animation: false,
    color: [token.colorPrimary, token.colorWarning, token.colorError],
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
      data: labels,
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
}
