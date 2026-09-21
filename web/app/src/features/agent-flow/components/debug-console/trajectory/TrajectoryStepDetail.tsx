import { useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Tabs } from 'antd';
import type {
  ProviderTrajectoryStep,
  ProviderTrajectoryView
} from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../conversation-log-trace-model';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../shared/i18n/format';

export function TrajectoryStepDetail({
  step,
  loader
}: {
  step: ProviderTrajectoryStep;
  loader: ConversationLogTraceLoader;
}) {
  const [view, setView] = useState<ProviderTrajectoryView>('semantic');
  const { flow_run_id, node_run_id } = step.metadata;
  const body = useInfiniteQuery({
    queryKey: [
      'provider-trajectory-body',
      flow_run_id,
      node_run_id,
      step.event_id,
      view
    ],
    enabled: Boolean(loader.loadTrajectoryBody),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      loader.loadTrajectoryBody!(
        flow_run_id,
        node_run_id,
        step.event_id,
        pageParam,
        view
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchOnWindowFocus: false
  });
  return (
    <section
      className="provider-trajectory__detail"
      aria-label={i18nText('agentFlow', 'trajectory.inspector')}
    >
      <div className="provider-trajectory__detail-meta">
        <span>
          {step.metadata.source === 'ai_native'
            ? i18nText('agentFlow', 'trajectory.native_source')
            : i18nText('agentFlow', 'trajectory.supplier_source')}
        </span>
        <time>{formatDateTime(step.created_at)}</time>
        <span>{step.metadata.node_id || node_run_id}</span>
        <span title={step.metadata.invocation_id}>
          {i18nText('agentFlow', 'trajectory.invocation')}:{' '}
          {step.metadata.invocation_id}
        </span>
      </div>
      <Tabs
        size="small"
        activeKey={view}
        onChange={(key) => setView(key as ProviderTrajectoryView)}
        items={[
          {
            key: 'semantic',
            label: i18nText('agentFlow', 'trajectory.step_detail')
          },
          {
            key: 'protocol',
            label: i18nText('agentFlow', 'trajectory.raw_evidence')
          }
        ]}
      />
      {view === 'protocol' &&
      body.data?.pages[0]?.evidence_scope === 'invocation' ? (
        <Alert
          type="info"
          showIcon
          title={i18nText('agentFlow', 'trajectory.invocation_evidence')}
        />
      ) : null}
      {body.isLoading ? <Spin /> : null}
      {body.isError ? (
        <Alert
          type="error"
          showIcon
          title={i18nText('agentFlow', 'auto.loading_failed')}
          action={
            <Button onClick={() => void body.refetch()}>
              {i18nText('agentFlow', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
      {body.isSuccess && !body.data.pages.some((page) => page.items.length) ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'trajectory.no_evidence')}
        />
      ) : null}
      {body.data?.pages
        .flatMap((page) => page.items)
        .map((evidence) => (
          <pre key={evidence.event_id} className="provider-trajectory__body">
            {evidence.body}
          </pre>
        ))}
      {body.hasNextPage ? (
        <Button
          size="small"
          loading={body.isFetchingNextPage}
          onClick={() => void body.fetchNextPage()}
        >
          {i18nText('agentFlow', 'trajectory.more_evidence')}
        </Button>
      ) : null}
    </section>
  );
}
