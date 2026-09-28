import { EChart } from '@1flowbase/charts';
import type { EChartOption, EChartProps } from '@1flowbase/charts';

export function ApplicationMonitoringChart({
  ariaLabel,
  option,
  onDataClick,
  yAxisValueFormatters,
  seriesValueFormatters
}: {
  onDataClick?: (dataIndex: number) => void;
  ariaLabel: string;
  option: EChartOption;
} & Pick<EChartProps, 'yAxisValueFormatters' | 'seriesValueFormatters'>) {
  return (
    <EChart
      ariaLabel={ariaLabel}
      className="application-monitoring-chart"
      option={option}
      onDataClick={onDataClick}
      yAxisValueFormatters={yAxisValueFormatters}
      seriesValueFormatters={seriesValueFormatters}
    />
  );
}
