import { formatDate, formatDateTime, formatNumber, formatTime as formatClockTime } from '../../../shared/i18n/format';
import { i18nText } from '../../../shared/i18n/text';
import type { ApplicationRunMonitoringBucket, ApplicationRunMonitoringReport } from '../api/runtime';

export type MonitoringTimeRange = 1 | 7 | 28 | 90 | 365;

function monitoringTimeRangeOptions(): Array<{
  label: string;
  value: MonitoringTimeRange;
}> {
  return [
    { label: i18nText("applications", "auto.past_twenty_four_hours"), value: 1 },
    { label: i18nText("applications", "auto.past_seven_days"), value: 7 },
    { label: i18nText("applications", "auto.past_four_weeks"), value: 28 },
    { label: i18nText("applications", "auto.past_three_months"), value: 90 },
    { label: i18nText("applications", "auto.past_twelve_months"), value: 365 }
  ];
}

function getMonitoringBucket(
  range: MonitoringTimeRange
): ApplicationRunMonitoringBucket {
  if (range <= 1) {
    return 'hour';
  }
  if (range >= 180) {
    return 'month';
  }
  if (range >= 60) {
    return 'week';
  }
  return 'day';
}

function formatInteger(value: number) {
  return formatNumber(value, { maximumFractionDigits: 0 });
}

function formatTokenAmount(value: number) {
  const absoluteValue = Math.abs(value);
  const unit = [
    { threshold: 1_000_000_000, suffix: 'B' },
    { threshold: 1_000_000, suffix: 'M' },
    { threshold: 1_000, suffix: 'K' }
  ].find((candidate) => absoluteValue >= candidate.threshold);

  if (!unit) {
    return formatInteger(value);
  }

  return `${formatNumber(value / unit.threshold, {
    maximumFractionDigits: 1
  })}${unit.suffix}`;
}

function formatDecimal(value: number, digits = 1) {
  return formatNumber(value, {
    maximumFractionDigits: digits,
    minimumFractionDigits: digits
  });
}

function formatPercent(value: number) {
  return `${formatDecimal(value * 100, 1)}%`;
}

function formatSignedPercent(value: number) {
  const prefix = value > 0 ? '+' : '';
  return `${prefix}${formatPercent(value)}`;
}

function tokenComparisonMetric(report: ApplicationRunMonitoringReport) {
  if (
    report.tokens_comparison.previous_total_tokens_sum === 0 &&
    report.tokens.total_tokens_sum > 0
  ) {
    return {
      label: i18nText('applications', 'auto.token_increase_from_empty'),
      value: formatTokenAmount(report.tokens.total_tokens_sum)
    };
  }

  return {
    label: i18nText('applications', 'auto.token_change'),
    value: formatSignedPercent(report.tokens_comparison.token_change_rate)
  };
}

function formatDuration(value: number | null | undefined) {
  if (value == null || Number.isNaN(value)) {
    return '-';
  }
  if (value < 1000) {
    return `${formatInteger(value)}ms`;
  }
  return `${formatDecimal(value / 1000, 1)}s`;
}

function formatTime(value: string | null | undefined) {
  if (!value) {
    return '-';
  }
  return formatDateTime(value);
}

function formatTrendBucket(
  value: string,
  bucket: ApplicationRunMonitoringBucket
) {
  if (bucket === 'hour') {
    return formatClockTime(value, {
      hour: '2-digit',
      minute: '2-digit'
    });
  }
  if (bucket === 'month') {
    return formatDate(value, {
      year: 'numeric',
      month: '2-digit'
    });
  }
  return formatDate(value);
}

function sourceLabel(source: string) {
  switch (source) {
    case 'agent_flow_api':
      return i18nText('applications', 'auto.invocation_source_agent_flow_api');
    case 'workflow_http':
      return i18nText('applications', 'auto.invocation_source_workflow_http');
    case 'workflow_schedule':
      return i18nText('applications', 'auto.invocation_source_workflow_schedule');
    default:
      return i18nText('applications', 'auto.invocation_source_debug');
  }
}

export {
  formatDecimal,
  formatDuration,
  formatInteger,
  formatPercent,
  formatTime,
  formatTokenAmount,
  formatTrendBucket,
  getMonitoringBucket,
  monitoringTimeRangeOptions,
  sourceLabel,
  tokenComparisonMetric,
};
