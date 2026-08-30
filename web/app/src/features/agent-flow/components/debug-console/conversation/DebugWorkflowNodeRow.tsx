import CheckCircleFilled from '@ant-design/icons/es/icons/CheckCircleFilled';
import DownOutlined from '@ant-design/icons/es/icons/DownOutlined';
import LoadingOutlined from '@ant-design/icons/es/icons/LoadingOutlined';
import RightOutlined from '@ant-design/icons/es/icons/RightOutlined';
import WarningFilled from '@ant-design/icons/es/icons/WarningFilled';
import { Tag, Tooltip, Typography } from 'antd';
import type { ReactNode } from 'react';

import type { AgentFlowTraceItem } from '../../../api/runtime';
import { getAgentFlowNodeTypeIcon } from '../../../lib/node-type-icons';
import { nodeDisplayName } from './debug-workflow-trace-utils';
import { collectLlmToolCallbacks } from './llm-tool-callbacks';
import './debug-message.css';
import { i18nText } from '../../../../../shared/i18n/text';
import { formatTokens, formatDurationScaled } from './metrics-formatter';

function statusTone(status: string) {
  switch (status) {
    case 'succeeded':
      return 'success';
    case 'failed':
      return 'error';
    case 'intercepted':
    case 'waiting_human':
    case 'waiting_callback':
      return 'warning';
    default:
      return 'running';
  }
}

function readRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return null;
  }

  return value as Record<string, unknown>;
}

function readTotalTokens(payload: unknown) {
  const record = readRecord(payload);
  if (!record) {
    return null;
  }

  const directTotalTokens = record.total_tokens;
  if (typeof directTotalTokens === 'number') {
    return directTotalTokens;
  }

  const usage = readRecord(record.usage);
  if (!usage) {
    return null;
  }

  const totalTokens = usage.total_tokens;
  return typeof totalTokens === 'number' ? totalTokens : null;
}

function readToolMode(item: AgentFlowTraceItem) {
  if (item.nodeType !== 'tool') {
    return null;
  }

  const debugPayload = readRecord(item.debugPayload);
  if (!debugPayload) {
    return null;
  }

  const toolMode = debugPayload.tool_mode;
  if (toolMode === 'fusion') {
    return i18nText('agentFlow', 'auto.tool_mode_fusion');
  }
  if (toolMode === 'route') {
    return i18nText('agentFlow', 'auto.tool_mode_agent');
  }

  const routeTrace = readRecord(debugPayload.route_trace);
  const routeKind = routeTrace?.route_kind;
  if (routeKind === 'fusion') {
    return i18nText('agentFlow', 'auto.tool_mode_fusion');
  }
  if (routeKind === 'route') {
    return i18nText('agentFlow', 'auto.tool_mode_agent');
  }

  return null;
}

function metricText(item: AgentFlowTraceItem) {
  const tokens =
    readTotalTokens(item.metricsPayload) ?? readTotalTokens(item.outputPayload);
  const durationMs = item.durationMs;
  const toolCount =
    item.nodeType === 'llm'
      ? collectLlmToolCallbacks(item.debugPayload).length
      : 0;

  const elements: ReactNode[] = [];

  if (typeof tokens === 'number') {
    const formattedTokens = `${formatTokens(tokens)} tokens`;
    elements.push(
      <Tooltip title={`${tokens.toLocaleString()} tokens`} key="tokens">
        <span>{formattedTokens}</span>
      </Tooltip>
    );
  }

  if (typeof durationMs === 'number') {
    const formattedDuration = formatDurationScaled(durationMs);
    elements.push(
      <Tooltip title={`${durationMs.toLocaleString()} ms`} key="duration">
        <span>{formattedDuration}</span>
      </Tooltip>
    );
  }

  if (toolCount > 0) {
    elements.push(
      <span key="tools">
        {i18nText('agentFlow', 'auto.tools_alt', { value1: toolCount })}
      </span>
    );
  }

  if (elements.length === 0) {
    if (statusTone(item.status) === 'success') {
      return i18nText('agentFlow', 'auto.executed_successfully');
    }
    if (statusTone(item.status) === 'error') {
      return i18nText('agentFlow', 'auto.execution_failed');
    }
    if (item.status === 'intercepted') {
      return i18nText('agentFlow', 'auto.execution_intercepted');
    }
    return i18nText('agentFlow', 'auto.in_progress');
  }

  const joined: ReactNode[] = [];
  elements.forEach((el, index) => {
    joined.push(el);
    if (index < elements.length - 1) {
      joined.push(<span key={`dot-${index}`}> · </span>);
    }
  });

  return <>{joined}</>;
}

export function StatusIcon({ status }: { status: string }) {
  const tone = statusTone(status);

  if (tone === 'running') {
    return (
      <LoadingOutlined
        aria-label={i18nText('agentFlow', 'auto.status_alt', {
          value1: status
        })}
        className="agent-flow-editor__debug-workflow-status-icon"
        spin
      />
    );
  }

  if (tone === 'error' || tone === 'warning') {
    return (
      <WarningFilled
        aria-label={i18nText('agentFlow', 'auto.status_alt', {
          value1: status
        })}
        className={`agent-flow-editor__debug-workflow-status-icon agent-flow-editor__debug-workflow-status-icon--${tone}`}
      />
    );
  }

  return (
    <CheckCircleFilled
      aria-label={i18nText('agentFlow', 'auto.status_alt', { value1: status })}
      className={`agent-flow-editor__debug-workflow-status-icon agent-flow-editor__debug-workflow-status-icon--${tone}`}
    />
  );
}

export function NodeTypeIcon({ nodeType }: { nodeType: string }) {
  return (
    <span
      aria-label={i18nText('agentFlow', 'auto.node_type_alt', {
        value1: nodeType
      })}
      className="agent-flow-editor__debug-workflow-node-icon"
    >
      {getAgentFlowNodeTypeIcon(nodeType)}
    </span>
  );
}

export function DebugWorkflowNodeRow({ item }: { item: AgentFlowTraceItem }) {
  const toolMode = readToolMode(item);

  return (
    <span
      className="agent-flow-editor__debug-workflow-row"
      data-testid="debug-workflow-node-row"
    >
      <NodeTypeIcon nodeType={item.nodeType} />
      <span className="agent-flow-editor__debug-workflow-node-main">
        <span className="agent-flow-editor__debug-workflow-node-title">
          <Typography.Text strong>{nodeDisplayName(item)}</Typography.Text>
          {toolMode ? (
            <span
              className="agent-flow-editor__debug-workflow-node-mode"
              data-testid="debug-workflow-node-mode"
            >
              {toolMode}
            </span>
          ) : null}
        </span>
        <Typography.Text
          className="agent-flow-editor__debug-workflow-metric"
          type="secondary"
        >
          {metricText(item)}
        </Typography.Text>
      </span>
      <Tag className="agent-flow-editor__debug-workflow-node-type">
        {item.nodeType}
      </Tag>
      <StatusIcon status={item.status} />
    </span>
  );
}

export function DebugWorkflowNodeItem({
  item,
  expanded,
  selected = false,
  children,
  onToggle
}: {
  item: AgentFlowTraceItem;
  expanded: boolean;
  selected?: boolean;
  children: ReactNode;
  onToggle: () => void;
}) {
  return (
    <div
      className="agent-flow-editor__debug-workflow-node-item"
      data-expanded={expanded ? 'true' : 'false'}
      data-selected={selected ? 'true' : 'false'}
      data-node-type={item.nodeType}
      data-testid="debug-workflow-node-item"
    >
      <button
        aria-expanded={expanded}
        className="agent-flow-editor__debug-workflow-node-trigger"
        onClick={onToggle}
        type="button"
      >
        <DebugWorkflowNodeRow item={item} />
        {expanded ? (
          <DownOutlined className="agent-flow-editor__debug-workflow-collapse" />
        ) : (
          <RightOutlined className="agent-flow-editor__debug-workflow-collapse" />
        )}
      </button>
      {expanded ? children : null}
    </div>
  );
}
