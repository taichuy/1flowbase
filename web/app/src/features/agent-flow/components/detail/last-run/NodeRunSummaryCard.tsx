import { Card, Table, Tag, Space, Typography } from 'antd';

import type { NodeLastRun } from '../../../api/runtime';
import { i18nText } from '../../../../../shared/i18n/text';

const STATUS_COLOR: Record<string, string> = {
  succeeded: 'green',
  failed: 'red',
  running: 'blue'
};

function formatDuration(startedAt: string, finishedAt: string | null) {
  if (!finishedAt) {
    return i18nText('agentFlow', 'auto.in_progress');
  }

  const durationMs =
    new Date(finishedAt).getTime() - new Date(startedAt).getTime();

  return `${Math.max(durationMs, 0)}`;
}

function summarizeTokenUsage(lastRun: NodeLastRun) {
  const metricsPayload = lastRun.node_run.metrics_payload;
  if (
    !metricsPayload ||
    typeof metricsPayload !== 'object' ||
    Array.isArray(metricsPayload)
  ) {
    return '—';
  }

  const metricsRecord = metricsPayload as Record<string, unknown>;
  const usage =
    metricsRecord.usage &&
    typeof metricsRecord.usage === 'object' &&
    !Array.isArray(metricsRecord.usage)
      ? (metricsRecord.usage as Record<string, unknown>)
      : metricsRecord;
  const value =
    typeof usage.total_tokens === 'number' ||
    typeof usage.total_tokens === 'string'
      ? usage.total_tokens
      : undefined;

  if (typeof value === 'number') {
    return `${value}`;
  }

  if (typeof value === 'string' && value.trim()) {
    return value.trim();
  }

  return '—';
}

export function NodeRunSummaryCard({ lastRun }: { lastRun: NodeLastRun }) {
  const row = {
    key: 'summary',
    status: lastRun.node_run.status,
    duration: formatDuration(
      lastRun.node_run.started_at,
      lastRun.node_run.finished_at
    ),
    tokens: summarizeTokenUsage(lastRun)
  };

  const columns = [
    {
      title: i18nText('agentFlow', 'auto.status'),
      dataIndex: 'status',
      key: 'status',
      render: (status: string) => (
        <Tag color={STATUS_COLOR[status] ?? 'default'}>{status}</Tag>
      )
    },
    {
      title: i18nText('agentFlow', 'auto.time_taken_ms'),
      dataIndex: 'duration',
      key: 'duration'
    },
    {
      title: 'token',
      dataIndex: 'tokens',
      key: 'tokens'
    }
  ];

  return (
    <Card title={i18nText('agentFlow', 'auto.running_summary')}>
      <Space wrap style={{ marginBottom: 8 }}>
        <Typography.Text type="secondary">
          {i18nText('agentFlow', 'execution.flow_status')}
        </Typography.Text>
        <Tag color={STATUS_COLOR[lastRun.flow_run.status] ?? 'default'}>
          {lastRun.flow_run.status}
        </Tag>
        {lastRun.node_run.execution_identity?.round_path.map((round) => (
          <Typography.Text
            key={`${round.container_execution_id}:${round.round_index}`}
          >
            {round.container_node_id} ·{' '}
            {i18nText('agentFlow', 'execution.round', {
              round: round.round_index + 1
            })}
          </Typography.Text>
        ))}
      </Space>
      <Table
        columns={columns}
        dataSource={[row]}
        pagination={false}
        size="small"
      />
    </Card>
  );
}
