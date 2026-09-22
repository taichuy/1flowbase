import { lazy, Suspense, useState, type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin } from 'antd';
import type { WorkflowTrajectoryEvent } from '@1flowbase/api-client';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { JsonPreviewBlock } from '../../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { TraceActivityDetailContext } from '../activities/activity-model';
import { workflowNodeName, workflowSectionLabel } from './presentation';
const NodeDetail = lazy(() =>
  import('../../ConversationLogPanel').then((module) => ({
    default: module.LazyTraceNodeItem
  }))
);

function NodeIO({
  event,
  loader,
  detailOnly = false
}: {
  event: WorkflowTrajectoryEvent;
  loader: ConversationLogTraceLoader;
  detailOnly?: boolean;
}) {
  const node = useQuery({
    queryKey: ['workflow-event-node-io', event.flow_run_id, event.node_run_id],
    queryFn: async () => {
      const tree = await loader.loadTree(event.flow_run_id);
      let found = tree.nodes.find(
        (item) => item.node_run_id === event.node_run_id
      );
      let cursor = tree.page_info?.next_cursor;
      const visited = new Set<string>();
      while (!found && cursor && !visited.has(cursor)) {
        visited.add(cursor);
        const page = await loader.loadChildren(
          event.flow_run_id,
          'root',
          cursor
        );
        found = page.items.find(
          (item) => item.node_run_id === event.node_run_id
        );
        cursor = page.page_info.has_more ? page.page_info.next_cursor : null;
      }
      return found ?? null;
    },
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  if (node.isPending) return <Spin />;
  if (node.isError)
    return (
      <Alert
        type="error"
        title={i18nText('agentFlow', 'auto.loading_failed')}
        action={
          <Button onClick={() => void node.refetch()}>
            {i18nText('agentFlow', 'auto.retry')}
          </Button>
        }
      />
    );
  if (!node.data)
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('agentFlow', 'trajectory.no_evidence')}
      />
    );
  return (
    <TraceActivityDetailContext.Provider value>
      <Suspense fallback={<Spin />}>
        <NodeDetail
          detailOnly={detailOnly}
          initiallyExpanded
          defaultToolsExpanded={false}
          node={node.data}
          runId={event.flow_run_id}
          traceLoader={loader}
          onLoadArtifact={loader.loadArtifact}
          onLoadArtifacts={loader.loadArtifacts}
        />
      </Suspense>
    </TraceActivityDetailContext.Provider>
  );
}
export function WorkflowEventDetail({
  event,
  runId,
  loader,
  children
}: {
  event: WorkflowTrajectoryEvent;
  runId: string;
  loader: ConversationLogTraceLoader;
  children?: ReactNode;
}) {
  const [nodeLog, setNodeLog] = useState(false);
  const body = useQuery({
    queryKey: ['workflow-trajectory-body', runId, event.event_id],
    enabled:
      !event.native_step &&
      event.category !== 'nodes' &&
      Boolean(loader.loadWorkflowTrajectoryBody),
    queryFn: () => loader.loadWorkflowTrajectoryBody!(runId, event.event_id),
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  return (
    <>
      <div className="provider-trajectory__detail workflow-trajectory__identity">
        <strong>{workflowNodeName(event)}</strong>
        <details>
          <summary>{i18nText('agentFlow', 'auto.metadata')}</summary>
          <small>
            {event.node_id} · {event.node_run_id}
          </small>
          {event.task_run_id ? (
            <span>
              {i18nText('agentFlow', 'trajectory.task')}: {event.task_run_id}
            </span>
          ) : null}
          {event.parent_task_run_id ? (
            <span>
              {i18nText('agentFlow', 'trajectory.parent_task')}:{' '}
              {event.parent_task_run_id}
            </span>
          ) : null}
        </details>
        {event.status ? <span>{event.status}</span> : null}
        {event.node_run_id ? (
          <Button
            size="small"
            onClick={() => setNodeLog(!nodeLog)}
            aria-expanded={nodeLog}
          >
            {i18nText(
              'agentFlow',
              nodeLog ? 'trajectory.back_to_event' : 'trajectory.view_node_logs'
            )}
          </Button>
        ) : null}
      </div>
      {nodeLog ? (
        <NodeIO event={event} loader={loader} />
      ) : (
        <>
          {event.category === 'nodes' ? (
            <NodeIO event={event} loader={loader} detailOnly />
          ) : (
            children
          )}
          {!event.native_step && event.category !== 'nodes' ? (
            <section className="provider-trajectory__detail">
              {body.isLoading ? <Spin /> : null}
              {body.isError ? (
                <Alert
                  type="error"
                  title={i18nText('agentFlow', 'auto.loading_failed')}
                  action={
                    <Button onClick={() => void body.refetch()}>
                      {i18nText('agentFlow', 'auto.retry')}
                    </Button>
                  }
                />
              ) : null}
              {body.data?.sections.map((section, index) => (
                <JsonPreviewBlock
                  key={`${section.kind}:${index}`}
                  title={workflowSectionLabel(section.kind)}
                  value={section.value}
                />
              ))}
              {body.isSuccess && !body.data.sections.length ? (
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={i18nText('agentFlow', 'trajectory.no_evidence')}
                />
              ) : null}
            </section>
          ) : null}
        </>
      )}
    </>
  );
}
