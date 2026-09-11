import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Breadcrumb, Button, Descriptions, Drawer, Space, Table, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type { GatewayLogEntry, GatewayLogQuery } from '@1flowbase/api-client';
import { fetchGatewayLogs } from '../../../api/gateway-logs';

type Level = { title: string; query: GatewayLogQuery };

export function GatewayLogs({ applicationId }: { applicationId: string }) {
  const { t } = useTranslation('applications');
  const [levels, setLevels] = useState<Level[]>([]);
  const [page, setPage] = useState(1);
  const [detail, setDetail] = useState<GatewayLogEntry | null>(null);
  const query = { ...levels.at(-1)?.query, page, page_size: 20 };
  const result = useQuery({ queryKey: ['applications', applicationId, 'gateway-logs', query], queryFn: () => fetchGatewayLogs(applicationId, query) });
  const unknown = t('gateway.unknown');
  function enter(entry: GatewayLogEntry) {
    const selector = entry.kind === 'conversation' ? { conversation_id: entry.id }
      : entry.kind === 'turn' ? { turn_id: entry.id }
      : entry.kind === 'invocation' || entry.kind === 'unknown_turn' ? { flow_run_id: entry.flow_run_id ?? entry.id } : null;
    if (!selector) { setDetail(entry); return; }
    setLevels([...levels, { title: entry.title, query: selector }]);
    setPage(1);
  }
  return <Space orientation="vertical" size="middle" style={{ width: '100%', minWidth: 0 }}>
    <Space wrap>
      <Breadcrumb items={[
        { title: <Button type="link" onClick={() => { setLevels([]); setPage(1); }}>{t('gateway.conversations')}</Button> },
        ...levels.map((level, index) => ({ title: <Button type="link" style={{ maxWidth: 240 }} onClick={() => { setLevels(levels.slice(0, index + 1)); setPage(1); }}><Typography.Text ellipsis title={level.title}>{level.title}</Typography.Text></Button> }))
      ]} />
      <Button onClick={() => void result.refetch()} loading={result.isFetching}>{t('gateway.refresh')}</Button>
    </Space>
    {result.isError && <Alert type="error" title={t('gateway.load_failed')} />}
    <Table<GatewayLogEntry> rowKey="id" loading={result.isLoading} dataSource={result.data?.items ?? []} tableLayout="fixed" scroll={{ x: 1080 }}
      pagination={{ current: result.data?.page ?? page, pageSize: 20, total: result.data?.total ?? 0, showSizeChanger: false, onChange: setPage }}
      columns={[
        { title: t('gateway.subject'), key: 'title', width: 340, render: (_, entry) => <Space orientation="vertical" size={0} style={{ width: '100%', minWidth: 0 }}><Button type="link" style={{ width: '100%', justifyContent: 'flex-start', paddingInline: 0 }} onClick={() => enter(entry)}><Typography.Text ellipsis title={entry.title}>{entry.title}</Typography.Text></Button><Typography.Text type="secondary">{t(`gateway.kinds.${entry.kind}`, { defaultValue: unknown })} · {entry.identity_status}</Typography.Text></Space> },
        { title: t('gateway.task_status'), key: 'state', width: 180, render: (_, entry) => <Space orientation="vertical" size={0}><span>{entry.completion_status === 'unknown' ? unknown : entry.completion_status}</span><Typography.Text type="secondary">{entry.observations.map(observation => t(`gateway.observations.${observation}`, { defaultValue: unknown })).join(' · ')}</Typography.Text></Space> },
        { title: t('gateway.calls_attempts'), key: 'calls', width: 120, render: (_, entry) => `${entry.metrics.invocation_count} / ${entry.metrics.attempt_count}` },
        { title: t('gateway.tokens'), key: 'tokens', width: 100, render: (_, entry) => entry.metrics.total_tokens ?? unknown },
        { title: t('gateway.cost'), key: 'cost', width: 190, render: (_, entry) => <>{entry.metrics.costs.map(cost => <div key={cost.currency_code}>{cost.amount} {cost.currency_code}</div>)}{entry.metrics.unknown_cost_attempts > 0 && <span>{unknown} ({entry.metrics.unknown_cost_attempts})</span>}</> },
        { title: t('gateway.details'), key: 'details', width: 150, render: (_, entry) => <Button onClick={() => setDetail(entry)}>{t('gateway.details')}</Button> }
      ]} />
    <Drawer open={detail !== null} onClose={() => setDetail(null)} title={<Typography.Text ellipsis title={detail?.title}>{detail?.title}</Typography.Text>} size="large" destroyOnHidden>
      {detail && <Space orientation="vertical" size="large" style={{ width: '100%', minWidth: 0 }}>
        <Descriptions column={1} items={[
          { key: 'identity', label: t('gateway.identity'), children: detail.identity_status },
          { key: 'sources', label: t('gateway.identity_sources'), children: detail.identity_sources.join(' · ') || unknown },
          { key: 'fork', label: 'forked_from_thread_id', children: detail.forked_from_thread_id ?? unknown },
          { key: 'parent', label: t('gateway.parent_task'), children: detail.parent_task_id ?? unknown },
          { key: 'thread', label: 'thread_id', children: detail.thread_id ?? unknown },
          { key: 'turn', label: 'turn_id', children: detail.client_turn_id ?? unknown },
          { key: 'relation', label: t('gateway.relation'), children: [detail.relation_status, detail.parent_thread_id, detail.parent_turn_id].filter(Boolean).join(' · ') },
          { key: 'cause', label: t('gateway.cause'), children: detail.caused_by_run_id ?? unknown },
          { key: 'status', label: t('gateway.task_status'), children: detail.completion_status === 'unknown' ? unknown : detail.completion_status },
          { key: 'run', label: t('gateway.run_status'), children: detail.status ?? unknown },
          { key: 'elapsed', label: t('gateway.elapsed'), children: detail.metrics.elapsed_ms == null ? unknown : `${detail.metrics.elapsed_ms} ms` },
          { key: 'model', label: t('gateway.model_time'), children: detail.metrics.model_duration_ms == null ? unknown : `${detail.metrics.model_duration_ms} ms` },
          { key: 'tool', label: t('gateway.tool_wait'), children: detail.metrics.tool_result_wait_ms == null ? unknown : `${detail.metrics.tool_result_wait_ms} ms` },
          { key: 'attempt', label: 'Attempt', children: detail.attempt_index ?? unknown },
          { key: 'error', label: t('gateway.error'), children: detail.error_code ?? '—' }
        ]} />
        {detail.messages.map(message => <section key={message.id}>
          <Typography.Title level={5}>{message.tool_name ?? message.phase ?? message.kind}</Typography.Title>
          <Typography.Paragraph style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{message.text}</Typography.Paragraph>
          {message.call_id && <>
            <Typography.Text code>{message.call_id}</Typography.Text>
            <Typography.Paragraph style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{message.tool_input}</Typography.Paragraph>
            <Typography.Text>{message.result_received ? t('gateway.result_received') : t('gateway.result_unknown')}</Typography.Text>
            <Typography.Paragraph style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{message.tool_result}</Typography.Paragraph>
          </>}
        </section>)}
        {detail.messages_has_more && <Alert title={t('gateway.more_messages')} type="info" />}
        {detail.flow_run_id && <Button href={`?run_id=${encodeURIComponent(detail.flow_run_id)}`}>{t('gateway.original_run')}</Button>}
      </Space>}
    </Drawer>
  </Space>;
}
