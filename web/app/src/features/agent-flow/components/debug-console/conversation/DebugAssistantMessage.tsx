import {
  CopyOutlined,
  FileTextOutlined,
  HistoryOutlined
} from '@ant-design/icons';
import { App, Button, Space, Tooltip } from 'antd';

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
  const parsedContent = parseAssistantContent(message.content);
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
          reasoning={parsedContent.reasoningText}
          reasoningStreaming={message.status === 'running'}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
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
