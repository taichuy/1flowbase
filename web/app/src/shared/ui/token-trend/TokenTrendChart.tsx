import { EChart, type EChartOption } from '@1flowbase/charts';
import { theme } from 'antd';
import { useTranslation } from 'react-i18next';
import { formatTokenCount } from '../../i18n/format';

export interface TokenTrendPoint {
  readonly bucket_start: string;
  readonly input_tokens: number | null;
  readonly output_tokens: number | null;
  readonly input_cache_hit_tokens: number | null;
  readonly cache_write_tokens?: number | null;
  /** Backend ratio in [0, 1]; null means no recorded rate. */
  readonly input_cache_hit_rate: number | null;
}

const percentage = (value: number) => `${value}%`;
const replaceSeries = ['series'];

type TokenField = Exclude<keyof TokenTrendPoint, 'bucket_start'>;
export interface TokenTrendChartProps {
  readonly points: readonly TokenTrendPoint[];
  /** Display labels aligned with points; omitted labels use bucket_start verbatim. */
  readonly bucketLabels?: readonly string[];
  readonly labels?: Partial<Record<TokenField, string>>;
  readonly ariaLabel?: string;
  readonly height?: number;
  readonly onDataClick?: (dataIndex: number) => void;
}

export function TokenTrendChart({
  points,
  bucketLabels,
  labels,
  ariaLabel,
  height = 320,
  onDataClick
}: TokenTrendChartProps) {
  const { token } = theme.useToken();
  const { t } = useTranslation('applications');
  const { t: settingsT } = useTranslation('settings');
  const hasCacheWrite = points.some(
    (point) => point.cache_write_tokens !== undefined
  );
  const fields: {
    field: Exclude<TokenField, 'input_cache_hit_rate'>;
    name: string;
    color: string;
  }[] = [
    { field: 'input_tokens', name: t('auto.input_tokens'), color: token.blue },
    {
      field: 'output_tokens',
      name: t('auto.output_tokens'),
      color: token.colorSuccess
    },
    {
      field: 'input_cache_hit_tokens',
      name: t('auto.input_cache_hit_tokens'),
      color: token.cyan
    },
    ...(hasCacheWrite
      ? [
          {
            field: 'cache_write_tokens' as const,
            name: settingsT('auto.request_log_cache_write_tokens'),
            color: token.colorWarning
          }
        ]
      : [])
  ];
  const option: EChartOption = {
    tooltip: { trigger: 'axis' },
    legend: { type: 'scroll', top: 0 },
    grid: { left: 64, right: 64, top: 56, bottom: 48 },
    xAxis: {
      type: 'category',
      data: points.map(
        (point, index) => bucketLabels?.[index] ?? point.bucket_start
      )
    },
    yAxis: [
      { type: 'value', name: 'Token' },
      { type: 'value', name: '%', min: 0, max: 100, splitLine: { show: false } }
    ],
    series: [
      ...fields.map(({ field, name, color }) => ({
        id: field,
        name: labels?.[field] ?? name,
        type: 'line',
        showSymbol: points.length < 32,
        symbolSize: 6,
        connectNulls: false,
        lineStyle: { width: 2, color },
        itemStyle: { color },
        areaStyle: { opacity: 0.08, color },
        data: points.map((point) => point[field] ?? null)
      })),
      {
        id: 'input_cache_hit_rate',
        name: labels?.input_cache_hit_rate ?? t('auto.input_cache_hit_rate'),
        type: 'line',
        yAxisIndex: 1,
        showSymbol: points.length < 32,
        symbolSize: 6,
        connectNulls: false,
        lineStyle: { width: 2, color: token.purple, type: 'dashed' },
        itemStyle: { color: token.purple },
        data: points.map((point) =>
          point.input_cache_hit_rate === null
            ? null
            : Number((point.input_cache_hit_rate * 100).toFixed(2))
        )
      }
    ]
  };
  return (
    <EChart
      ariaLabel={ariaLabel ?? t('statistics.token_trend')}
      style={{ height, width: '100%', minWidth: 0 }}
      option={option}
      replaceMerge={replaceSeries}
      onDataClick={onDataClick}
      yAxisValueFormatters={[formatTokenCount]}
      seriesValueFormatters={[
        ...fields.map(() => formatTokenCount),
        percentage
      ]}
    />
  );
}
