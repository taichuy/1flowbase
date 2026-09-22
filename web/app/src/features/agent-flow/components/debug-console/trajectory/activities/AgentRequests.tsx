import { PageStatus } from './ActivityPageStatus';
import { lazy, Suspense, useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Tabs } from 'antd';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import ApartmentOutlined from '@ant-design/icons/es/icons/ApartmentOutlined';
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import type { ConversationLogTraceNodeSummary } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../../shared/i18n/format';
import { TrajectoryStepDetail } from '../TrajectoryStepDetail';
import {
  invocationKey,
  purposeLabel,
  stepLabel,
  integrityLabel
} from '../trajectory-presentation';
import { useProgressiveTrajectory } from '../use-progressive-trajectory';
import { TraceActivityDetailContext } from './activity-model';
import { useChildren, type Scope } from './activity-query';

const NodeDetail = lazy(() =>
  import('../../ConversationLogPanel').then((module) => ({
    default: module.LazyTraceNodeItem
  }))
);
export type AgentRequestSelection = {
  node: ConversationLogTraceNodeSummary;
  invocation: string;
};

// The backend supplies the subagent's exact source run. Never infer ownership
// from its prompt, timestamp, or a parent workflow node's identifier.
function useAgentRequests(
  scope: Scope,
  node: ConversationLogTraceNodeSummary,
  progressive = true
) {
  const pages = useInfiniteQuery({
    queryKey: [
      'provider-trajectory',
      node.source_flow_run_id,
      'run',
      undefined
    ],
    enabled:
      scope.active &&
      Boolean(node.source_flow_run_id && scope.loader.loadRunTrajectory),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) =>
      scope.loader.loadRunTrajectory!(node.source_flow_run_id!, pageParam),
    getNextPageParam: (page, _pages, cursor) =>
      page.next_cursor != null &&
      (cursor === undefined || page.next_cursor > cursor)
        ? page.next_cursor
        : undefined,
    staleTime: 60_000,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(scope.active && progressive, pages);
  const requests = new Map<string, ProviderTrajectoryStep[]>();
  for (const step of pages.data?.pages.flatMap((page) => page.items) ?? []) {
    const key = invocationKey(step);
    const steps = requests.get(key) ?? [];
    steps.push(step);
    requests.set(key, steps);
  }
  return { pages, requests };
}

export function AgentRequestGroup({
  scope,
  node,
  onSelect,
  selected
}: {
  scope: Scope;
  node: ConversationLogTraceNodeSummary;
  onSelect: (selection: AgentRequestSelection) => void;
  selected?: AgentRequestSelection | null;
}) {
  const children = useChildren(
    scope,
    node.trace_node_id,
    node.node_kind === 'agent_group' && node.has_children
  );
  if (node.node_kind === 'agent_group')
    return (
      <>
        <PageStatus pages={children} />
        {children.data?.pages
          .flatMap((page) => page.items)
          .map((child) => (
            <AgentRequestGroup
              key={child.trace_node_id}
              scope={scope}
              node={child}
              onSelect={onSelect}
              selected={selected}
            />
          ))}
      </>
    );
  return (
    <AgentRequests
      scope={scope}
      node={node}
      onSelect={onSelect}
      selected={selected}
    />
  );
}

function AgentRequests({
  scope,
  node,
  onSelect,
  selected
}: Parameters<typeof AgentRequestGroup>[0]) {
  const { pages, requests } = useAgentRequests(scope, node);
  return (
    <section
      className="agent-requests"
      aria-label={node.source_flow_run_id ?? node.trace_node_id}
    >
      <header className="agent-requests__heading">
        <strong>
          {node.node_mode ||
            i18nText('agentFlow', 'trajectory.agent_activities')}
        </strong>
        <span title={node.source_flow_run_id ?? node.trace_node_id}>
          {node.source_flow_run_id ?? node.trace_node_id}
        </span>
        <small>{node.status}</small>
      </header>
      {!node.source_flow_run_id || !scope.loader.loadRunTrajectory ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'trajectory.source_not_recorded')}
        />
      ) : (
        <>
          <PageStatus
            pages={{
              isError: pages.isError,
              isFetching: pages.isFetching,
              refetch: () =>
                pages.isFetchNextPageError
                  ? pages.fetchNextPage()
                  : pages.refetch()
            }}
          />
          {!pages.isPending && !pages.isError && !requests.size ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText(
                'agentFlow',
                'trajectory.no_internal_calls'
              )}
            />
          ) : null}
          {[...requests.entries()].map(([key, steps], index) => {
            const first = steps[0];
            return (
              <button
                type="button"
                key={key}
                className="agent-requests__request"
                aria-pressed={
                  selected?.node.trace_node_id === node.trace_node_id &&
                  selected.invocation === key
                }
                onClick={() => onSelect({ node, invocation: key })}
              >
                <span className="agent-requests__number">
                  {i18nText('agentFlow', 'trajectory.agent_request', {
                    count: index + 1
                  })}
                </span>
                <span className="agent-requests__summary">
                  <span>
                    {purposeLabel(first.metadata.purpose)} ·{' '}
                    {first.metadata.node_id}
                  </span>
                  <small title={first.metadata.invocation_id}>
                    {first.metadata.invocation_id} ·{' '}
                    {i18nText('agentFlow', 'trajectory.attempt', {
                      count: first.metadata.provider_attempt_index
                    })}
                  </small>
                </span>
                <time>{formatDateTime(first.created_at)}</time>
              </button>
            );
          })}
          {pages.hasNextPage && !pages.isError ? (
            <div className="provider-trajectory__more">
              <Spin size="small" />{' '}
              {i18nText('agentFlow', 'trajectory.loading_pages')}
            </div>
          ) : null}
          {pages.data?.pages[0] ? (
            <div className="agent-requests__integrity">
              {i18nText('agentFlow', 'trajectory.semantic_integrity')}:{' '}
              {integrityLabel(pages.data.pages[0].integrity)}
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}

export function AgentRequestInspector({
  scope,
  selection,
  onClose,
  onClient
}: {
  scope: Scope;
  selection: AgentRequestSelection;
  onClose: () => void;
  onClient?: (link: ProviderTrajectoryStep['links'][number]) => void;
}) {
  const { pages, requests } = useAgentRequests(scope, selection.node, false);
  const steps = requests.get(selection.invocation) ?? [];
  const [eventId, setEventId] = useState<string>();
  const [showNode, setShowNode] = useState(false);
  const step = steps.find((item) => item.event_id === eventId) ?? steps[0];
  return (
    <aside
      className="provider-trajectory__inspector workflow-activity__inspector"
      aria-label={i18nText('agentFlow', 'trajectory.inspector')}
    >
      <div className="provider-trajectory__inspector-header">
        <strong>{i18nText('agentFlow', 'trajectory.invocation')}</strong>
        <Button
          type="text"
          size="small"
          icon={<CloseOutlined />}
          aria-label={i18nText('agentFlow', 'auto.close', {
            value1: i18nText('agentFlow', 'trajectory.inspector')
          })}
          onClick={onClose}
        />
      </div>
      <div className="agent-requests__actions">
        <Button
          type="link"
          size="small"
          icon={<ApartmentOutlined />}
          aria-expanded={showNode}
          onClick={() => setShowNode(!showNode)}
        >
          {i18nText('agentFlow', 'trajectory.workflow_node')}
        </Button>
      </div>
      {showNode ? (
        <div className="workflow-activity__body">
          <TraceActivityDetailContext.Provider value={true}>
            <Suspense fallback={<Spin />}>
              <NodeDetail
                node={selection.node}
                runId={scope.runId}
                traceLoader={scope.loader}
                initiallyExpanded
                defaultToolsExpanded={false}
                onLoadArtifact={scope.loader.loadArtifact}
                onLoadArtifacts={scope.loader.loadArtifacts}
              />
            </Suspense>
          </TraceActivityDetailContext.Provider>
        </div>
      ) : (
        <>
          {pages.isError ? (
            <Alert
              type="error"
              title={i18nText('agentFlow', 'auto.loading_failed')}
            />
          ) : null}
          <Tabs
            className="agent-requests__events"
            size="small"
            activeKey={step?.event_id}
            onChange={setEventId}
            items={steps.map((item) => ({
              key: item.event_id,
              label: `${stepLabel(item)} · #${item.event_sequence}`
            }))}
          />
          {step ? (
            <TrajectoryStepDetail
              key={step.event_id}
              step={step}
              loader={scope.loader}
              onClient={onClient}
            />
          ) : (
            <Spin />
          )}
        </>
      )}
    </aside>
  );
}
