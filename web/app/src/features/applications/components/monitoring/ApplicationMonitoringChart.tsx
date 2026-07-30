import { EChart } from '@1flowbase/charts';
import type { EChartOption } from '@1flowbase/charts';

export function ApplicationMonitoringChart({
  ariaLabel,
  option
}: {
  ariaLabel: string;
  option: EChartOption;
}) {
  return (
    <EChart
      ariaLabel={ariaLabel}
      className="application-monitoring-chart"
      option={option}
    />
  );
}
