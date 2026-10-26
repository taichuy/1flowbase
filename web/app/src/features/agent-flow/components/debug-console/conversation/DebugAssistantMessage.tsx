import BranchesOutlined from '@ant-design/icons/es/icons/BranchesOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import HistoryOutlined from '@ant-design/icons/es/icons/HistoryOutlined';
import { Button, Tooltip } from 'antd';
import type { ReactNode } from 'react';

import type { AgentFlowDebugMessage } from '../../../api/runtime';
import type { RuntimeDebugArtifactBatchLoader } from '../../detail/last-run/runtime-debug-payload';
import { parseAssistantContent } from '../../../lib/debug-console/assistant-content';
import { MessageActions } from './actions/MessageActions';
import { MessageCopyButton } from './actions/MessageCopyButton';
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
    return message.presentation === 'status'
      ? i18nText('agentFlow', 'auto.cancelled_without_reply')
      : i18nText('agentFlow', 'auto.stopped');
  }

  if (message.status === 'failed') {
    return i18nText('agentFlow', 'auto.debug_run_failed_alt');
  }

  return message.presentation === 'status'
    ? i18nText('agentFlow', 'auto.no_reply_yet')
    : i18nText('agentFlow', 'auto.no_output_yet');
}

export function DebugAssistantMessage({
  message,
  messageMain,
  extraActions,
  footerStart,
  onLoadArtifact,
  onLoadArtifacts,
  onOpenLog,
  onOpenResumeTimeline
}: {
  message: AgentFlowDebugMessage;
  messageMain?: ReactNode;
  extraActions?: ReactNode;
  footerStart?: ReactNode;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onOpenLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
}) {
  const parsedContent = parseAssistantContent(message.content);
  const hasReasoning = Boolean(parsedContent.reasoningText.trim());
  const hasAnswer = Boolean(parsedContent.answerText.trim());
  const canOpenLog = message.canOpenDetail !== false;
  const openLogLabel =
    message.presentation === 'answer'
      ? i18nText('appShell', 'auto.assistant_activity')
      : i18nText('agentFlow', 'auto.view_conversation_log');

  return (
    <article
      className={[
        'agent-flow-editor__debug-message',
        'agent-flow-editor__debug-message-actions-owner',
        'agent-flow-editor__debug-message--assistant',
        message.presentation === 'status' &&
          'agent-flow-editor__debug-message--status'
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <div className="agent-flow-editor__debug-message-main">
        {messageMain ?? (
          <>
            {message.presentation !== 'answer' &&
            message.presentation !== 'status' ? (
              <DebugWorkflowProcess
                items={message.traceSummary}
                reasoning={parsedContent.reasoningText}
                reasoningStreaming={message.status === 'running'}
                onLoadArtifact={onLoadArtifact}
                onLoadArtifacts={onLoadArtifacts}
              />
            ) : null}
            {hasAnswer || !hasReasoning || message.status === 'cancelled' ? (
              <DebugMarkdownContent
                className="agent-flow-editor__debug-message-content"
                content={
                  hasAnswer
                    ? parsedContent.answerText
                    : fallbackContent(message)
                }
                streaming={message.status === 'running'}
              />
            ) : null}
          </>
        )}
      </div>
      <MessageActions start={footerStart}>
        {message.presentation !== 'status' ? (
          <MessageCopyButton
            text={parsedContent.answerText}
            label={i18nText('agentFlow', 'auto.copy_output')}
          />
        ) : null}
        {onOpenLog && canOpenLog ? (
          <Tooltip title={openLogLabel}>
            <Button
              aria-label={openLogLabel}
              icon={
                message.presentation === 'answer' ? (
                  <BranchesOutlined />
                ) : (
                  <FileTextOutlined />
                )
              }
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
        {extraActions}
      </MessageActions>
    </article>
  );
}
