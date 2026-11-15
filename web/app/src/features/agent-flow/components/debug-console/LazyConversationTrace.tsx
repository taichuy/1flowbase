import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState
} from 'react';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Typography } from 'antd';
import { ClientTrajectoryWorkspace } from './trajectory/client/ClientTrajectoryWorkspace';
import { TraceActivityDetailContext } from './trajectory/activities/activity-model';
import { ProviderTrajectory } from './trajectory/ProviderTrajectory';
import { DebugWorkflowNodeItem } from './conversation/DebugWorkflowNodeRow';
import { DebugWorkflowNodeDetailContent } from './conversation/LlmToolTraceTree';
import { nodeDisplayName } from './conversation/debug-workflow-trace-utils';
import { appendTraceChildrenPage } from './lazy-trace-children-state';
import type { RuntimeDebugArtifactBatchLoader } from '../detail/last-run/runtime-debug-payload';
import { i18nText } from '../../../../shared/i18n/text';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceTree,
  ConversationLogTraceNodeChildren,
  ConversationLogTraceNodeSummary,
  ConversationLogTraceProjectionStatus
} from './conversation-log-trace-model';
import {
  findNodeRunDetailRefId,
  isTraceGroupNode,
  isToolModeTraceNode,
  mapTraceContentToTraceItem,
  mapTraceSummaryToTraceItem,
  traceItemWithToolMode,
  traceProjectionStatusSucceeded,
  toolModeFromTraceNodes
} from './conversation-log-trace-model';
const CONVERSATION_LOG_QUERY_STALE_TIME_MS = 60_000;
const RunObservationContext = createContext(false);
function runIsActive(status?: string) {
  return (
    status !== undefined &&
    !['succeeded', 'failed', 'cancelled', 'incomplete'].includes(status)
  );
}
function TraceReadError({ retry }: { retry: () => unknown }) {
  return (
    <Alert
      type="error"
      showIcon
      title={i18nText('agentFlow', 'auto.loading_failed')}
      action={
        <Button size="small" onClick={() => void retry()}>
          {i18nText('agentFlow', 'auto.retry')}
        </Button>
      }
    />
  );
}

function traceProjectionStatusMessage(
  status: ConversationLogTraceProjectionStatus
) {
  switch (status.projection_status) {
    case 'pending':
      return i18nText('agentFlow', 'auto.trace_projection_pending');
    case 'running':
      return i18nText('agentFlow', 'auto.trace_projection_running');
    case 'failed':
      return i18nText('agentFlow', 'auto.trace_projection_failed');
    case 'stale':
      return i18nText('agentFlow', 'auto.trace_projection_stale');
    case 'partial':
      return i18nText('agentFlow', 'auto.trace_projection_partial');
    case 'succeeded':
      return i18nText('agentFlow', 'auto.trace_projection_succeeded');
  }
}

export function TraceProjectionStatusNotice({
  status
}: {
  status: ConversationLogTraceProjectionStatus;
}) {
  return (
    <Alert
      className="agent-flow-editor__conversation-log-projection-status"
      title={traceProjectionStatusMessage(status)}
      showIcon
      type={status.projection_status === 'failed' ? 'error' : 'info'}
    />
  );
}

export function LazyConversationTrace({
  defaultToolsExpanded,
  onLoadArtifact,
  onLoadArtifacts,
  runId,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  runId: string;
  traceLoader: ConversationLogTraceLoader;
}) {
  const traceTreeQuery = useInfiniteQuery({
    queryKey: [
      'conversation-log-trace-tree',
      traceLoader.sourceKind ?? 'native',
      runId,
      'pages'
    ],
    initialPageParam: undefined as string | undefined,
    queryFn: async ({
      pageParam
    }): Promise<ConversationLogTraceTree | ConversationLogTraceNodeChildren> =>
      pageParam
        ? traceLoader.loadChildren!(runId, 'root', pageParam)
        : traceLoader.loadTree!(runId),
    getNextPageParam: (page) =>
      page.page_info?.has_more
        ? (page.page_info.next_cursor ?? undefined)
        : undefined,
    refetchOnWindowFocus: false,
    refetchOnMount: 'always',
    refetchInterval: (query) => {
      const first = query.state.data?.pages[0];
      const status = first?.projection_status?.projection_status;
      return (first &&
        'flow_run' in first &&
        runIsActive(first.flow_run?.status)) ||
        status === 'pending' ||
        status === 'running'
        ? 1500
        : false;
    },
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const pages = traceTreeQuery.data?.pages ?? [];
  const firstPage = pages[0];
  const flowRun =
    firstPage && 'flow_run' in firstPage ? firstPage.flow_run : undefined;
  const rootPageInfo = pages.at(-1)?.page_info;

  if (traceTreeQuery.isError && !traceTreeQuery.data)
    return <TraceReadError retry={traceTreeQuery.refetch} />;

  if (traceTreeQuery.isLoading) {
    return (
      <div className="agent-flow-editor__conversation-log-empty">
        <Spin />
      </div>
    );
  }

  const projectionStatus = firstPage?.projection_status;
  const nodes = pages.reduce<ConversationLogTraceNodeSummary[]>(
    (items, page) =>
      appendTraceChildrenPage(items, 'nodes' in page ? page.nodes : page.items),
    []
  );

  if (!traceProjectionStatusSucceeded(projectionStatus) && projectionStatus) {
    return (
      <div className="agent-flow-editor__conversation-log-trace">
        <TraceProjectionStatusNotice status={projectionStatus} />
      </div>
    );
  }

  if (nodes.length === 0) {
    return (
      <div className="agent-flow-editor__conversation-log-empty">
        <Empty
          description={i18nText('agentFlow', 'auto.tracking_record_yet')}
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        />
      </div>
    );
  }

  return (
    <div className="agent-flow-editor__conversation-log-trace">
      <RunObservationContext.Provider value={runIsActive(flowRun?.status)}>
        <LazyTraceNodeList
          defaultToolsExpanded={defaultToolsExpanded}
          nodes={nodes}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          runId={runId}
          traceLoader={traceLoader}
        />
      </RunObservationContext.Provider>
      {traceTreeQuery.isError ? (
        <TraceReadError
          retry={
            traceTreeQuery.isFetchNextPageError
              ? traceTreeQuery.fetchNextPage
              : traceTreeQuery.refetch
          }
        />
      ) : null}
      {rootPageInfo?.has_more ? (
        <Button
          type="link"
          size="small"
          loading={traceTreeQuery.isFetchingNextPage}
          onClick={() => void traceTreeQuery.fetchNextPage()}
        >
          {i18nText('agentFlow', 'auto.load_more_trace_children')}
        </Button>
      ) : null}
    </div>
  );
}

function LazyTraceNodeList({
  defaultToolsExpanded,
  nodes,
  onLoadArtifact,
  onLoadArtifacts,
  runId,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  nodes: ConversationLogTraceNodeSummary[];
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  runId: string;
  traceLoader: ConversationLogTraceLoader;
}) {
  return (
    <div
      aria-label={i18nText('agentFlow', 'auto.tracking_nodes')}
      className="agent-flow-editor__conversation-log-node-list"
    >
      {nodes.map((node) => (
        <LazyTraceNodeItem
          key={`${runId}:${node.trace_node_id}`}
          defaultToolsExpanded={defaultToolsExpanded}
          node={node}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          runId={runId}
          traceLoader={traceLoader}
        />
      ))}
    </div>
  );
}

function FlattenedToolModeTraceNodeChildren({
  defaultToolsExpanded,
  nodes,
  onLoadArtifact,
  onLoadArtifacts,
  runId,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  nodes: ConversationLogTraceNodeSummary[];
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  runId: string;
  traceLoader: ConversationLogTraceLoader;
}) {
  if (nodes.length === 0) {
    return null;
  }

  return (
    <>
      {nodes.map((node) => (
        <FlattenedToolModeTraceNodeChild
          key={node.trace_node_id}
          defaultToolsExpanded={defaultToolsExpanded}
          node={node}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          runId={runId}
          traceLoader={traceLoader}
        />
      ))}
    </>
  );
}

function FlattenedToolModeTraceNodeChild({
  defaultToolsExpanded,
  node,
  onLoadArtifact,
  onLoadArtifacts,
  runId,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  node: ConversationLogTraceNodeSummary;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  runId: string;
  traceLoader: ConversationLogTraceLoader;
}) {
  const childrenQuery = useQuery({
    enabled: node.has_children,
    queryKey: [
      'conversation-log-trace-node-children',
      runId,
      node.trace_node_id
    ],
    queryFn: () =>
      traceLoader.loadChildren!(runId, node.trace_node_id, undefined),
    refetchOnWindowFocus: false,
    refetchInterval: !node.finished_at ? 1500 : false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const childProjectionStatus = childrenQuery.data?.projection_status;
  const childNodes = childrenQuery.data?.items ?? [];

  if (!node.has_children) {
    return null;
  }

  return (
    <>
      {childrenQuery.isLoading ? <Spin /> : null}
      {childrenQuery.isError ? (
        <Alert
          title={i18nText('agentFlow', 'auto.loading_failed')}
          showIcon
          type="error"
        />
      ) : null}
      {childProjectionStatus &&
      !traceProjectionStatusSucceeded(childProjectionStatus) ? (
        <TraceProjectionStatusNotice status={childProjectionStatus} />
      ) : null}
      {childNodes.length > 0 ? (
        <LazyTraceNodeList
          defaultToolsExpanded={defaultToolsExpanded}
          nodes={childNodes}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          runId={runId}
          traceLoader={traceLoader}
        />
      ) : null}
    </>
  );
}

export function LazyTraceNodeItem({
  detailOnly = false,
  initiallyExpanded = false,
  defaultToolsExpanded,
  node,
  onLoadArtifact,
  onLoadArtifacts,
  runId,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  initiallyExpanded?: boolean;
  detailOnly?: boolean;
  node: ConversationLogTraceNodeSummary;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  runId: string;
  traceLoader: ConversationLogTraceLoader;
}) {
  const activityDetail = useContext(TraceActivityDetailContext);
  const isGroupNode = isTraceGroupNode(node);
  const hasTrajectory = node.node_type === 'llm' && Boolean(node.node_run_id);
  const [expanded, setExpanded] = useState(initiallyExpanded);
  const runActive = useContext(RunObservationContext);
  const fallbackItem = useMemo(() => mapTraceSummaryToTraceItem(node), [node]);
  const contentQuery = useQuery({
    enabled:
      expanded &&
      node.has_content &&
      !isGroupNode &&
      !node.record_id &&
      Boolean(traceLoader.loadContent),
    queryKey: [
      'conversation-log-trace-node-content',
      runId,
      node.trace_node_id,
      node.status,
      node.finished_at
    ],
    queryFn: () => traceLoader.loadContent!(runId, node.trace_node_id),
    refetchOnWindowFocus: false,
    refetchInterval: !node.finished_at ? 1500 : false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const nodeRunDetailRefId = useMemo(
    () => findNodeRunDetailRefId(contentQuery.data),
    [contentQuery.data]
  );
  const loadNodeRunSection =
    nodeRunDetailRefId && traceLoader.loadDetail
      ? async (
          section: 'input_payload' | 'debug_payload' | 'output_payload'
        ) => {
          const result = await traceLoader.loadDetail!(
            runId,
            node.trace_node_id,
            nodeRunDetailRefId,
            section
          );
          const nodeRun = result.payload.node_run as
            | Record<string, unknown>
            | undefined;
          return nodeRun?.[section];
        }
      : undefined;
  const childrenQuery = useInfiniteQuery({
    enabled: expanded && node.has_children && !detailOnly,
    queryKey: [
      'conversation-log-trace-node-children',
      runId,
      node.trace_node_id,
      'pages'
    ],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      traceLoader.loadChildren!(runId, node.trace_node_id, pageParam),
    getNextPageParam: (page) =>
      page.page_info.has_more
        ? (page.page_info.next_cursor ?? undefined)
        : undefined,
    refetchOnWindowFocus: false,
    refetchInterval: runActive || !node.finished_at ? 1500 : false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const signature = `${node.status}:${node.finished_at}:${runActive}`;
  const previousSignature = useRef(signature);
  useEffect(() => {
    const changed = previousSignature.current !== signature;
    previousSignature.current = signature;
    if (changed && expanded && node.has_children && !detailOnly)
      void childrenQuery.refetch();
  }, [
    signature,
    expanded,
    node.has_children,
    detailOnly,
    childrenQuery.refetch
  ]);
  const childPages = childrenQuery.data?.pages ?? [];
  const childNodes = useMemo(
    () =>
      childPages.reduce<ConversationLogTraceNodeSummary[]>(
        (nodes, page) => appendTraceChildrenPage(nodes, page.items),
        []
      ),
    [childrenQuery.data]
  );
  const childPageInfo = childPages.at(-1)?.page_info;
  const childProjectionStatus = childPages[0]?.projection_status;
  const toolModeNodes = useMemo(
    () =>
      node.node_kind === 'tool_callback'
        ? childNodes.filter(isToolModeTraceNode)
        : [],
    [childNodes, node.node_kind]
  );
  const visibleChildNodes = useMemo(
    () =>
      node.node_kind === 'tool_callback'
        ? childNodes.filter((childNode) => !isToolModeTraceNode(childNode))
        : childNodes,
    [childNodes, node.node_kind]
  );
  const item = useMemo(
    () =>
      traceItemWithToolMode(
        mapTraceContentToTraceItem(fallbackItem, contentQuery.data),
        toolModeFromTraceNodes(childNodes)
      ),
    [childNodes, contentQuery.data, fallbackItem]
  );
  const contentProjectionStatus = contentQuery.data?.projection_status;
  const contentLoading = contentQuery.isLoading;
  const loadToolCallbackDetail = traceLoader.loadToolCallbackDetail;
  const childNodesBeforePayload =
    !detailOnly &&
    (visibleChildNodes.length > 0 || toolModeNodes.length > 0) ? (
      <>
        <FlattenedToolModeTraceNodeChildren
          defaultToolsExpanded={defaultToolsExpanded}
          nodes={toolModeNodes}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          runId={runId}
          traceLoader={traceLoader}
        />
        {visibleChildNodes.length > 0 ? (
          <LazyTraceNodeList
            defaultToolsExpanded={defaultToolsExpanded}
            nodes={visibleChildNodes}
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
            runId={runId}
            traceLoader={traceLoader}
          />
        ) : null}
      </>
    ) : null;
  const childLoadStatusContent = (
    <>
      {childrenQuery.isLoading ? <Spin /> : null}
      {childrenQuery.isError ? (
        <TraceReadError
          retry={
            childrenQuery.isFetchNextPageError
              ? childrenQuery.fetchNextPage
              : childrenQuery.refetch
          }
        />
      ) : null}
      {childProjectionStatus &&
      !traceProjectionStatusSucceeded(childProjectionStatus) ? (
        <TraceProjectionStatusNotice status={childProjectionStatus} />
      ) : null}
    </>
  );
  const loadMoreChildrenButton = childPageInfo?.has_more ? (
    <Button
      className="agent-flow-editor__conversation-log-load-more"
      loading={childrenQuery.isFetchingNextPage}
      size="small"
      type="link"
      onClick={() => {
        void childrenQuery.fetchNextPage();
      }}
    >
      {i18nText('agentFlow', 'auto.load_more_trace_children')}
    </Button>
  ) : null;

  const detail = node.record_id ? (
    <section className="agent-flow-editor__conversation-log-node-detail">
      {expanded && node.has_content ? (
        <ClientTrajectoryWorkspace
          runId={node.record_id}
          loader={traceLoader}
        />
      ) : null}
      {childLoadStatusContent}
      {childNodesBeforePayload}
      {loadMoreChildrenButton}
    </section>
  ) : isGroupNode ? (
    <div className="agent-flow-editor__conversation-log-node-group">
      {childLoadStatusContent}
      {childNodesBeforePayload}
      {loadMoreChildrenButton}
    </div>
  ) : (
    <section
      aria-label={i18nText('agentFlow', 'auto.node_details_alt', {
        value1: nodeDisplayName(fallbackItem)
      })}
      className="agent-flow-editor__conversation-log-node-detail"
    >
      {contentLoading ? (
        <Spin />
      ) : contentQuery.isError ? (
        <TraceReadError retry={contentQuery.refetch} />
      ) : contentProjectionStatus &&
        !traceProjectionStatusSucceeded(contentProjectionStatus) ? (
        <TraceProjectionStatusNotice status={contentProjectionStatus} />
      ) : (
        <div className="agent-flow-editor__conversation-log-json-list">
          <DebugWorkflowNodeDetailContent
            onLoadSection={loadNodeRunSection}
            beforePayloadContent={childNodesBeforePayload}
            processAction={
              !activityDetail && hasTrajectory && node.node_run_id ? (
                <ProviderTrajectory
                  runId={node.source_flow_run_id ?? node.flow_run_id ?? runId}
                  nodeRunId={node.node_run_id}
                  loader={traceLoader}
                />
              ) : undefined
            }
            defaultToolsExpanded={defaultToolsExpanded}
            item={item}
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
            onLoadToolCallbackDetail={
              loadToolCallbackDetail
                ? (toolCallId) =>
                    loadToolCallbackDetail(
                      runId,
                      node.trace_node_id,
                      toolCallId
                    )
                : undefined
            }
          />
        </div>
      )}
      {childLoadStatusContent}
      {loadMoreChildrenButton}
    </section>
  );
  if (detailOnly) return detail;
  return (
    <>
      {node.record_id && node.child_count ? (
        <Typography.Text type="secondary">
          {i18nText('applications', 'auto.subagents', {
            count: node.child_count
          })}
        </Typography.Text>
      ) : null}
      <DebugWorkflowNodeItem
        expanded={expanded}
        item={item}
        onToggle={() => setExpanded((current) => !current)}
      >
        {detail}
      </DebugWorkflowNodeItem>
    </>
  );
}
