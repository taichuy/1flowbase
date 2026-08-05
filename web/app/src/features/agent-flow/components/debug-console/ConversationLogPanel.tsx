import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState
} from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Descriptions,
  Empty,
  Spin,
  Tabs,
  Tooltip,
  Typography
} from 'antd';
import { DownloadOutlined } from '@ant-design/icons';

import type { AgentFlowDebugMessage } from '../../api/runtime';
import { AgentFlowDockPanel } from '../editor/AgentFlowDockPanel';
import { NodeRunPayloadSections } from '../detail/last-run/NodeRunPayloadSections';
import type { RuntimeDebugArtifactBatchLoader } from '../detail/last-run/runtime-debug-payload';
import { DebugWorkflowNodeItem } from './conversation/DebugWorkflowNodeRow';
import { DebugWorkflowNodeDetailContent } from './conversation/LlmToolTraceTree';
import {
  groupTraceItemsForDisplay,
  nodeDisplayName
} from './conversation/debug-workflow-trace-utils';
import {
  appendTraceChildrenPage,
  createInitialLazyTraceChildrenState,
  lazyTraceChildrenReducer
} from './lazy-trace-children-state';
import './conversation-log-panel.css';
import { formatDateTime, formatNumber } from '../../../../shared/i18n/format';
import { i18nText } from '../../../../shared/i18n/text';
import type {
  ConversationLogOverviewLoader,
  ConversationLogRunOverview,
  ConversationLogTraceLoader,
  ConversationLogTraceNodeChildrenPageInfo,
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
export type {
  ConversationLogOverviewLoader,
  ConversationLogTraceLoader
} from './conversation-log-trace-model';

const CONVERSATION_LOG_QUERY_STALE_TIME_MS = 60_000;

function buildDetailInput(message: AgentFlowDebugMessage) {
  const firstTraceItem = message.traceSummary[0];

  if (firstTraceItem && Object.keys(firstTraceItem.inputPayload).length > 0) {
    return firstTraceItem.inputPayload;
  }

  if (firstTraceItem && Object.keys(firstTraceItem.outputPayload).length > 0) {
    return firstTraceItem.outputPayload;
  }

  return {};
}

function buildDetailOutput(message: AgentFlowDebugMessage) {
  if (message.rawOutput) {
    return message.rawOutput;
  }

  const lastTraceItem = message.traceSummary.at(-1);

  if (lastTraceItem && Object.keys(lastTraceItem.outputPayload).length > 0) {
    return lastTraceItem.outputPayload;
  }

  return {
    answer: message.content
  };
}

function formatTimestamp(value: string | null | undefined) {
  if (!value) {
    return '—';
  }

  return formatDateTime(value, { hour12: false });
}

function messageCompatibilityModeLabel(message: AgentFlowDebugMessage) {
  return message.compatibilityModeLabel ?? message.compatibilityMode ?? '—';
}

function overviewCompatibilityModeLabel(
  message: AgentFlowDebugMessage,
  overview: ConversationLogRunOverview | undefined
) {
  return (
    overview?.run.compatibility_mode ?? messageCompatibilityModeLabel(message)
  );
}

function formatNullableNumber(value: number | null | undefined) {
  return typeof value === 'number' && Number.isFinite(value)
    ? formatNumber(value)
    : '-';
}

function overviewDetailInput(
  message: AgentFlowDebugMessage,
  overview: ConversationLogRunOverview | undefined
) {
  return overview?.flow_run.input_payload ?? buildDetailInput(message);
}

function overviewDetailOutput(
  message: AgentFlowDebugMessage,
  overview: ConversationLogRunOverview | undefined
) {
  if (!overview) {
    return buildDetailOutput(message);
  }

  if (Object.keys(overview.flow_run.output_payload).length > 0) {
    return overview.flow_run.output_payload;
  }

  const answerPayload = overview.answer_snapshot?.output_payload;
  if (answerPayload && Object.keys(answerPayload).length > 0) {
    return answerPayload;
  }

  return {
    answer: overview.answer_snapshot?.text ?? message.content
  };
}

function ConversationLogDetailContent({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  overview
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overview?: ConversationLogRunOverview;
}) {
  const firstTraceItem = message.traceSummary[0] ?? null;
  const lastTraceItem = message.traceSummary.at(-1) ?? null;
  const startedAt =
    overview?.flow_run.started_at ??
    overview?.run.started_at ??
    firstTraceItem?.startedAt;
  const finishedAt =
    overview?.flow_run.finished_at ??
    overview?.run.finished_at ??
    lastTraceItem?.finishedAt;

  return (
    <div className="agent-flow-editor__conversation-log-tab">
      <div className="agent-flow-editor__conversation-log-json-list">
        <NodeRunPayloadSections
          debugPayload={{}}
          includeDebugPayload={false}
          inputPayload={overviewDetailInput(message, overview)}
          outputPayload={overviewDetailOutput(message, overview)}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      </div>
      <section
        aria-label={i18nText('agentFlow', 'auto.metadata')}
        className="agent-flow-editor__conversation-log-metadata"
      >
        <Typography.Text strong>
          {i18nText('agentFlow', 'auto.metadata')}
        </Typography.Text>
        <Descriptions
          column={1}
          items={[
            {
              key: 'runId',
              label: i18nText('agentFlow', 'auto.run_id'),
              children: overview?.flow_run.id ?? message.runId ?? '—'
            },
            {
              key: 'status',
              label: i18nText('agentFlow', 'auto.status'),
              children: overview?.flow_run.status ?? message.status
            },
            {
              key: 'compatibilityMode',
              label: i18nText('agentFlow', 'auto.agreement'),
              children: overviewCompatibilityModeLabel(message, overview)
            },
            {
              key: 'totalTokens',
              label: i18nText('agentFlow', 'auto.total_tokens'),
              children: formatNullableNumber(
                overview?.statistics?.total_tokens ??
                  message.statistics?.total_tokens
              )
            },
            {
              key: 'uniqueNodeCount',
              label: i18nText('agentFlow', 'auto.real_number_nodes'),
              children: formatNullableNumber(
                overview?.statistics?.unique_node_count ??
                  message.statistics?.unique_node_count
              )
            },
            {
              key: 'toolCallbackCount',
              label: i18nText('agentFlow', 'auto.number_tool_callbacks'),
              children: formatNullableNumber(
                overview?.statistics?.tool_callback_count ??
                  message.statistics?.tool_callback_count
              )
            },
            {
              key: 'startedAt',
              label: i18nText('agentFlow', 'auto.start_time'),
              children: formatTimestamp(startedAt)
            },
            {
              key: 'finishedAt',
              label: i18nText('agentFlow', 'auto.end_time'),
              children: formatTimestamp(finishedAt)
            }
          ]}
          size="small"
        />
      </section>
    </div>
  );
}

function ConversationLogLazyDetail({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  overviewLoader,
  overviewRunId
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overviewLoader: ConversationLogOverviewLoader;
  overviewRunId: string;
}) {
  const overviewQuery = useQuery({
    queryKey: ['conversation-log-run-overview', overviewRunId],
    queryFn: () => overviewLoader.loadOverview(overviewRunId),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });

  if (overviewQuery.isLoading) {
    return (
      <div className="agent-flow-editor__conversation-log-empty">
        <Spin />
      </div>
    );
  }

  return (
    <ConversationLogDetailContent
      message={message}
      overview={overviewQuery.data}
      onLoadArtifact={onLoadArtifact}
      onLoadArtifacts={onLoadArtifacts}
    />
  );
}

function ConversationLogDetail({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  overviewLoader
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overviewLoader?: ConversationLogOverviewLoader;
}) {
  const overviewRunId = message.detailRunId ?? message.runId;

  if (overviewLoader && overviewRunId) {
    return (
      <ConversationLogLazyDetail
        message={message}
        overviewLoader={overviewLoader}
        overviewRunId={overviewRunId}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
      />
    );
  }

  return (
    <ConversationLogDetailContent
      message={message}
      onLoadArtifact={onLoadArtifact}
      onLoadArtifacts={onLoadArtifacts}
    />
  );
}

function ConversationTrace({
  defaultToolsExpanded,
  message,
  onLoadArtifact,
  onLoadArtifacts,
  traceLoader
}: {
  defaultToolsExpanded: boolean;
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  traceLoader?: ConversationLogTraceLoader;
}) {
  const traceRunId = message.detailRunId ?? message.runId;

  if (traceLoader && traceRunId) {
    return (
      <LazyConversationTrace
        key={`${message.id}:${traceRunId}`}
        defaultToolsExpanded={defaultToolsExpanded}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
        runId={traceRunId}
        traceLoader={traceLoader}
      />
    );
  }

  return (
    <ConversationTraceContent
      key={message.id}
      message={message}
      onLoadArtifact={onLoadArtifact}
      onLoadArtifacts={onLoadArtifacts}
    />
  );
}

const initialLazyTraceChildrenState = createInitialLazyTraceChildrenState<
  ConversationLogTraceNodeSummary,
  ConversationLogTraceNodeChildrenPageInfo,
  ConversationLogTraceProjectionStatus
>();

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

function TraceProjectionStatusNotice({
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

function LazyConversationTrace({
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
  const traceTreeQuery = useQuery({
    queryKey: ['conversation-log-trace-tree', runId],
    queryFn: () => traceLoader.loadTree(runId),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });

  if (traceTreeQuery.isLoading) {
    return (
      <div className="agent-flow-editor__conversation-log-empty">
        <Spin />
      </div>
    );
  }

  const projectionStatus = traceTreeQuery.data?.projection_status;
  const nodes = traceTreeQuery.data?.nodes ?? [];

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
      <LazyTraceNodeList
        defaultToolsExpanded={defaultToolsExpanded}
        nodes={nodes}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
        runId={runId}
        traceLoader={traceLoader}
      />
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
      traceLoader.loadChildren(runId, node.trace_node_id, undefined),
    refetchOnWindowFocus: false,
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

function LazyTraceNodeItem({
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
  const isGroupNode = isTraceGroupNode(node);
  const [expanded, setExpanded] = useState(false);
  const [childrenState, dispatchChildrenState] = useReducer(
    lazyTraceChildrenReducer,
    initialLazyTraceChildrenState
  );
  const fallbackItem = useMemo(() => mapTraceSummaryToTraceItem(node), [node]);
  const contentQuery = useQuery({
    enabled: expanded && node.has_content && !isGroupNode,
    queryKey: [
      'conversation-log-trace-node-content',
      runId,
      node.trace_node_id
    ],
    queryFn: () => traceLoader.loadContent(runId, node.trace_node_id),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const nodeRunDetailRefId = useMemo(
    () => findNodeRunDetailRefId(contentQuery.data),
    [contentQuery.data]
  );
  const nodeRunDetailQuery = useQuery({
    enabled:
      expanded &&
      Boolean(nodeRunDetailRefId) &&
      Boolean(traceLoader.loadDetail),
    queryKey: [
      'conversation-log-trace-node-detail',
      runId,
      node.trace_node_id,
      nodeRunDetailRefId
    ],
    queryFn: () => {
      if (!traceLoader.loadDetail || !nodeRunDetailRefId) {
        throw new Error('trace_node_detail_loader_unavailable');
      }

      return traceLoader.loadDetail(
        runId,
        node.trace_node_id,
        nodeRunDetailRefId
      );
    },
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const childrenQuery = useQuery({
    enabled: expanded && node.has_children,
    queryKey: [
      'conversation-log-trace-node-children',
      runId,
      node.trace_node_id
    ],
    queryFn: () =>
      traceLoader.loadChildren(runId, node.trace_node_id, undefined),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  const childNodes = useMemo(
    () =>
      appendTraceChildrenPage(
        childrenQuery.data?.items ?? [],
        childrenState.additionalNodes
      ),
    [childrenState.additionalNodes, childrenQuery.data?.items]
  );
  const childPageInfo =
    childrenState.pageInfo ?? childrenQuery.data?.page_info ?? null;
  const childProjectionStatus =
    childrenState.projectionStatus ?? childrenQuery.data?.projection_status;
  const loadMoreTraceChildren = useCallback(async () => {
    const cursor = childPageInfo?.next_cursor;
    if (!cursor || childrenState.loadingMore) {
      return;
    }

    dispatchChildrenState({ type: 'load_more_started' });
    try {
      const nextPage = await traceLoader.loadChildren(
        runId,
        node.trace_node_id,
        cursor
      );
      dispatchChildrenState({ type: 'load_more_succeeded', page: nextPage });
    } catch {
      dispatchChildrenState({ type: 'load_more_failed' });
    }
  }, [
    childPageInfo?.next_cursor,
    childrenState.loadingMore,
    node.trace_node_id,
    runId,
    traceLoader
  ]);
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
        mapTraceContentToTraceItem(
          fallbackItem,
          contentQuery.data,
          nodeRunDetailQuery.data
        ),
        toolModeFromTraceNodes(childNodes)
      ),
    [childNodes, contentQuery.data, fallbackItem, nodeRunDetailQuery.data]
  );
  const contentProjectionStatus = contentQuery.data?.projection_status;
  const contentLoading = contentQuery.isLoading || nodeRunDetailQuery.isLoading;
  const loadToolCallbackDetail = traceLoader.loadToolCallbackDetail;
  const childNodesBeforePayload =
    visibleChildNodes.length > 0 || toolModeNodes.length > 0 ? (
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
      {childProjectionStatus &&
      !traceProjectionStatusSucceeded(childProjectionStatus) ? (
        <TraceProjectionStatusNotice status={childProjectionStatus} />
      ) : null}
      {childrenState.loadMoreFailed ? (
        <Alert
          title={i18nText('agentFlow', 'auto.loading_failed')}
          showIcon
          type="error"
        />
      ) : null}
    </>
  );
  const loadMoreChildrenButton = childPageInfo?.has_more ? (
    <Button
      className="agent-flow-editor__conversation-log-load-more"
      loading={childrenState.loadingMore}
      size="small"
      type="link"
      onClick={() => {
        void loadMoreTraceChildren();
      }}
    >
      {i18nText('agentFlow', 'auto.load_more_trace_children')}
    </Button>
  ) : null;

  return (
    <DebugWorkflowNodeItem
      expanded={expanded}
      item={item}
      onToggle={() => setExpanded((current) => !current)}
    >
      {isGroupNode ? (
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
          ) : contentProjectionStatus &&
            !traceProjectionStatusSucceeded(contentProjectionStatus) ? (
            <TraceProjectionStatusNotice status={contentProjectionStatus} />
          ) : (
            <div className="agent-flow-editor__conversation-log-json-list">
              <DebugWorkflowNodeDetailContent
                beforePayloadContent={childNodesBeforePayload}
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
      )}
    </DebugWorkflowNodeItem>
  );
}

function ConversationTraceContent({
  message,
  onLoadArtifact,
  onLoadArtifacts
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [expandedNodeKey, setExpandedNodeKey] = useState<string | null>(null);
  const traceGroups = useMemo(
    () => groupTraceItemsForDisplay(message.traceSummary),
    [message.traceSummary]
  );

  if (message.traceSummary.length === 0) {
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
      <div
        aria-label={i18nText('agentFlow', 'auto.tracking_nodes')}
        className="agent-flow-editor__conversation-log-node-list"
      >
        {traceGroups.map((group) => {
          const item = group.item;
          const nodeExpanded = group.key === expandedNodeKey;

          return (
            <DebugWorkflowNodeItem
              key={group.key}
              expanded={nodeExpanded}
              item={item}
              onToggle={() =>
                setExpandedNodeKey((current) =>
                  current === group.key ? null : group.key
                )
              }
            >
              <section
                aria-label={i18nText('agentFlow', 'auto.node_details_alt', {
                  value1: nodeDisplayName(item)
                })}
                className="agent-flow-editor__conversation-log-node-detail"
              >
                <div className="agent-flow-editor__conversation-log-json-list">
                  <DebugWorkflowNodeDetailContent
                    item={item}
                    onLoadArtifact={onLoadArtifact}
                    onLoadArtifacts={onLoadArtifacts}
                  />
                </div>
              </section>
            </DebugWorkflowNodeItem>
          );
        })}
      </div>
    </div>
  );
}

function useConversationLogArtifactLoader(
  messageId: string,
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>
) {
  const loadArtifactRef = useRef(onLoadArtifact);
  const artifactRequestsRef = useRef<Map<string, Promise<unknown>> | null>(
    null
  );
  const hasArtifactLoader = Boolean(onLoadArtifact);

  useEffect(() => {
    loadArtifactRef.current = onLoadArtifact;
  }, [onLoadArtifact]);

  useEffect(() => {
    artifactRequestsRef.current?.clear();
  }, [messageId]);

  const loadCachedArtifact = useCallback(async (artifactRef: string) => {
    if (artifactRequestsRef.current === null) {
      artifactRequestsRef.current = new Map();
    }

    const artifactRequests = artifactRequestsRef.current;
    const existingRequest = artifactRequests.get(artifactRef);

    if (existingRequest) {
      return existingRequest;
    }

    const loadArtifact = loadArtifactRef.current;
    if (!loadArtifact) {
      throw new Error('missing_conversation_log_artifact_loader');
    }

    const request = loadArtifact(artifactRef).catch((error: unknown) => {
      if (artifactRequests.get(artifactRef) === request) {
        artifactRequests.delete(artifactRef);
      }
      throw error;
    });

    artifactRequests.set(artifactRef, request);
    return request;
  }, []);

  return hasArtifactLoader ? loadCachedArtifact : undefined;
}

export function ConversationLogPanel({
  defaultTraceToolsExpanded = false,
  exportingRun = false,
  message,
  onClose,
  onExportRun,
  onLoadArtifact,
  onLoadArtifacts,
  overviewLoader,
  traceLoader
}: {
  defaultTraceToolsExpanded?: boolean;
  exportingRun?: boolean;
  message: AgentFlowDebugMessage;
  onClose: () => void;
  onExportRun?: (runId: string) => void | Promise<void>;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overviewLoader?: ConversationLogOverviewLoader;
  traceLoader?: ConversationLogTraceLoader;
}) {
  const loadArtifact = useConversationLogArtifactLoader(
    message.id,
    onLoadArtifact
  );
  const [activeTabKey, setActiveTabKey] = useState('detail');
  const exportRunId = message.detailRunId ?? message.runId ?? null;
  const exportActionLabel = i18nText(
    'agentFlow',
    'auto.export_current_run_json'
  );
  const exportAction =
    onExportRun && exportRunId ? (
      <Tooltip title={exportActionLabel}>
        <Button
          aria-label={exportActionLabel}
          icon={<DownloadOutlined aria-hidden="true" />}
          loading={exportingRun}
          size="small"
          type="text"
          onClick={() => {
            void onExportRun(exportRunId);
          }}
        />
      </Tooltip>
    ) : null;

  return (
    <AgentFlowDockPanel
      actions={exportAction}
      bodyClassName="agent-flow-editor__conversation-log-body"
      className="agent-flow-editor__conversation-log-panel"
      closeLabel={i18nText('agentFlow', 'auto.turn_off_conversation_log')}
      title={i18nText('agentFlow', 'auto.conversation_log')}
      onClose={onClose}
    >
      <Tabs
        activeKey={activeTabKey}
        className="agent-flow-editor__conversation-log-tabs"
        items={[
          {
            key: 'detail',
            label: i18nText('agentFlow', 'auto.details'),
            children: (
              <ConversationLogDetail
                message={message}
                overviewLoader={overviewLoader}
                onLoadArtifact={loadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
            )
          },
          {
            key: 'trace',
            label: i18nText('agentFlow', 'auto.track'),
            children:
              activeTabKey === 'trace' ? (
                <ConversationTrace
                  defaultToolsExpanded={defaultTraceToolsExpanded}
                  message={message}
                  onLoadArtifact={loadArtifact}
                  onLoadArtifacts={onLoadArtifacts}
                  traceLoader={traceLoader}
                />
              ) : null
          }
        ]}
        onChange={setActiveTabKey}
      />
    </AgentFlowDockPanel>
  );
}
