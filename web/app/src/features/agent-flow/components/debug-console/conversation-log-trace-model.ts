import type { AgentFlowTraceItem } from '../../api/runtime';

export interface ConversationLogTraceNodeSummary {
  trace_node_id: string;
  stable_locator?: string;
  parent_trace_node_id?: string | null;
  node_kind: string;
  flow_run_id?: string;
  node_run_id?: string | null;
  callback_task_id?: string | null;
  node_id?: string | null;
  node_type?: string | null;
  node_mode?: string | null;
  node_alias: string;
  status: string;
  started_at: string;
  finished_at?: string | null;
  duration_ms?: number | null;
  metrics_payload?: Record<string, unknown>;
  has_children: boolean;
  child_count?: number;
  has_content: boolean;
  source_flow_run_id?: string | null;
  source_trace_node_id?: string | null;
  parent_callback_task_id?: string | null;
  parent_tool_call_id?: string | null;
  trace_relation_kind?: string | null;
}

export interface ConversationLogTraceProjectionStatus {
  projection_status:
    | 'pending'
    | 'running'
    | 'succeeded'
    | 'failed'
    | 'stale'
    | 'partial';
  projection_version: number;
  source_watermark: string;
  attempt_count: number;
  last_attempt_at?: string | null;
  last_success_at?: string | null;
  last_error_code?: string | null;
  last_error_stage?: string | null;
  last_error_source_kind?: string | null;
  last_error_source_locator?: string | null;
  last_error_ref?: string | null;
  retriable: boolean;
}

export interface ConversationLogTraceTree {
  projection_status?: ConversationLogTraceProjectionStatus;
  nodes: ConversationLogTraceNodeSummary[];
}

export interface ConversationLogTraceNodeChildrenPageInfo {
  has_more: boolean;
  next_cursor?: string | null;
  page_size: number;
}

export interface ConversationLogTraceNodeChildren {
  projection_status?: ConversationLogTraceProjectionStatus;
  items: ConversationLogTraceNodeSummary[];
  page_info: ConversationLogTraceNodeChildrenPageInfo;
}

export interface ConversationLogTraceNodeContent {
  trace_node_id: string;
  node_kind: string;
  projection_status?: ConversationLogTraceProjectionStatus;
  content_kind?: string;
  source_refs?: unknown;
  detail_refs?: unknown;
  payload: Record<string, unknown>;
}

export interface ConversationLogTraceNodeDetail {
  trace_node_id: string;
  node_kind: string;
  projection_status?: ConversationLogTraceProjectionStatus;
  detail_ref_id: string;
  detail_kind: string;
  source_refs?: unknown;
  payload: Record<string, unknown>;
}

export interface ConversationLogRunOverview {
  run: {
    id: string;
    status?: string;
    compatibility_mode?: string | null;
    started_at?: string | null;
    finished_at?: string | null;
  };
  statistics?: {
    total_tokens?: number | null;
    unique_node_count?: number | null;
    tool_callback_count?: number | null;
  };
  flow_run: {
    id: string;
    status: string;
    input_payload: Record<string, unknown>;
    output_payload: Record<string, unknown>;
    error_payload?: Record<string, unknown> | null;
    started_at: string;
    finished_at?: string | null;
  };
  answer_snapshot?: {
    text: string;
    output_payload: Record<string, unknown>;
  } | null;
}

export interface ConversationLogTraceLoader {
  loadTree: (runId: string) => Promise<ConversationLogTraceTree>;
  loadChildren: (
    runId: string,
    traceNodeId: string,
    cursor?: string
  ) => Promise<ConversationLogTraceNodeChildren>;
  loadContent: (
    runId: string,
    traceNodeId: string
  ) => Promise<ConversationLogTraceNodeContent>;
  loadDetail?: (
    runId: string,
    traceNodeId: string,
    detailRefId: string
  ) => Promise<ConversationLogTraceNodeDetail>;
  loadToolCallbackDetail?: (
    runId: string,
    traceNodeId: string,
    toolCallId: string
  ) => Promise<unknown>;
}

export interface ConversationLogOverviewLoader {
  loadOverview: (runId: string) => Promise<ConversationLogRunOverview>;
}

export function mapTraceSummaryToTraceItem(
  summary: ConversationLogTraceNodeSummary
): AgentFlowTraceItem {
  const debugPayload =
    summary.node_type === 'tool' && summary.node_mode
      ? { tool_mode: summary.node_mode }
      : {};

  return {
    nodeId: summary.node_id ?? summary.trace_node_id,
    nodeRunId: summary.node_run_id ?? summary.trace_node_id,
    nodeAlias: summary.node_alias,
    nodeType: summary.node_type ?? summary.node_kind,
    status: summary.status,
    startedAt: summary.started_at,
    finishedAt: summary.finished_at ?? null,
    durationMs: summary.duration_ms ?? null,
    inputPayload: {},
    outputPayload: {},
    errorPayload: null,
    metricsPayload: summary.metrics_payload ?? {},
    debugPayload
  };
}

export function isToolModeTraceNode(node: ConversationLogTraceNodeSummary) {
  return node.node_kind === 'fusion' || node.node_kind === 'route';
}

export function isTraceGroupNode(node: ConversationLogTraceNodeSummary) {
  return (
    node.node_kind === 'tool_group' ||
    node.node_kind === 'agent_group' ||
    node.node_kind === 'stitched_context' ||
    node.node_kind === 'stitched_run' ||
    node.node_type === 'tools' ||
    node.node_type === 'agents' ||
    node.node_type === 'stitched_context'
  );
}

export function toolModeFromTraceNodes(
  nodes: ConversationLogTraceNodeSummary[]
) {
  const toolModeNode = nodes.find(isToolModeTraceNode);

  return toolModeNode?.node_kind ?? null;
}

export function traceItemWithToolMode(
  item: AgentFlowTraceItem,
  toolMode: string | null
): AgentFlowTraceItem {
  if (item.nodeType !== 'tool' || !toolMode) {
    return item;
  }

  return {
    ...item,
    debugPayload: {
      ...(item.debugPayload ?? {}),
      tool_mode: toolMode
    }
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function recordArray(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function firstStringField(
  record: Record<string, unknown>,
  keys: string[]
): string | null {
  for (const key of keys) {
    const value = record[key];

    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }
  }

  return null;
}

function payloadRecordField(
  payload: Record<string, unknown>,
  key: string
): Record<string, unknown> {
  const value = payload[key];

  return isRecord(value) ? value : {};
}

function firstPayloadRecordField(
  payload: Record<string, unknown>,
  keys: string[]
): Record<string, unknown> {
  for (const key of keys) {
    const value = payloadRecordField(payload, key);

    if (Object.keys(value).length > 0) {
      return value;
    }
  }

  return {};
}

function payloadValueHasValue(value: unknown): boolean {
  if (value === null || value === undefined) {
    return false;
  }

  if (Array.isArray(value)) {
    return value.length > 0;
  }

  if (typeof value === 'object') {
    return Object.keys(value).length > 0;
  }

  return true;
}

function pickPayloadFields(
  payload: Record<string, unknown>,
  keys: string[]
): Record<string, unknown> {
  const picked: Record<string, unknown> = {};

  for (const key of keys) {
    const value = payload[key];

    if (payloadValueHasValue(value)) {
      picked[key] = value;
    }
  }

  return picked;
}

function omitPayloadFields(
  payload: Record<string, unknown>,
  keys: string[]
): Record<string, unknown> {
  const omitted = new Set(keys);
  const next: Record<string, unknown> = {};

  for (const [key, value] of Object.entries(payload)) {
    if (!omitted.has(key) && payloadValueHasValue(value)) {
      next[key] = value;
    }
  }

  return next;
}

function mapToolCallbackPayloadToTraceItem(
  fallback: AgentFlowTraceItem,
  payload: Record<string, unknown>
): AgentFlowTraceItem {
  return {
    ...fallback,
    inputPayload: firstPayloadRecordField(payload, [
      'request_payload',
      'tool_call'
    ]),
    outputPayload: firstPayloadRecordField(payload, [
      'parsed_result',
      'tool_result',
      'callback_payload'
    ]),
    debugPayload: pickPayloadFields(payload, [
      'callback_task_id',
      'tool_call_id',
      'callback_status',
      'execution_status',
      'duration_ms',
      'route_trace'
    ])
  };
}

function mapPayloadContentToTraceItem(
  fallback: AgentFlowTraceItem,
  content: ConversationLogTraceNodeContent
): AgentFlowTraceItem {
  const payload = content.payload;

  if (!isRecord(payload)) {
    return fallback;
  }

  if (content.node_kind === 'tool_callback') {
    return mapToolCallbackPayloadToTraceItem(fallback, payload);
  }

  if (content.node_kind === 'fusion' || content.node_kind === 'route') {
    const metricsPayload = payloadRecordField(payload, 'metrics_payload');
    const usage = payloadRecordField(payload, 'usage');
    const effectiveMetrics =
      Object.keys(metricsPayload).length > 0 ? metricsPayload : usage;

    return {
      ...fallback,
      inputPayload: {},
      outputPayload:
        Object.keys(effectiveMetrics).length > 0
          ? { usage: effectiveMetrics }
          : {},
      debugPayload: payload,
      metricsPayload: effectiveMetrics
    };
  }

  if (content.node_kind === 'branch') {
    const inputPayload = payloadRecordField(payload, 'input_payload');
    const outputPayload = payloadRecordField(payload, 'output_payload');
    const debugPayload = payloadRecordField(payload, 'debug_payload');
    const metricsPayload = payloadRecordField(payload, 'metrics_payload');

    return {
      ...fallback,
      inputPayload: Object.keys(inputPayload).length > 0 ? inputPayload : {},
      outputPayload: Object.keys(outputPayload).length > 0 ? outputPayload : {},
      debugPayload: Object.keys(debugPayload).length > 0 ? debugPayload : {},
      metricsPayload:
        Object.keys(metricsPayload).length > 0
          ? metricsPayload
          : fallback.metricsPayload
    };
  }

  const inputPayload = payloadRecordField(payload, 'input_payload');
  const outputPayload = payloadRecordField(payload, 'output_payload');
  const debugPayload = payloadRecordField(payload, 'debug_payload');
  const metricsPayload = payloadRecordField(payload, 'metrics_payload');
  const errorPayload = payloadRecordField(payload, 'error_payload');
  const hasStructuredPayload =
    Object.keys(inputPayload).length > 0 ||
    Object.keys(outputPayload).length > 0 ||
    Object.keys(debugPayload).length > 0 ||
    Object.keys(metricsPayload).length > 0 ||
    Object.keys(errorPayload).length > 0;

  if (!hasStructuredPayload) {
    return {
      ...fallback,
      inputPayload: {},
      outputPayload: {},
      debugPayload: payload
    };
  }

  return {
    ...fallback,
    inputPayload,
    outputPayload,
    errorPayload: Object.keys(errorPayload).length > 0 ? errorPayload : null,
    metricsPayload:
      Object.keys(metricsPayload).length > 0
        ? metricsPayload
        : fallback.metricsPayload,
    debugPayload:
      Object.keys(debugPayload).length > 0
        ? debugPayload
        : omitPayloadFields(payload, [
            'input_payload',
            'output_payload',
            'error_payload',
            'metrics_payload',
            'debug_payload'
          ])
  };
}

export function findNodeRunDetailRefId(
  content: ConversationLogTraceNodeContent | undefined
) {
  if (!content || content.node_kind !== 'node_run') {
    return null;
  }

  for (const detailRef of recordArray(content.detail_refs)) {
    if (firstStringField(detailRef, ['detail_kind']) !== 'node_run') {
      continue;
    }

    return firstStringField(detailRef, ['detail_ref_id']);
  }

  return null;
}

function mapNodeRunRecordToTraceItem(
  fallback: AgentFlowTraceItem,
  nodeRun: Record<string, unknown>
): AgentFlowTraceItem {
  const inputPayload = payloadRecordField(nodeRun, 'input_payload');
  const outputPayload = payloadRecordField(nodeRun, 'output_payload');
  const debugPayload = payloadRecordField(nodeRun, 'debug_payload');
  const fallbackDebugPayload = isRecord(fallback.debugPayload)
    ? fallback.debugPayload
    : {};
  const metricsPayload = payloadRecordField(nodeRun, 'metrics_payload');
  const errorPayload = payloadRecordField(nodeRun, 'error_payload');

  return {
    ...fallback,
    inputPayload,
    outputPayload,
    errorPayload: Object.keys(errorPayload).length > 0 ? errorPayload : null,
    metricsPayload:
      Object.keys(metricsPayload).length > 0
        ? metricsPayload
        : fallback.metricsPayload,
    debugPayload: {
      ...fallbackDebugPayload,
      ...debugPayload
    }
  };
}

function mapDetailContentToTraceItem(
  fallback: AgentFlowTraceItem,
  detail: ConversationLogTraceNodeDetail | undefined
): AgentFlowTraceItem {
  if (!detail || detail.detail_kind !== 'node_run') {
    return fallback;
  }

  const nodeRun = payloadRecordField(detail.payload, 'node_run');

  if (Object.keys(nodeRun).length === 0) {
    return fallback;
  }

  return mapNodeRunRecordToTraceItem(fallback, nodeRun);
}

export function mapTraceContentToTraceItem(
  fallback: AgentFlowTraceItem,
  content: ConversationLogTraceNodeContent | undefined,
  detail?: ConversationLogTraceNodeDetail
): AgentFlowTraceItem {
  const contentItem = content
    ? mapPayloadContentToTraceItem(fallback, content)
    : fallback;

  return mapDetailContentToTraceItem(contentItem, detail);
}

export function traceProjectionStatusSucceeded(
  status: ConversationLogTraceProjectionStatus | undefined
) {
  return !status || status.projection_status === 'succeeded';
}
