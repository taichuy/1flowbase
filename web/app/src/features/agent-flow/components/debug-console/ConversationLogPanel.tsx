import type { ConsoleApplicationLogRecordOverview } from '@1flowbase/api-client';
import { ConversationTrajectoryWorkspace } from './trajectory/client/ConversationTrajectoryWorkspace';
import { ClientTrajectoryWorkspace } from './trajectory/client/ClientTrajectoryWorkspace';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
import DownloadOutlined from '@ant-design/icons/es/icons/DownloadOutlined';

import type { AgentFlowDebugMessage } from '../../api/runtime';
import { AgentFlowDockPanel } from '../editor/AgentFlowDockPanel';
import { NodeRunPayloadSections } from '../detail/last-run/NodeRunPayloadSections';
import type { RuntimeDebugArtifactBatchLoader } from '../detail/last-run/runtime-debug-payload';
import { DebugWorkflowNodeDetailContent } from './conversation/LlmToolTraceTree';
import { DebugWorkflowNodeItem } from './conversation/DebugWorkflowNodeRow';
import {
  groupTraceItemsForDisplay,
  nodeDisplayName
} from './conversation/debug-workflow-trace-utils';
import './conversation-log-panel.css';
import { formatDateTime, formatNumber } from '../../../../shared/i18n/format';
import { i18nText } from '../../../../shared/i18n/text';
import type {
  ConversationLogOverviewLoader,
  ConversationLogRunOverview,
  ConversationLogTraceLoader
} from './conversation-log-trace-model';
import { LazyConversationTrace } from './LazyConversationTrace';
import { WorkflowRunTrace } from './trajectory/workflow/WorkflowRunTrace';
export {
  LazyTraceNodeItem,
  TraceProjectionStatusNotice
} from './LazyConversationTrace';
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

function ConversationLogDetailContent({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  overview,
  record,
  overviewLoader
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overview?: ConversationLogRunOverview;
  record?: ConsoleApplicationLogRecordOverview;
  overviewLoader?: ConversationLogOverviewLoader;
  traceLoader?: ConversationLogTraceLoader;
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
      {!record ? (
        <div className="agent-flow-editor__conversation-log-json-list">
          <NodeRunPayloadSections
            key={message.detailRunId ?? message.runId ?? message.id}
            defaultCollapsed={Boolean(overview)}
            onLoadSection={
              overviewLoader?.loadPayload &&
              (message.detailRunId ?? message.runId)
                ? (section) => {
                    if (section === 'debug_payload') return Promise.resolve({});
                    return overviewLoader.loadPayload!(
                      message.detailRunId ?? message.runId!,
                      section
                    );
                  }
                : undefined
            }
            debugPayload={{}}
            includeDebugPayload={false}
            inputPayload={buildDetailInput(message)}
            outputPayload={buildDetailOutput(message)}
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
          />
        </div>
      ) : null}
      <section
        aria-label={i18nText('agentFlow', 'auto.metadata')}
        className="agent-flow-editor__conversation-log-metadata"
      >
        <div className="agent-flow-editor__conversation-log-metadata-heading">
          <Typography.Text strong>
            {i18nText('agentFlow', 'auto.metadata')}
          </Typography.Text>
        </div>
        <Descriptions
          column={1}
          items={
            record
              ? [
                  {
                    key: 'record_id',
                    label: i18nText('applications', 'agent_logs.record_id'),
                    children: record.record_id
                  },
                  {
                    key: 'source_client',
                    label: i18nText('applications', 'agent_logs.source_client'),
                    children: record.source_client ?? '—'
                  },
                  {
                    key: 'status',
                    label: i18nText('agentFlow', 'auto.status'),
                    children: record.outcome
                  },
                  {
                    key: 'total_tokens',
                    label: i18nText('agentFlow', 'auto.total_tokens'),
                    children: formatNullableNumber(record.total_tokens)
                  },
                  {
                    key: 'total_cost',
                    label: i18nText('applications', 'auto.total_cost'),
                    children:
                      record.cost_breakdown.total_cost != null
                        ? `${formatNumber(Number(record.cost_breakdown.total_cost), { maximumSignificantDigits: 21 })} $`
                        : '—'
                  }
                ]
              : [
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
                ]
          }
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
  traceLoader,
  overviewRunId
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overviewLoader: ConversationLogOverviewLoader;
  traceLoader?: ConversationLogTraceLoader;
  overviewRunId: string;
}) {
  const overviewQuery = useQuery({
    queryKey: ['conversation-log-run-overview', overviewRunId],
    enabled: Boolean(overviewLoader.loadOverview),
    queryFn: () => overviewLoader.loadOverview!(overviewRunId),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });

  const recordQuery = useQuery({
    queryKey: ['conversation-log-record-overview', overviewRunId],
    enabled: Boolean(overviewLoader.loadRecordOverview),
    queryFn: () => overviewLoader.loadRecordOverview!(overviewRunId),
    refetchOnWindowFocus: false,
    staleTime: CONVERSATION_LOG_QUERY_STALE_TIME_MS
  });
  if (overviewQuery.isError || recordQuery.isError) {
    return (
      <Alert
        type="error"
        title={i18nText('agentFlow', 'auto.loading_failed')}
      />
    );
  }
  if (overviewQuery.isLoading || recordQuery.isLoading) {
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
      record={recordQuery.data}
      overviewLoader={overviewLoader}
      traceLoader={traceLoader}
      onLoadArtifact={onLoadArtifact}
      onLoadArtifacts={onLoadArtifacts}
    />
  );
}

function ConversationLogDetail({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  overviewLoader,
  traceLoader
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  overviewLoader?: ConversationLogOverviewLoader;
  traceLoader?: ConversationLogTraceLoader;
}) {
  const overviewRunId = message.detailRunId ?? message.runId;

  if (overviewLoader && overviewRunId) {
    return (
      <ConversationLogLazyDetail
        message={message}
        overviewLoader={overviewLoader}
        traceLoader={traceLoader}
        overviewRunId={overviewRunId}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
      />
    );
  }

  return (
    <ConversationLogDetailContent
      message={message}
      overviewLoader={overviewLoader}
      traceLoader={traceLoader}
      onLoadArtifact={onLoadArtifact}
      onLoadArtifacts={onLoadArtifacts}
    />
  );
}

function ConversationTrace({
  workflowTrace,
  defaultToolsExpanded,
  message,
  onLoadArtifact,
  onLoadArtifacts,
  traceLoader
}: {
  workflowTrace: boolean;
  defaultToolsExpanded: boolean;
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  traceLoader?: ConversationLogTraceLoader;
}) {
  const traceRunId = message.detailRunId ?? message.runId;

  if (traceLoader?.sourceKind === 'imported' && traceRunId) {
    return (
      <ConversationTrajectoryWorkspace
        recordId={traceRunId}
        loader={traceLoader}
      />
    );
  }
  if (
    (traceLoader?.loadRecordClientTrajectory ||
      traceLoader?.loadClientTrajectory) &&
    !traceLoader.loadTree &&
    traceRunId
  ) {
    return (
      <ClientTrajectoryWorkspace runId={traceRunId} loader={traceLoader} />
    );
  }
  if (traceLoader?.loadTree && traceRunId) {
    const nodeTree = (
      <LazyConversationTrace
        key={`${message.id}:${traceRunId}`}
        defaultToolsExpanded={defaultToolsExpanded}
        onLoadArtifact={onLoadArtifact}
        onLoadArtifacts={onLoadArtifacts}
        runId={traceRunId}
        traceLoader={traceLoader}
      />
    );
    return workflowTrace && traceLoader.loadWorkflowTrajectory ? (
      <WorkflowRunTrace
        key={traceRunId}
        runId={traceRunId}
        loader={traceLoader}
      >
        {nodeTree}
      </WorkflowRunTrace>
    ) : (
      nodeTree
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
  activeTab,
  onTabChange,
  title,
  closeLabel,
  traceLabel,
  defaultTraceToolsExpanded = false,
  workflowTrace = false,
  exportingRun = false,
  message,
  onClose,
  onExportRun,
  onLoadArtifact,
  onLoadArtifacts,
  overviewLoader,
  traceLoader
}: {
  activeTab?: 'detail' | 'trace';
  onTabChange?: (tab: 'detail' | 'trace') => void;
  title?: string;
  closeLabel?: string;
  traceLabel?: string;
  defaultTraceToolsExpanded?: boolean;
  workflowTrace?: boolean;
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
  const [localActiveTab, setLocalActiveTab] = useState<'detail' | 'trace'>(
    'detail'
  );
  const activeTabKey = activeTab ?? localActiveTab;
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
      closeLabel={
        closeLabel ?? i18nText('agentFlow', 'auto.turn_off_conversation_log')
      }
      title={title ?? i18nText('agentFlow', 'auto.conversation_log')}
      onClose={onClose}
    >
      <Tabs
        activeKey={activeTabKey}
        classNames={{
          body: 'agent-flow-editor__conversation-log-tabs-body',
          content: 'agent-flow-editor__conversation-log-scroll-region',
          header: 'agent-flow-editor__conversation-log-tabs-header',
          item: 'agent-flow-editor__conversation-log-tabs-item'
        }}
        className="agent-flow-editor__conversation-log-tabs"
        items={[
          {
            key: 'detail',
            label: i18nText('agentFlow', 'auto.details'),
            children: (
              <ConversationLogDetail
                message={message}
                overviewLoader={overviewLoader}
                traceLoader={traceLoader}
                onLoadArtifact={loadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
            )
          },
          {
            key: 'trace',
            label: traceLabel ?? i18nText('agentFlow', 'auto.track'),
            children:
              activeTabKey === 'trace' ? (
                <ConversationTrace
                  workflowTrace={workflowTrace}
                  defaultToolsExpanded={defaultTraceToolsExpanded}
                  message={message}
                  onLoadArtifact={loadArtifact}
                  onLoadArtifacts={onLoadArtifacts}
                  traceLoader={traceLoader}
                />
              ) : null
          }
        ]}
        onChange={(key) => {
          const tab = key === 'trace' ? 'trace' : 'detail';
          setLocalActiveTab(tab);
          onTabChange?.(tab);
        }}
      />
    </AgentFlowDockPanel>
  );
}
