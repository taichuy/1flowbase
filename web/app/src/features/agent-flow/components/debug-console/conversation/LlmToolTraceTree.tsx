import { DownOutlined, RightOutlined, ToolOutlined } from '@ant-design/icons';
import { Tag, Tooltip, Typography } from 'antd';
import {
  ReactNode,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState
} from 'react';

import type { AgentFlowTraceItem } from '../../../api/runtime';
import { NodeRunPayloadSections } from '../../detail/last-run/NodeRunPayloadSections';
import {
  RuntimeDebugPayloadBlock,
  type RuntimeDebugArtifactBatchLoader
} from '../../detail/last-run/runtime-debug-payload';
import { AnswerSnapshotTrace } from './AnswerSnapshotTrace';
import { DebugWorkflowNodeItem } from './DebugWorkflowNodeRow';
import {
  collectLlmToolCallbacksFromDebugPayloads,
  readLlmToolRouteTraceDetail,
  readLlmToolCallbackDetail,
  type LlmToolCallback,
  type LlmToolRouteBranchTrace,
  type LlmToolRouteBranchSummary,
  type LlmToolRouteTraceSummary,
  stripLlmRoundsFromDebugPayload
} from './llm-tool-callbacks';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatTokens, formatDurationScaled } from './metrics-formatter';

function callbackStatusLabel(status: LlmToolCallback['callbackStatus']) {
  switch (status) {
    case 'returned':
      return i18nText('agentFlow', 'auto.returned');
    case 'cancelled':
      return i18nText('agentFlow', 'auto.canceled');
    default:
      return i18nText('agentFlow', 'auto.wait_for_callback');
  }
}

function callbackStatusColor(status: LlmToolCallback['callbackStatus']) {
  switch (status) {
    case 'returned':
      return 'success';
    case 'cancelled':
      return 'default';
    default:
      return 'warning';
  }
}

function executionStatusLabel(status: LlmToolCallback['executionStatus']) {
  switch (status) {
    case 'succeeded':
      return i18nText('agentFlow', 'auto.executed_successfully');
    case 'intercepted':
      return i18nText('agentFlow', 'auto.execution_intercepted');
    case 'failed':
      return i18nText('agentFlow', 'auto.execution_failed');
    case 'timed_out':
      return i18nText('agentFlow', 'auto.execution_timeout');
    case 'cancelled':
      return i18nText('agentFlow', 'auto.execution_cancel');
    default:
      return i18nText('agentFlow', 'auto.execution_unknown');
  }
}

function executionStatusColor(status: LlmToolCallback['executionStatus']) {
  switch (status) {
    case 'succeeded':
      return 'success';
    case 'intercepted':
      return 'warning';
    case 'failed':
    case 'timed_out':
      return 'error';
    case 'cancelled':
      return 'default';
    default:
      return 'default';
  }
}

function callUsageTotalTokens(callback: LlmToolCallback): number | null {
  const totalTokens = callback.call_usage?.total_tokens;

  return typeof totalTokens === 'number' && Number.isFinite(totalTokens)
    ? totalTokens
    : null;
}

function LlmToolInlineMetrics({ callback }: { callback: LlmToolCallback }) {
  const elements: ReactNode[] = [];
  const totalTokens = callUsageTotalTokens(callback);

  if (typeof totalTokens === 'number') {
    const formattedTokens = `${formatTokens(totalTokens)} tokens`;
    elements.push(
      <Tooltip title={`${totalTokens.toLocaleString()} tokens`} key="tokens">
        <span>{formattedTokens}</span>
      </Tooltip>
    );
  }

  if (typeof callback.duration_ms === 'number') {
    const formattedDuration = formatDurationScaled(callback.duration_ms);
    elements.push(
      <Tooltip
        title={`${callback.duration_ms.toLocaleString()} ms`}
        key="duration"
      >
        <span>{formattedDuration}</span>
      </Tooltip>
    );
  }

  if (elements.length === 0) {
    return null;
  }

  const joined: ReactNode[] = [];
  elements.forEach((el, index) => {
    joined.push(el);
    if (index < elements.length - 1) {
      joined.push(<span key={`dot-${index}`}> · </span>);
    }
  });

  return (
    <Typography.Text
      className="agent-flow-editor__debug-llm-tool-inline-metrics"
      type="secondary"
    >
      {joined}
    </Typography.Text>
  );
}

function routeTraceLabel(routeTrace: LlmToolRouteTraceSummary) {
  return routeTrace.routeKind === 'fusion'
    ? i18nText('agentFlow', 'auto.tool_mode_fusion')
    : i18nText('agentFlow', 'auto.tool_mode_agent');
}

function branchNodeStatus(branch: LlmToolRouteBranchSummary) {
  switch (branch.status) {
    case 'succeeded':
    case 'returned_to_main':
    case 'route_completed':
      return 'succeeded';
    case 'failed':
      return 'failed';
    case 'cancelled':
    case 'canceled':
      return 'cancelled';
    case 'waiting_callback':
      return 'waiting_callback';
    default:
      return branch.status ?? 'succeeded';
  }
}

function buildRouteBranchTraceItem(
  callback: LlmToolCallback,
  branch: LlmToolRouteBranchSummary | LlmToolRouteBranchTrace,
  index: number
): AgentFlowTraceItem {
  const fallbackId = `${callback.id}:branch:${index + 1}`;
  const loadedBranchTrace = isRouteBranchTrace(branch);
  const inputPayload = loadedBranchTrace ? branch.inputPayload : {};
  const outputPayload = loadedBranchTrace ? branch.outputPayload : {};
  const metricsPayload = loadedBranchTrace ? branch.metricsPayload : {};
  const debugPayload = loadedBranchTrace ? branch.debugPayload : {};

  return {
    nodeId: branch.nodeId ?? fallbackId,
    nodeRunId: `${fallbackId}:summary`,
    nodeAlias: branch.nodeAlias ?? branch.nodeId ?? `Panel ${index + 1}`,
    nodeType: branch.nodeType ?? 'llm',
    status: branchNodeStatus(branch),
    startedAt: '',
    finishedAt: callback.callbackStatus === 'returned' ? '' : null,
    durationMs: null,
    inputPayload,
    outputPayload,
    errorPayload: null,
    metricsPayload,
    debugPayload
  };
}

function isRouteBranchTrace(
  branch: LlmToolRouteBranchSummary | LlmToolRouteBranchTrace
): branch is LlmToolRouteBranchTrace {
  return Object.prototype.hasOwnProperty.call(branch, 'inputPayload');
}

function routeBranchNodeKey(
  callback: LlmToolCallback,
  branch: LlmToolRouteBranchSummary | LlmToolRouteBranchTrace
) {
  const debugPayloadRef = isRouteBranchTrace(branch)
    ? branch.debugPayloadRef
    : null;

  return (
    branch.nodeId ??
    debugPayloadRef ??
    branch.nodeAlias ??
    `${callback.id}:${JSON.stringify(branch.rawPayload)}`
  );
}

export function DebugWorkflowNodeDetailContent({
  item,
  beforePayloadContent,
  defaultToolsExpanded = false,
  payloadDisclosure = 'immediate',
  onLoadArtifact,
  onLoadArtifacts,
  onLoadToolCallbackDetail
}: {
  item: AgentFlowTraceItem;
  beforePayloadContent?: ReactNode;
  defaultToolsExpanded?: boolean;
  payloadDisclosure?: 'immediate' | 'after-finished';
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onLoadToolCallbackDetail?: (detailRef: string) => Promise<unknown>;
}) {
  const debugPayload = stripLlmRoundsFromDebugPayload(item.debugPayload ?? {});

  return (
    <>
      <LlmToolTraceTree
        debugPayload={item.debugPayload}
        defaultToolsExpanded={defaultToolsExpanded}
        payloadDisclosure={payloadDisclosure}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
        onLoadToolCallbackDetail={onLoadToolCallbackDetail}
      />
      {item.answerSnapshot ? (
        <AnswerSnapshotTrace
          snapshot={item.answerSnapshot}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
      {beforePayloadContent}
      {payloadDisclosure === 'immediate' || item.finishedAt !== null ? (
        <NodeRunPayloadSections
          defaultCollapsed
          debugPayload={debugPayload}
          inputPayload={item.inputPayload}
          outputPayload={item.outputPayload}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
    </>
  );
}

function LlmToolRouteBranchNode({
  branch,
  callback,
  defaultToolsExpanded,
  payloadDisclosure,
  index,
  onLoadArtifact,
  onLoadArtifacts
}: {
  branch: LlmToolRouteBranchSummary | LlmToolRouteBranchTrace;
  callback: LlmToolCallback;
  defaultToolsExpanded: boolean;
  payloadDisclosure: 'immediate' | 'after-finished';
  index: number;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [expanded, setExpanded] = useState(false);
  const branchTraceItem = buildRouteBranchTraceItem(callback, branch, index);

  return (
    <div
      className="agent-flow-editor__debug-llm-route-branch-node"
      data-testid="debug-llm-route-branch-node"
    >
      <DebugWorkflowNodeItem
        expanded={expanded}
        item={branchTraceItem}
        onToggle={() => setExpanded((current) => !current)}
      >
        <div className="agent-flow-editor__debug-workflow-node-detail agent-flow-editor__debug-llm-route-branch-detail">
          <DebugWorkflowNodeDetailContent
            defaultToolsExpanded={defaultToolsExpanded}
            item={branchTraceItem}
            payloadDisclosure={payloadDisclosure}
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
          />
        </div>
      </DebugWorkflowNodeItem>
    </div>
  );
}

function LlmToolRouteNode({
  callback,
  defaultToolsExpanded,
  payloadDisclosure,
  onLoadArtifact,
  onLoadArtifacts
}: {
  callback: LlmToolCallback;
  defaultToolsExpanded: boolean;
  payloadDisclosure: 'immediate' | 'after-finished';
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [routeTraceLoadState, dispatchRouteTraceLoad] = useReducer(
    routeTraceLoadReducer,
    INITIAL_ROUTE_TRACE_LOAD_STATE
  );
  const loadArtifactRef = useRef(onLoadArtifact);
  const sourceRouteTrace = callback.routeTrace;
  const routeKind = sourceRouteTrace?.routeKind ?? null;
  const detailArtifactRef = sourceRouteTrace?.detailArtifactRef ?? null;
  const hasArtifactLoader = Boolean(onLoadArtifact);

  useEffect(() => {
    loadArtifactRef.current = onLoadArtifact;
  }, [onLoadArtifact]);

  useEffect(() => {
    if (
      routeKind !== 'fusion' ||
      !detailArtifactRef ||
      !hasArtifactLoader ||
      routeTraceLoadState.loadedRouteTrace ||
      routeTraceLoadState.loadFailed
    ) {
      return;
    }

    let active = true;
    const loadArtifact = loadArtifactRef.current;

    if (!loadArtifact) {
      return;
    }

    dispatchRouteTraceLoad({ type: 'start' });
    void loadArtifact(detailArtifactRef)
      .then((payload) => {
        if (!active) {
          return;
        }
        const detail = readLlmToolRouteTraceDetail(payload);
        if (!detail) {
          throw new Error('invalid_route_trace_detail');
        }
        dispatchRouteTraceLoad({ type: 'success', routeTrace: detail });
      })
      .catch(() => {
        if (active) {
          dispatchRouteTraceLoad({ type: 'failed' });
        }
      });

    return () => {
      active = false;
    };
  }, [
    detailArtifactRef,
    hasArtifactLoader,
    routeKind,
    routeTraceLoadState.loadFailed,
    routeTraceLoadState.loadedRouteTrace
  ]);

  if (!sourceRouteTrace) {
    return null;
  }

  const routeTrace = routeTraceLoadState.loadedRouteTrace ?? sourceRouteTrace;
  const branchTraces = routeTrace.branchTraces;
  const branchNodes =
    branchTraces.length > 0 ? branchTraces : routeTrace.branchSummaries;
  const routeTraceTitle = routeTraceLabel(routeTrace);

  const routeContent = (
    <>
      {routeTraceLoadState.loading ? (
        <Tag color="processing">{i18nText('agentFlow', 'auto.loading')}</Tag>
      ) : null}
      {routeTraceLoadState.loadFailed ? (
        <Tag color="error">{i18nText('agentFlow', 'auto.loading_failed')}</Tag>
      ) : null}
      {branchNodes.length > 0 ? (
        <div className="agent-flow-editor__debug-llm-route-branch-list">
          {branchNodes.map((branch, index) => (
            <LlmToolRouteBranchNode
              key={routeBranchNodeKey(callback, branch)}
              branch={branch}
              callback={callback}
              defaultToolsExpanded={defaultToolsExpanded}
              payloadDisclosure={payloadDisclosure}
              index={index}
              onLoadArtifact={onLoadArtifact}
              onLoadArtifacts={onLoadArtifacts}
            />
          ))}
        </div>
      ) : null}
      {routeTrace.routeKind === 'fusion' ? null : (
        <RuntimeDebugPayloadBlock
          height="11rem"
          payload={sourceRouteTrace.rawPayload}
          title={routeTraceTitle}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      )}
    </>
  );

  if (!defaultToolsExpanded) {
    return routeContent;
  }

  return (
    <div
      className="agent-flow-editor__debug-llm-route-node"
      data-testid="debug-llm-route-node"
    >
      {routeContent}
    </div>
  );
}

function LlmToolCallbackItem({
  callback,
  defaultToolsExpanded,
  payloadDisclosure,
  expanded,
  loadFailed,
  loading,
  onLoadArtifact,
  onLoadArtifacts,
  onToggle
}: {
  callback: LlmToolCallback;
  defaultToolsExpanded: boolean;
  payloadDisclosure: 'immediate' | 'after-finished';
  expanded: boolean;
  loadFailed: boolean;
  loading: boolean;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onToggle: () => void;
}) {
  return (
    <article
      className="agent-flow-editor__debug-llm-tool-item"
      data-expanded={expanded ? 'true' : 'false'}
    >
      <button
        aria-expanded={expanded}
        className="agent-flow-editor__debug-llm-tool-trigger"
        onClick={onToggle}
        type="button"
      >
        <span className="agent-flow-editor__debug-llm-tool-main">
          <Typography.Text strong>{callback.name}</Typography.Text>
          {callback.routeTrace ? (
            <Tag className="agent-flow-editor__debug-llm-tool-route-tag">
              {routeTraceLabel(callback.routeTrace)}
            </Tag>
          ) : (
            <LlmToolInlineMetrics callback={callback} />
          )}
        </span>
        <Tag color={callbackStatusColor(callback.callbackStatus)}>
          {callbackStatusLabel(callback.callbackStatus)}
        </Tag>
        {callback.executionStatus === 'unknown' ? null : (
          <Tag color={executionStatusColor(callback.executionStatus)}>
            {executionStatusLabel(callback.executionStatus)}
          </Tag>
        )}
        {expanded ? (
          <DownOutlined className="agent-flow-editor__debug-workflow-collapse" />
        ) : (
          <RightOutlined className="agent-flow-editor__debug-workflow-collapse" />
        )}
      </button>
      {expanded ? (
        <div className="agent-flow-editor__debug-llm-tool-detail">
          {loading ? (
            <Tag color="processing">
              {i18nText('agentFlow', 'auto.loading')}
            </Tag>
          ) : null}
          {loadFailed ? (
            <Tag color="error">
              {i18nText('agentFlow', 'auto.loading_failed')}
            </Tag>
          ) : null}
          {!loading && !loadFailed ? (
            <>
              <LlmToolRouteNode
                key={routeTraceNodeKey(callback)}
                callback={callback}
                defaultToolsExpanded={defaultToolsExpanded}
                payloadDisclosure={payloadDisclosure}
                onLoadArtifact={onLoadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
              <RuntimeDebugPayloadBlock
                height="11rem"
                payload={callback.requestPayload}
                title={i18nText('agentFlow', 'auto.tool_call')}
                onLoadArtifact={onLoadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
              {callback.parsedResult ? (
                <RuntimeDebugPayloadBlock
                  height="11rem"
                  payload={callback.parsedResult}
                  title={i18nText('agentFlow', 'auto.parse_results')}
                  onLoadArtifact={onLoadArtifact}
                  onLoadArtifacts={onLoadArtifacts}
                />
              ) : null}
              {callback.callbackPayload ? (
                <RuntimeDebugPayloadBlock
                  height="11rem"
                  payload={callback.callbackPayload}
                  title={i18nText('agentFlow', 'auto.full_callback')}
                  onLoadArtifact={onLoadArtifact}
                  onLoadArtifacts={onLoadArtifacts}
                />
              ) : (
                <Typography.Text type="secondary">
                  {i18nText('agentFlow', 'auto.wait_callback_return')}
                </Typography.Text>
              )}
            </>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

interface RouteTraceLoadState {
  loadedRouteTrace: LlmToolRouteTraceSummary | null;
  loading: boolean;
  loadFailed: boolean;
}

const INITIAL_ROUTE_TRACE_LOAD_STATE: RouteTraceLoadState = {
  loadedRouteTrace: null,
  loading: false,
  loadFailed: false
};

type RouteTraceLoadAction =
  | { type: 'start' }
  | { type: 'success'; routeTrace: LlmToolRouteTraceSummary }
  | { type: 'failed' };

function routeTraceLoadReducer(
  state: RouteTraceLoadState,
  action: RouteTraceLoadAction
): RouteTraceLoadState {
  switch (action.type) {
    case 'start':
      return {
        ...state,
        loading: true,
        loadFailed: false
      };
    case 'success':
      return {
        loadedRouteTrace: action.routeTrace,
        loading: false,
        loadFailed: false
      };
    case 'failed':
      return {
        ...state,
        loading: false,
        loadFailed: true
      };
    default:
      return state;
  }
}

function routeTraceNodeKey(callback: LlmToolCallback) {
  const routeTrace = callback.routeTrace;

  return [
    callback.id,
    routeTrace?.routeKind ?? '',
    routeTrace?.detailArtifactRef ?? ''
  ].join(':');
}

interface LlmToolTraceTreeState {
  toolsExpanded: boolean;
  expandedToolKey: string | null;
  loadedToolCallbacks: Record<string, Omit<LlmToolCallback, 'key'>>;
  loadingToolKey: string | null;
  failedToolKeys: Set<string>;
}

const INITIAL_LLM_TOOL_TRACE_TREE_STATE: LlmToolTraceTreeState = {
  toolsExpanded: false,
  expandedToolKey: null,
  loadedToolCallbacks: {},
  loadingToolKey: null,
  failedToolKeys: new Set()
};

function createInitialLlmToolTraceTreeState(
  defaultToolsExpanded: boolean
): LlmToolTraceTreeState {
  return {
    ...INITIAL_LLM_TOOL_TRACE_TREE_STATE,
    toolsExpanded: defaultToolsExpanded
  };
}

type LlmToolTraceTreeAction =
  | { type: 'toggle-tools' }
  | { type: 'expand-tools' }
  | { type: 'set-expanded-tool'; toolKey: string | null }
  | { type: 'load-start'; toolKey: string }
  | {
      type: 'load-success';
      callbackId: string;
      loadedCallback: Omit<LlmToolCallback, 'key'>;
    }
  | { type: 'load-failed'; toolKey: string }
  | { type: 'load-finished'; toolKey: string };

function llmToolTraceTreeReducer(
  state: LlmToolTraceTreeState,
  action: LlmToolTraceTreeAction
): LlmToolTraceTreeState {
  switch (action.type) {
    case 'toggle-tools':
      return {
        ...state,
        toolsExpanded: !state.toolsExpanded
      };
    case 'expand-tools':
      return state.toolsExpanded
        ? state
        : {
            ...state,
            toolsExpanded: true
          };
    case 'set-expanded-tool':
      return {
        ...state,
        expandedToolKey: action.toolKey
      };
    case 'load-start': {
      const failedToolKeys = new Set(state.failedToolKeys);
      failedToolKeys.delete(action.toolKey);

      return {
        ...state,
        loadingToolKey: action.toolKey,
        failedToolKeys
      };
    }
    case 'load-success':
      return {
        ...state,
        loadedToolCallbacks: {
          ...state.loadedToolCallbacks,
          [action.callbackId]: action.loadedCallback
        }
      };
    case 'load-failed':
      return {
        ...state,
        failedToolKeys: new Set(state.failedToolKeys).add(action.toolKey)
      };
    case 'load-finished':
      return {
        ...state,
        loadingToolKey:
          state.loadingToolKey === action.toolKey ? null : state.loadingToolKey
      };
    default:
      return state;
  }
}

export function LlmToolTraceTree(props: {
  debugPayload: unknown;
  debugPayloads?: unknown[];
  defaultToolsExpanded?: boolean;
  payloadDisclosure?: 'immediate' | 'after-finished';
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onLoadToolCallbackDetail?: (detailRef: string) => Promise<unknown>;
}) {
  return <LlmToolTraceTreeContent {...props} />;
}

function LlmToolTraceTreeContent({
  defaultToolsExpanded = false,
  debugPayload,
  debugPayloads,
  payloadDisclosure = 'immediate',
  onLoadArtifact,
  onLoadArtifacts,
  onLoadToolCallbackDetail
}: {
  defaultToolsExpanded?: boolean;
  debugPayload: unknown;
  debugPayloads?: unknown[];
  payloadDisclosure?: 'immediate' | 'after-finished';
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onLoadToolCallbackDetail?: (detailRef: string) => Promise<unknown>;
}) {
  const [traceTreeState, dispatchTraceTree] = useReducer(
    llmToolTraceTreeReducer,
    defaultToolsExpanded,
    createInitialLlmToolTraceTreeState
  );
  const mountedRef = useRef(true);
  const debugPayloadList = useMemo(
    () => debugPayloads ?? [debugPayload],
    [debugPayload, debugPayloads]
  );
  const toolCallbacks = useMemo(
    () => collectLlmToolCallbacksFromDebugPayloads(debugPayloadList),
    [debugPayloadList]
  );
  const effectiveToolCallbacks = useMemo(
    () =>
      toolCallbacks.map((callback) => {
        const loadedCallback = traceTreeState.loadedToolCallbacks[callback.id];

        if (!loadedCallback) {
          return callback;
        }

        return {
          ...callback,
          ...loadedCallback,
          key: callback.key,
          call_usage: loadedCallback.call_usage ?? callback.call_usage,
          result_context_usage:
            loadedCallback.result_context_usage ??
            callback.result_context_usage,
          duration_ms: loadedCallback.duration_ms ?? callback.duration_ms,
          detailArtifactRef:
            callback.detailArtifactRef ?? loadedCallback.detailArtifactRef,
          detailRef: callback.detailRef ?? loadedCallback.detailRef,
          routeTrace: loadedCallback.routeTrace ?? callback.routeTrace
        };
      }),
    [traceTreeState.loadedToolCallbacks, toolCallbacks]
  );
  useEffect(() => {
    if (defaultToolsExpanded && effectiveToolCallbacks.length > 0) {
      dispatchTraceTree({ type: 'expand-tools' });
    }
  }, [defaultToolsExpanded, effectiveToolCallbacks]);
  useEffect(() => {
    mountedRef.current = true;

    return () => {
      mountedRef.current = false;
    };
  }, []);

  const loadToolCallbackDetail = (callback: LlmToolCallback) => {
    const loadDetail = callback.detailArtifactRef
      ? onLoadArtifact
        ? () => onLoadArtifact(callback.detailArtifactRef as string)
        : null
      : callback.detailRef && onLoadToolCallbackDetail
        ? () => onLoadToolCallbackDetail(callback.detailRef as string)
        : null;

    if (!loadDetail) {
      return;
    }
    if (
      traceTreeState.loadedToolCallbacks[callback.id] ||
      traceTreeState.loadingToolKey === callback.key
    ) {
      return;
    }

    dispatchTraceTree({ type: 'load-start', toolKey: callback.key });

    void loadDetail()
      .then((payload) => {
        if (!mountedRef.current) {
          return;
        }
        const loadedCallback = readLlmToolCallbackDetail(payload);

        if (!loadedCallback) {
          throw new Error('invalid_tool_callback_detail');
        }

        dispatchTraceTree({
          type: 'load-success',
          callbackId: callback.id,
          loadedCallback
        });
      })
      .catch(() => {
        if (!mountedRef.current) {
          return;
        }

        dispatchTraceTree({ type: 'load-failed', toolKey: callback.key });
      })
      .finally(() => {
        if (!mountedRef.current) {
          return;
        }

        dispatchTraceTree({ type: 'load-finished', toolKey: callback.key });
      });
  };

  if (effectiveToolCallbacks.length === 0) {
    return null;
  }

  const handleToggleTools = () => {
    dispatchTraceTree({ type: 'toggle-tools' });
  };

  const summaryText =
    effectiveToolCallbacks.length > 0
      ? i18nText('agentFlow', 'auto.tool_callbacks', {
          value1: effectiveToolCallbacks.length
        })
      : i18nText('agentFlow', 'auto.need_to_load');

  return (
    <section
      aria-label={i18nText('agentFlow', 'auto.llm_tools')}
      className="agent-flow-editor__debug-llm-tools"
    >
      <button
        aria-label={`${i18nText('agentFlow', 'auto.tools')} ${summaryText}`}
        aria-expanded={traceTreeState.toolsExpanded}
        className="agent-flow-editor__debug-llm-tools-trigger"
        onClick={handleToggleTools}
        type="button"
      >
        <span className="agent-flow-editor__debug-llm-tools-title">
          <ToolOutlined className="agent-flow-editor__debug-llm-tools-icon" />
          <Typography.Text strong>
            {i18nText('agentFlow', 'auto.tools')}
          </Typography.Text>
          <Typography.Text type="secondary">{summaryText}</Typography.Text>
        </span>
        {traceTreeState.toolsExpanded ? (
          <DownOutlined className="agent-flow-editor__debug-workflow-collapse" />
        ) : (
          <RightOutlined className="agent-flow-editor__debug-workflow-collapse" />
        )}
      </button>
      {traceTreeState.toolsExpanded ? (
        <div className="agent-flow-editor__debug-llm-tools-body">
          {effectiveToolCallbacks.length > 0 ? (
            <>
              <div
                aria-label={i18nText('agentFlow', 'auto.tool_callback_list')}
                className="agent-flow-editor__debug-llm-tool-list"
              >
                {effectiveToolCallbacks.map((callback) => {
                  const expanded =
                    traceTreeState.expandedToolKey === callback.key;

                  return (
                    <LlmToolCallbackItem
                      key={callback.key}
                      callback={callback}
                      defaultToolsExpanded={defaultToolsExpanded}
                      payloadDisclosure={payloadDisclosure}
                      expanded={expanded}
                      loadFailed={traceTreeState.failedToolKeys.has(
                        callback.key
                      )}
                      loading={traceTreeState.loadingToolKey === callback.key}
                      onLoadArtifact={onLoadArtifact}
                      onLoadArtifacts={onLoadArtifacts}
                      onToggle={() => {
                        const nextExpanded = !expanded;
                        dispatchTraceTree({
                          type: 'set-expanded-tool',
                          toolKey: nextExpanded ? callback.key : null
                        });

                        if (nextExpanded) {
                          loadToolCallbackDetail(callback);
                        }
                      }}
                    />
                  );
                })}
              </div>
            </>
          ) : (
            <Typography.Text type="secondary">
              {i18nText('agentFlow', 'auto.tool_callback_truncated')}
            </Typography.Text>
          )}
        </div>
      ) : null}
    </section>
  );
}
