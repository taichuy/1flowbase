import type { ApplicationRunMonitoringReport } from '../api/runtime';
import { i18nText } from '../../../shared/i18n/text';
import {
  formatInteger,
  formatTrendBucket,
  sourceLabel
} from './application-monitoring-format';

function buildTokenTrendOption(report: ApplicationRunMonitoringReport) {
  const gradientColor = (colors: [string, string]) => ({
    type: 'linear',
    x: 0,
    y: 0,
    x2: 0,
    y2: 1,
    colorStops: [
      { offset: 0, color: colors[0] },
      { offset: 1, color: colors[1] }
    ]
  });
  const tokenTrendSeries = [
    {
      name: i18nText('applications', 'auto.total_tokens'),
      color: '#2f54eb',
      areaColor: undefined,
      lineWidth: 2.4,
      z: 4,
      data: report.tokens_trend.map((point) => point.total_tokens)
    },
    {
      name: i18nText('applications', 'auto.input_tokens'),
      color: '#1677ff',
      areaColor: ['rgba(22, 119, 255, 0.18)', 'rgba(22, 119, 255, 0.02)'] as [
        string,
        string
      ],
      lineWidth: 2,
      z: 3,
      data: report.tokens_trend.map((point) => point.input_tokens)
    },
    {
      name: i18nText('applications', 'auto.output_tokens'),
      color: '#22c55e',
      areaColor: ['rgba(34, 197, 94, 0.18)', 'rgba(34, 197, 94, 0.02)'] as [
        string,
        string
      ],
      lineWidth: 2,
      z: 2,
      data: report.tokens_trend.map((point) => point.output_tokens)
    },
    {
      name: i18nText('applications', 'auto.input_cache_hit_tokens'),
      color: '#f59e0b',
      areaColor: ['rgba(245, 158, 11, 0.2)', 'rgba(245, 158, 11, 0.02)'] as [
        string,
        string
      ],
      lineWidth: 2,
      z: 2,
      data: report.tokens_trend.map((point) => point.input_cache_hit_tokens)
    }
  ];

  return {
    color: tokenTrendSeries.map((series) => series.color),
    grid: {
      left: 54,
      right: 20,
      top: 28,
      bottom: 58
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(255, 255, 255, 0.95)',
      borderColor: '#f0f0f0',
      borderWidth: 1,
      textStyle: { color: '#1f1f1f', fontSize: 12 }
    },
    legend: {
      bottom: 0,
      itemGap: 16
    },
    xAxis: {
      type: 'category',
      data: report.tokens_trend.map((point) =>
        formatTrendBucket(point.bucket_start, report.meta.bucket)
      ),
      axisLine: {
        lineStyle: {
          color: '#f0f0f0'
        }
      },
      axisLabel: {
        color: '#8c8c8c'
      }
    },
    yAxis: [
      {
        type: 'value',
        axisLine: { show: false },
        axisLabel: { color: '#8c8c8c' },
        splitLine: {
          lineStyle: {
            color: 'rgba(0, 0, 0, 0.05)',
            type: 'dashed'
          }
        }
      }
    ],
    series: tokenTrendSeries.map((series) => ({
      name: series.name,
      type: 'line',
      smooth: true,
      symbol: 'circle',
      symbolSize: 6,
      showSymbol: false,
      z: series.z,
      emphasis: { focus: 'series' },
      lineStyle: { width: series.lineWidth, color: series.color },
      itemStyle: { color: series.color },
      ...(series.areaColor
        ? { areaStyle: { color: gradientColor(series.areaColor) } }
        : {}),
      data: series.data
    }))
  };
}

function buildProtocolOption(report: ApplicationRunMonitoringReport) {
  return {
    grid: {
      left: 54,
      right: 20,
      top: 20,
      bottom: 38
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(255, 255, 255, 0.95)',
      borderColor: '#f0f0f0',
      borderWidth: 1,
      textStyle: { color: '#1f1f1f', fontSize: 12 }
    },
    xAxis: {
      type: 'category',
      data: report.protocols.map((item) => item.protocol),
      axisLine: {
        lineStyle: {
          color: '#f0f0f0'
        }
      },
      axisLabel: {
        color: '#8c8c8c'
      }
    },
    yAxis: {
      type: 'value',
      axisLine: { show: false },
      axisLabel: { color: '#8c8c8c' },
      splitLine: {
        lineStyle: {
          color: 'rgba(0, 0, 0, 0.05)',
          type: 'dashed'
        }
      }
    },
    series: [
      {
        name: i18nText('applications', 'auto.request_count'),
        type: 'bar',
        barMaxWidth: 24,
        itemStyle: {
          borderRadius: [4, 4, 0, 0],
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: '#1677ff' },
              { offset: 1, color: '#85a5ff' }
            ]
          }
        },
        data: report.protocols.map((item) => item.request_count)
      }
    ]
  };
}

function buildSourceOption(report: ApplicationRunMonitoringReport) {
  const totalRequests = report.sources.reduce(
    (sum, item) => sum + item.request_count,
    0
  );
  return {
    color: ['#1677ff', '#52c41a', '#faad14'],
    tooltip: {
      trigger: 'item',
      backgroundColor: 'rgba(255, 255, 255, 0.95)',
      borderColor: '#f0f0f0',
      borderWidth: 1,
      textStyle: { color: '#1f1f1f', fontSize: 12 }
    },
    legend: {
      bottom: 0,
      itemGap: 16
    },
    title: {
      text: formatInteger(totalRequests),
      subtext: i18nText('applications', 'auto.total_requests'),
      left: 'center',
      top: '38%',
      textStyle: {
        fontSize: 20,
        fontWeight: 'bold',
        color: '#1f1f1f',
        fontFamily: 'Outfit, Inter, sans-serif'
      },
      subtextStyle: {
        fontSize: 12,
        color: '#8c8c8c'
      }
    },
    series: [
      {
        name: i18nText('applications', 'auto.source'),
        type: 'pie',
        radius: ['55%', '75%'],
        center: ['50%', '46%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 4,
          borderColor: '#ffffff',
          borderWidth: 2
        },
        label: {
          show: false
        },
        data: report.sources.map((item) => ({
          name: sourceLabel(item.invocation_source),
          value: item.request_count
        }))
      }
    ]
  };
}

export { buildProtocolOption, buildSourceOption, buildTokenTrendOption };
