import QuestionCircleOutlined from '@ant-design/icons/es/icons/QuestionCircleOutlined';
import { Button, Result, Tooltip, Typography } from 'antd';
import type { ReactNode } from 'react';
import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import type { ApplicationRuntimeActivity } from '../api/runtime';
import {
  formatDecimal,
  formatInteger,
  formatPercent
} from '../lib/application-monitoring-format';
import './application-monitoring-page.css';
export { ApplicationTaskStatistics as ApplicationStatisticsPage } from '../components/statistics/ApplicationTaskStatistics';
function MonitoringPanel({
  children,
  title
}: {
  children: ReactNode;
  title: ReactNode;
}) {
  return (
    <section className="application-monitoring-panel">
      <Typography.Title level={5}>{title}</Typography.Title>
      {children}
    </section>
  );
}

function runtimeHealthLabel(
  state: ApplicationRuntimeActivity['health']['state']
) {
  switch (state) {
    case 'busy':
      return i18nText('applications', 'auto.runtime_health_busy');
    case 'slow':
      return i18nText('applications', 'auto.runtime_health_slow');
    case 'unstable':
      return i18nText('applications', 'auto.runtime_health_unstable');
    case 'failing':
      return i18nText('applications', 'auto.runtime_health_failing');
    case 'failing_now':
      return i18nText('applications', 'auto.runtime_health_failing_now');
    case 'healthy':
    default:
      return i18nText('applications', 'auto.runtime_health_healthy');
  }
}

function runtimeHealthTone(
  state: ApplicationRuntimeActivity['health']['state']
): 'blue' | 'green' | 'gold' | 'red' | 'purple' | 'cyan' {
  switch (state) {
    case 'healthy':
      return 'green';
    case 'busy':
      return 'cyan';
    case 'slow':
      return 'gold';
    case 'unstable':
      return 'purple';
    case 'failing':
    case 'failing_now':
      return 'red';
    default:
      return 'blue';
  }
}

function runtimeTrendLabel(
  trend: ApplicationRuntimeActivity['health']['throughput_trend']
) {
  switch (trend) {
    case 'rising':
      return i18nText('applications', 'auto.trend_rising');
    case 'falling':
      return i18nText('applications', 'auto.trend_falling');
    case 'steady':
    default:
      return i18nText('applications', 'auto.trend_steady');
  }
}

function RuntimeActivityMetric({
  label,
  value,
  tone = 'blue'
}: {
  label: string;
  value: string | number;
  tone?: 'blue' | 'green' | 'gold' | 'red' | 'purple' | 'cyan';
}) {
  return (
    <div className={`runtime-activity-metric runtime-activity-metric--${tone}`}>
      <span className="runtime-activity-metric__label">{label}</span>
      <span className="runtime-activity-metric__value">{value}</span>
    </div>
  );
}

function RuntimeActivityGroup({
  children,
  title
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <section className="runtime-activity-group">
      <Typography.Text
        className="runtime-activity-group__title"
        type="secondary"
      >
        {title}
      </Typography.Text>
      <div className="runtime-activity-group__metrics">{children}</div>
    </section>
  );
}

function RuntimeActivityTitle() {
  return (
    <span className="runtime-activity-panel__title">
      {i18nText('applications', 'auto.runtime_activity')}
      <Tooltip
        title={
          <span>
            {i18nText('applications', 'auto.current_instance_runtime_data')}
            <br />
            {i18nText('applications', 'auto.runtime_activity_memory_scope')}
          </span>
        }
      >
        <Button
          aria-label={i18nText('applications', 'auto.runtime_activity')}
          className="runtime-activity-panel__help"
          icon={<QuestionCircleOutlined aria-hidden="true" />}
          size="small"
          type="text"
        />
      </Tooltip>
    </span>
  );
}

export function RuntimeActivityPanel({
  activity,
  loading,
  error
}: {
  activity?: ApplicationRuntimeActivity;
  loading: boolean;
  error: boolean;
}) {
  if (loading && !activity) {
    return (
      <MonitoringPanel title={<RuntimeActivityTitle />}>
        <LoadingState compact />
      </MonitoringPanel>
    );
  }

  if (error || !activity) {
    return (
      <MonitoringPanel title={<RuntimeActivityTitle />}>
        <Result
          status="warning"
          title={i18nText('applications', 'auto.runtime_activity_load_failed')}
        />
      </MonitoringPanel>
    );
  }

  const active = activity.active;
  const pressure = activity.pressure;
  const health = activity.health;
  const fiveMinutes = activity.windows.five_minutes;

  return (
    <MonitoringPanel title={<RuntimeActivityTitle />}>
      <div className="runtime-activity-panel__groups">
        <RuntimeActivityGroup
          title={i18nText('applications', 'auto.runtime_group_overview')}
        >
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.runtime_health')}
            value={runtimeHealthLabel(health.state)}
            tone={runtimeHealthTone(health.state)}
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.active_total')}
            value={formatInteger(active.total)}
            tone="blue"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.process_peak')}
            value={formatInteger(activity.peaks.process_peak_concurrency)}
            tone="blue"
          />
        </RuntimeActivityGroup>

        <RuntimeActivityGroup
          title={i18nText('applications', 'auto.runtime_group_protocol')}
        >
          <RuntimeActivityMetric
            label="HTTP"
            value={formatInteger(active.http_requests)}
            tone="cyan"
          />
          <RuntimeActivityMetric
            label="SSE"
            value={formatInteger(active.sse_connections)}
            tone="green"
          />
          <RuntimeActivityMetric
            label="WebSocket"
            value={formatInteger(active.websocket_connections)}
            tone="purple"
          />
        </RuntimeActivityGroup>

        <RuntimeActivityGroup
          title={i18nText('applications', 'auto.runtime_group_execution')}
        >
          <RuntimeActivityMetric
            label={i18nText(
              'applications',
              'auto.application_executions_active'
            )}
            value={formatInteger(active.application_executions)}
            tone="gold"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.tool_calls_active')}
            value={formatInteger(active.tool_calls)}
            tone="purple"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.model_requests_active')}
            value={formatInteger(active.model_requests)}
            tone="blue"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.waiting_active')}
            value={active.waiting == null ? '-' : formatInteger(active.waiting)}
            tone="gold"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.slow_active_executions')}
            value={formatInteger(pressure.slow_active_executions)}
            tone="gold"
          />
        </RuntimeActivityGroup>

        <RuntimeActivityGroup
          title={i18nText('applications', 'auto.runtime_group_five_minutes')}
        >
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.five_minute_failure_rate')}
            value={formatPercent(health.failure_rate_5m)}
            tone="red"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.five_minute_disconnect_rate')}
            value={formatPercent(health.disconnect_rate_5m)}
            tone="purple"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.completed_five_minutes')}
            value={formatInteger(fiveMinutes.completed)}
            tone="green"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.five_minute_throughput')}
            value={formatDecimal(health.throughput_5m_per_minute, 1)}
            tone="green"
          />
          <RuntimeActivityMetric
            label={i18nText('applications', 'auto.throughput_trend')}
            value={runtimeTrendLabel(health.throughput_trend)}
            tone="cyan"
          />
        </RuntimeActivityGroup>
      </div>
    </MonitoringPanel>
  );
}
