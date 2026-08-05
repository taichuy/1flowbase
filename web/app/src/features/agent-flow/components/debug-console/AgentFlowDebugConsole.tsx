import { ReloadOutlined } from '@ant-design/icons';
import { Button } from 'antd';
import type { ReactNode } from 'react';
import { useMemo, useState } from 'react';

import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../api/runtime';
import type { AgentFlowDebugSessionStatus } from '../../hooks/runtime/useAgentFlowDebugSession';
import { AgentFlowDockPanel } from '../editor/AgentFlowDockPanel';
import type { RuntimeDebugArtifactBatchLoader } from '../detail/last-run/runtime-debug-payload';
import { ConversationLogPanel } from './ConversationLogPanel';
import { DebugConversationPane } from './conversation/DebugConversationPane';
import { i18nText } from '../../../../shared/i18n/text';

export function AgentFlowDebugConsole({
  ariaLabel,
  closeLabel,
  composerUiOnly = false,
  headerActions,
  logActionRunId,
  messages,
  runContext,
  showClearAction = true,
  showComposer = true,
  status,
  stopping,
  subtitle,
  title = i18nText('agentFlow', 'auto.preview'),
  onChangeRunContextValue,
  onClearSession,
  onClose,
  onLoadArtifact,
  onLoadArtifacts,
  onOpenMessageLog,
  onOpenResumeTimeline,
  onReachConversationTop,
  onStopRun,
  onSubmitPrompt
}: {
  ariaLabel?: string;
  closeLabel?: string;
  composerUiOnly?: boolean;
  headerActions?: ReactNode;
  logActionRunId?: string | null;
  messages: AgentFlowDebugMessage[];
  runContext: AgentFlowRunContext;
  showClearAction?: boolean;
  showComposer?: boolean;
  status: AgentFlowDebugSessionStatus;
  stopping: boolean;
  subtitle?: ReactNode;
  title?: string;
  onChangeRunContextValue: (
    nodeId: string,
    key: string,
    value: unknown
  ) => void;
  onClearSession: () => void;
  onClose: () => void;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
  onReachConversationTop?: () => void;
  onStopRun: () => void;
  onSubmitPrompt: (prompt: string) => void;
}) {
  const [openLogMessageId, setOpenLogMessageId] = useState<string | null>(null);
  const openLogMessage = useMemo(
    () =>
      messages.find(
        (message) =>
          message.id === openLogMessageId && message.role === 'assistant'
      ) ?? null,
    [messages, openLogMessageId]
  );

  return (
    <>
      {!onOpenMessageLog && openLogMessage ? (
        <ConversationLogPanel
          message={openLogMessage}
          onClose={() => setOpenLogMessageId(null)}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
        />
      ) : null}
      <AgentFlowDockPanel
        actions={
          <>
            {showClearAction ? (
              <Button
                aria-label={i18nText('agentFlow', 'auto.clear_preview')}
                disabled={messages.length === 0}
                icon={<ReloadOutlined />}
                size="small"
                type="text"
                onClick={() => {
                  setOpenLogMessageId(null);
                  onClearSession();
                }}
              />
            ) : null}
            {headerActions}
          </>
        }
        ariaLabel={ariaLabel}
        bodyClassName="agent-flow-editor__debug-console-body"
        className="agent-flow-editor__debug-console"
        closeLabel={
          closeLabel ?? i18nText('agentFlow', 'auto.close', { value1: title })
        }
        subtitle={subtitle}
        title={title}
        onClose={onClose}
      >
        <DebugConversationPane
          composerUiOnly={composerUiOnly}
          logActionRunId={logActionRunId}
          messages={messages}
          runContext={runContext}
          status={status}
          stopping={stopping}
          onLoadArtifact={onLoadArtifact}
          onLoadArtifacts={onLoadArtifacts}
          onOpenResumeTimeline={onOpenResumeTimeline}
          onReachTop={onReachConversationTop}
          onOpenMessageLog={(message) => {
            if (onOpenMessageLog) {
              onOpenMessageLog(message);
              return;
            }

            setOpenLogMessageId(message.id);
          }}
          onChangeQuery={(value) => {
            const queryField =
              runContext.fields.find((field) => field.key === 'query') ?? null;

            if (!queryField) {
              return;
            }

            onChangeRunContextValue(queryField.nodeId, queryField.key, value);
          }}
          showComposer={showComposer}
          onStopRun={onStopRun}
          onSubmitPrompt={onSubmitPrompt}
        />
      </AgentFlowDockPanel>
    </>
  );
}
