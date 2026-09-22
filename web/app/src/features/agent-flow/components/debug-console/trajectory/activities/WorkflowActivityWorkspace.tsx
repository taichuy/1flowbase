import { lazy, Suspense, useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin } from 'antd';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceNodeSummary,
  ConversationLogTraceProjectionStatus
} from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { formatDateTime } from '../../../../../../shared/i18n/format';
import { useProgressiveTrajectory } from '../use-progressive-trajectory';
import {
  activityCategory,
  TraceActivityDetailContext,
  type ActivityCategory
} from './activity-model';
import './workflow-activity.css';

const NodeDetail = lazy(() =>
  import('../../ConversationLogPanel').then((module) => ({
    default: module.LazyTraceNodeItem
  }))
);

const ProjectionNotice = lazy(() =>
  import('../../ConversationLogPanel').then((module) => ({
    default: module.TraceProjectionStatusNotice
  }))
);
type ActivityPages = {
  data?: {
    pages: { projection_status?: ConversationLogTraceProjectionStatus }[];
  };
};
function unfinishedProjection(pages: ActivityPages) {
  return pages.data?.pages
    .map((page) => page.projection_status)
    .find((status) => status && status.projection_status !== 'succeeded');
}

type Scope = {
  runId: string;
  loader: ConversationLogTraceLoader;
  active: boolean;
};
type Selection = { node: ConversationLogTraceNodeSummary; path: string[] };

function useChildren(scope: Scope, parent: string, enabled: boolean) {
  const pages = useInfiniteQuery({
    queryKey: ['trajectory-activity-children', scope.runId, parent],
    enabled: scope.active && enabled,
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      scope.loader.loadChildren(scope.runId, parent, pageParam),
    getNextPageParam: (page, all, cursor) => {
      const next = page.page_info.next_cursor;
      return page.page_info.has_more &&
        next &&
        next !== cursor &&
        !all.slice(0, -1).some((p) => p.page_info.next_cursor === next)
        ? next
        : undefined;
    },
    refetchInterval: (query) =>
      scope.active &&
      ['pending', 'running'].includes(
        query.state.data?.pages.at(-1)?.projection_status?.projection_status ??
          ''
      )
        ? 1000
        : false,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(scope.active && enabled, pages);
  return pages;
}
function PageStatus({
  pages
}: {
  pages: {
    isError: boolean;
    isFetching: boolean;
    refetch: () => Promise<unknown>;
  } & ActivityPages;
}) {
  const projection = unfinishedProjection(pages);
  if (projection)
    return (
      <>
        <Suspense fallback={<Spin size="small" />}>
          <ProjectionNotice status={projection} />
        </Suspense>
        {projection.retriable ? (
          <Button onClick={() => void pages.refetch()}>
            {i18nText('agentFlow', 'auto.retry')}
          </Button>
        ) : null}
      </>
    );
  return pages.isError ? (
    <Alert
      type="error"
      title={i18nText('agentFlow', 'auto.loading_failed')}
      action={
        <Button onClick={() => void pages.refetch()}>
          {i18nText('agentFlow', 'auto.retry')}
        </Button>
      }
    />
  ) : pages.isFetching ? (
    <Spin size="small" />
  ) : null;
}
function ActivityRow({
  scope,
  node,
  path,
  onSelect,
  selected
}: {
  scope: Scope;
  node: ConversationLogTraceNodeSummary;
  path: string[];
  onSelect: (selection: Selection) => void;
  selected?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const pages = useChildren(
    scope,
    node.trace_node_id,
    expanded && node.has_children
  );
  return (
    <section>
      <div
        className="workflow-activity__row"
        data-selected={selected === node.trace_node_id || undefined}
      >
        {node.has_children ? (
          <Button
            type="text"
            size="small"
            aria-label={node.node_alias}
            aria-expanded={expanded}
            icon={expanded ? <DownOutlined /> : <RightOutlined />}
            onClick={() => setExpanded(!expanded)}
          />
        ) : (
          <span className="workflow-activity__leaf" />
        )}
        <button
          className="workflow-activity__select"
          onClick={() => {
            onSelect({ node, path });
            if (node.has_children) setExpanded(true);
          }}
        >
          <span>{node.node_alias}</span>
          <small>{node.node_type ?? node.node_kind}</small>
          <small>{node.status}</small>
        </button>
      </div>
      {expanded ? (
        <div className="workflow-activity__children">
          {(pages.data?.pages.flatMap((page) => page.items) ?? []).map(
            (child) => (
              <ActivityRow
                key={child.trace_node_id}
                scope={scope}
                node={child}
                path={[...path, node.node_alias]}
                selected={selected}
                onSelect={onSelect}
              />
            )
          )}
          <PageStatus pages={pages} />
        </div>
      ) : null}
    </section>
  );
}
function NodeActivities({
  scope,
  node,
  category,
  onSelect,
  selected
}: {
  scope: Scope;
  node: ConversationLogTraceNodeSummary;
  category: ActivityCategory;
  onSelect: (selection: Selection) => void;
  selected?: string;
}) {
  const direct = activityCategory(node);
  const pages = useChildren(
    scope,
    node.trace_node_id,
    !direct && node.has_children && node.node_type === 'llm'
  );
  const children =
    pages.data?.pages
      .flatMap((page) => page.items)
      .filter((child) => activityCategory(child) === category) ?? [];
  if (direct)
    return direct === category ? (
      <ActivityRow
        scope={scope}
        node={node}
        path={[]}
        selected={selected}
        onSelect={onSelect}
      />
    ) : null;
  if (node.node_type !== 'llm') return null;
  return (
    <section>
      {children.length ? (
        <div className="provider-trajectory__group">{node.node_alias}</div>
      ) : null}
      {children.map((child) => (
        <ActivityRow
          key={child.trace_node_id}
          scope={scope}
          node={child}
          path={[node.node_alias]}
          selected={selected}
          onSelect={onSelect}
        />
      ))}
      <PageStatus pages={pages} />
      {!pages.isFetching &&
      !pages.isError &&
      !unfinishedProjection(pages) &&
      !children.length ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'trajectory.no_activities')}
        />
      ) : null}
    </section>
  );
}
export function WorkflowActivityWorkspace({
  runId,
  nodeRunId,
  loader,
  active,
  category
}: Scope & { nodeRunId?: string; category: ActivityCategory }) {
  const scope = { runId, loader, active };
  const [selection, setSelection] = useState<Selection | null>(null);
  const roots = useInfiniteQuery({
    queryKey: ['trajectory-activity-roots', runId],
    enabled: active,
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam }) => {
      if (pageParam) return loader.loadChildren(runId, 'root', pageParam);
      const tree = await loader.loadTree(runId);
      return {
        items: tree.nodes,
        page_info: tree.page_info ?? {
          has_more: false,
          page_size: tree.nodes.length
        },
        projection_status: tree.projection_status
      };
    },
    getNextPageParam: (page, all, cursor) => {
      const next = page.page_info.next_cursor;
      return page.page_info.has_more &&
        next &&
        next !== cursor &&
        !all.slice(0, -1).some((p) => p.page_info.next_cursor === next)
        ? next
        : undefined;
    },
    refetchInterval: (query) =>
      active &&
      ['pending', 'running'].includes(
        query.state.data?.pages.at(-1)?.projection_status?.projection_status ??
          ''
      )
        ? 1000
        : false,
    refetchOnWindowFocus: false
  });
  useProgressiveTrajectory(active, roots);
  const nodes =
    roots.data?.pages
      .flatMap((page) => page.items)
      .filter(
        (node) =>
          (node.node_type === 'llm' || activityCategory(node) !== null) &&
          (!nodeRunId || node.node_run_id === nodeRunId)
      ) ?? [];
  return (
    <div className="provider-trajectory__split workflow-activity">
      <div
        className="provider-trajectory__ledger"
        aria-label={i18nText('agentFlow', 'trajectory.activities')}
      >
        <PageStatus pages={roots} />
        {!roots.isLoading &&
        !roots.isError &&
        !unfinishedProjection(roots) &&
        !nodes.length ? (
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
        ) : null}
        {nodes.map((node) => (
          <NodeActivities
            key={node.trace_node_id}
            scope={scope}
            node={node}
            category={category}
            selected={selection?.node.trace_node_id}
            onSelect={setSelection}
          />
        ))}
      </div>
      {selection ? (
        <aside className="provider-trajectory__inspector workflow-activity__inspector">
          <div className="provider-trajectory__inspector-header">
            <span>{selection.node.node_alias}</span>
            <Button
              size="small"
              type="text"
              icon={<CloseOutlined />}
              aria-label={i18nText('agentFlow', 'auto.close', {
                value1: selection.node.node_alias
              })}
              onClick={() => setSelection(null)}
            />
          </div>
          <div className="workflow-activity__body">
            <p>{[...selection.path, selection.node.node_alias].join(' › ')}</p>
            <p>
              {formatDateTime(selection.node.started_at)} ·{' '}
              {selection.node.status}
            </p>
            <p>
              {i18nText('agentFlow', 'trajectory.workflow_node')} ·{' '}
              {selection.node.node_run_id ?? selection.node.trace_node_id}
            </p>
            {selection.node.source_flow_run_id ? (
              <p>
                {i18nText('agentFlow', 'auto.run_id')} ·{' '}
                {selection.node.source_flow_run_id}
              </p>
            ) : null}
            <TraceActivityDetailContext.Provider value={true}>
              <Suspense fallback={<Spin />}>
                <NodeDetail
                  key={selection.node.trace_node_id}
                  defaultToolsExpanded={false}
                  initiallyExpanded
                  node={selection.node}
                  runId={runId}
                  traceLoader={loader}
                  onLoadArtifact={loader.loadArtifact}
                  onLoadArtifacts={loader.loadArtifacts}
                />
              </Suspense>
            </TraceActivityDetailContext.Provider>
          </div>
        </aside>
      ) : null}
    </div>
  );
}
