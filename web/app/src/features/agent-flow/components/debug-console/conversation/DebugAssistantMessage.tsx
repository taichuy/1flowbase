import {
  CopyOutlined,
  DownOutlined,
  FileTextOutlined,
  HistoryOutlined,
  RightOutlined
} from '@ant-design/icons';
import { App, Button, Space, Tooltip } from 'antd';
import { useEffect, useState } from 'react';

import type { AgentFlowDebugMessage } from '../../../api/runtime';
import type { RuntimeDebugArtifactBatchLoader } from '../../detail/last-run/runtime-debug-payload';
import { parseAssistantContent } from '../../../lib/debug-console/assistant-content';
import { copyTextToClipboard } from '../../../../../shared/ui/clipboard/copy-text';
import { DebugMarkdownContent } from './DebugMarkdownContent';
import { DebugWorkflowProcess } from './DebugWorkflowProcess';
import './debug-message.css';
import { i18nText } from '../../../../../shared/i18n/text';

function fallbackContent(message: AgentFlowDebugMessage) {
  if (message.status === 'running') {
    return i18nText('agentFlow', 'auto.running');
  }

  if (message.status === 'waiting_human') {
    return i18nText('agentFlow', 'auto.wait_manual_intervention');
  }

  if (message.status === 'waiting_callback') {
    return i18nText('agentFlow', 'auto.wait_external_callback');
  }

  if (message.status === 'cancelled') {
    return i18nText('agentFlow', 'auto.stopped');
  }

  if (message.status === 'failed') {
    return i18nText('agentFlow', 'auto.debug_run_failed_alt');
  }

  return i18nText('agentFlow', 'auto.no_output_yet');
}

const TYPEWRITER_INTERVAL_MS = 24;
const TYPEWRITER_CHARS_PER_TICK = 12;

interface ProgressiveTextState {
  enabled: boolean;
  target: string;
  visibleText: string;
}

function resolveVisibleTextOnTargetChange({
  currentVisibleText,
  enabled,
  target
}: {
  currentVisibleText: string;
  enabled: boolean;
  target: string;
}) {
  if (!enabled) {
    return target;
  }

  if (!target) {
    return '';
  }

  return target.startsWith(currentVisibleText) ? currentVisibleText : target;
}

function useProgressiveText(target: string, enabled: boolean) {
  const [state, setState] = useState<ProgressiveTextState>(() => ({
    enabled,
    target,
    visibleText: target
  }));
  const stateChanged = state.enabled !== enabled || state.target !== target;
  const effectiveState = stateChanged
    ? {
        enabled,
        target,
        visibleText: resolveVisibleTextOnTargetChange({
          currentVisibleText: state.visibleText,
          enabled,
          target
        })
      }
    : state;

  useEffect(() => {
    if (!effectiveState.enabled) {
      return undefined;
    }

    if (effectiveState.visibleText.length >= effectiveState.target.length) {
      return undefined;
    }

    const timer = window.setTimeout(() => {
      setState((current) => ({
        ...current,
        visibleText: current.target.slice(
          0,
          Math.min(
            current.target.length,
            current.visibleText.length + TYPEWRITER_CHARS_PER_TICK
          )
        )
      }));
    }, TYPEWRITER_INTERVAL_MS);

    return () => window.clearTimeout(timer);
  }, [
    effectiveState.enabled,
    effectiveState.target,
    effectiveState.visibleText
  ]);

  if (stateChanged) {
    setState(effectiveState);
    return effectiveState.visibleText;
  }

  return state.visibleText;
}

export function DebugAssistantMessage({
  message,
  onLoadArtifact,
  onLoadArtifacts,
  onOpenLog,
  onOpenResumeTimeline
}: {
  message: AgentFlowDebugMessage;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onOpenLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
}) {
  const { message: messageApi } = App.useApp();
  const [isReasoningExpanded, setIsReasoningExpanded] = useState(true);
  const visibleContent = useProgressiveText(
    message.content,
    message.status !== 'running'
  );
  const parsedContent = parseAssistantContent(visibleContent);
  const parsedFullContent = parseAssistantContent(message.content);
  const hasReasoning = Boolean(parsedContent.reasoningText.trim());
  const hasAnswer = Boolean(parsedContent.answerText.trim());
  const canOpenLog = message.canOpenDetail !== false;

  async function handleCopyOutput() {
    if (!parsedFullContent.answerText) {
      return;
    }

    try {
      await copyTextToClipboard(parsedFullContent.answerText);
      messageApi.success(i18nText('agentFlow', 'auto.copied'));
    } catch {
      messageApi.error(i18nText('agentFlow', 'auto.copy_failed'));
    }
  }

  return (
    <article className="agent-flow-editor__debug-message agent-flow-editor__debug-message--assistant">
      <div className="agent-flow-editor__debug-message-main">
        <DebugWorkflowProcess
          items={message.traceSummary}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
        {hasReasoning ? (
          <section
            aria-label={i18nText('agentFlow', 'auto.think')}
            className="agent-flow-editor__debug-reasoning"
          >
            <button
              aria-expanded={isReasoningExpanded}
              className="agent-flow-editor__debug-reasoning-toggle"
              type="button"
              onClick={() => setIsReasoningExpanded((current) => !current)}
            >
              {isReasoningExpanded ? <DownOutlined /> : <RightOutlined />}
              <span className="agent-flow-editor__debug-reasoning-title">
                {i18nText('agentFlow', 'auto.think')}
              </span>
            </button>
            {isReasoningExpanded ? (
              <DebugMarkdownContent
                className="agent-flow-editor__debug-reasoning-content"
                content={parsedContent.reasoningText}
                streaming={message.status === 'running'}
              />
            ) : null}
          </section>
        ) : null}
        {hasAnswer || !hasReasoning ? (
          <DebugMarkdownContent
            className="agent-flow-editor__debug-message-content"
            content={
              hasAnswer ? parsedContent.answerText : fallbackContent(message)
            }
            streaming={message.status === 'running'}
          />
        ) : null}
      </div>
      <fieldset
        aria-label={i18nText('agentFlow', 'auto.output_action')}
        className="agent-flow-editor__debug-message-action-row"
      >
        <Space
          className="agent-flow-editor__debug-message-actions"
          size={8}
          wrap
        >
          <Tooltip title={i18nText('agentFlow', 'auto.copy_output')}>
            <Button
              aria-label={i18nText('agentFlow', 'auto.copy_output')}
              disabled={!parsedFullContent.answerText}
              icon={<CopyOutlined />}
              size="small"
              onClick={() => {
                void handleCopyOutput();
              }}
            />
          </Tooltip>
          {onOpenLog && canOpenLog ? (
            <Tooltip
              title={i18nText('agentFlow', 'auto.view_conversation_log')}
            >
              <Button
                aria-label={i18nText('agentFlow', 'auto.view_conversation_log')}
                icon={<FileTextOutlined />}
                size="small"
                onClick={() => onOpenLog(message)}
              />
            </Tooltip>
          ) : null}
          {onOpenResumeTimeline ? (
            <Tooltip title={i18nText('agentFlow', 'auto.view_resume_timeline')}>
              <Button
                aria-label={i18nText('agentFlow', 'auto.view_resume_timeline')}
                icon={<HistoryOutlined />}
                size="small"
                onClick={() => onOpenResumeTimeline(message)}
              />
            </Tooltip>
          ) : null}
        </Space>
      </fieldset>
    </article>
  );
}
