import { Bubble } from '@ant-design/x';
import { Empty, Typography } from 'antd';
import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode
} from 'react';

import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../../api/runtime';
import type { AgentFlowDebugSessionStatus } from '../../../hooks/runtime/useAgentFlowDebugSession';
import type { RuntimeDebugArtifactBatchLoader } from '../../detail/last-run/runtime-debug-payload';
import { DebugAssistantMessage } from './DebugAssistantMessage';
import { DebugComposer } from './DebugComposer';
import { DebugMarkdownContent } from './DebugMarkdownContent';
import { i18nText } from '../../../../../shared/i18n/text';

const HISTORY_LOAD_SCROLL_THRESHOLD_PX = 96;

interface ConversationScrollSnapshot {
  firstMessageId: string | null;
  initialized: boolean;
  lastMessageId: string | null;
  scrollHeight: number;
  scrollTop: number;
}

function getQueryField(runContext: AgentFlowRunContext) {
  return runContext.fields.find((field) => field.key === 'query') ?? null;
}

function bubbleMessageStatus(message: AgentFlowDebugMessage) {
  switch (message.status) {
    case 'running':
      return message.content ? 'updating' : 'loading';
    case 'failed':
      return 'error';
    case 'cancelled':
      return 'abort';
    default:
      return 'success';
  }
}

export function DebugConversationPane({
  composerUiOnly = false,
  composerFooterActions,
  logActionRunId,
  status,
  stopping,
  runContext,
  messages,
  onChangeQuery,
  onLoadArtifact,
  onLoadArtifacts,
  onOpenMessageLog,
  onOpenResumeTimeline,
  onReachTop,
  onStopRun,
  onSubmitPrompt,
  showComposer = true
}: {
  status: AgentFlowDebugSessionStatus;
  stopping: boolean;
  runContext: AgentFlowRunContext;
  messages: AgentFlowDebugMessage[];
  onChangeQuery: (value: string) => void;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
  onReachTop?: () => void;
  onStopRun: () => void;
  onSubmitPrompt: (prompt: string) => void;
  composerUiOnly?: boolean;
  composerFooterActions?: ReactNode;
  logActionRunId?: string | null;
  showComposer?: boolean;
}) {
  const [uiOnlyComposerValue, setUiOnlyComposerValue] = useState('');
  const messagesRef = useRef<HTMLDivElement | null>(null);
  const messageListRef = useRef<HTMLDivElement | null>(null);
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const autoScrollEnabledRef = useRef(true);
  const activeAssistantMessageIdRef = useRef<string | null>(null);
  const scrollSnapshotRef = useRef<ConversationScrollSnapshot>({
    firstMessageId: null,
    initialized: false,
    lastMessageId: null,
    scrollHeight: 0,
    scrollTop: 0
  });
  const queryField = getQueryField(runContext);
  const composerDisabled =
    !queryField ||
    status === 'running' ||
    status === 'waiting_human' ||
    status === 'waiting_callback';
  const stopAvailable =
    status === 'running' ||
    status === 'waiting_human' ||
    status === 'waiting_callback';
  const activeAssistantMessage =
    [...messages].reverse().find((message) => message.role === 'assistant') ??
    null;
  const activeAssistantMessageId = activeAssistantMessage?.id ?? null;
  const activeAssistantContent = activeAssistantMessage?.content ?? '';
  const firstMessageId = messages[0]?.id ?? null;
  const lastMessageId = messages.at(-1)?.id ?? null;
  const bubbleItems = useMemo(
    () =>
      messages.map((message) => ({
        key: message.id,
        role: message.role,
        content: message,
        loading: message.role === 'assistant' && message.status === 'running' && !message.content,
        status: bubbleMessageStatus(message),
        streaming: message.role === 'assistant' && message.status === 'running'
      })),
    [messages]
  );
  const bubbleRoles = useMemo(
    () => ({
      assistant: {
        placement: 'start' as const,
        rootClassName: 'agent-flow-editor__debug-assistant-bubble',
        styles: {
          body: { width: '100%' },
          content: { width: '100%' },
          root: { paddingInlineEnd: 0, width: '100%' }
        },
        variant: 'borderless' as const,
        contentRender: (message: AgentFlowDebugMessage) => (
          <DebugAssistantMessage
            message={message}
            onLoadArtifact={onLoadArtifact}
            onLoadArtifacts={onLoadArtifacts}
            onOpenLog={
              !logActionRunId ||
              (message.detailRunId ?? message.runId) === logActionRunId
                ? onOpenMessageLog
                : undefined
            }
            onOpenResumeTimeline={onOpenResumeTimeline}
          />
        )
      },
      system: {
        placement: 'start' as const,
        variant: 'borderless' as const,
        contentRender: (message: AgentFlowDebugMessage) => (
          <DebugMarkdownContent
            className="agent-flow-editor__debug-message-content"
            content={message.content}
          />
        )
      },
      user: {
        placement: 'end' as const,
        shape: 'corner' as const,
        contentRender: (message: AgentFlowDebugMessage) => (
          <Typography.Paragraph className="agent-flow-editor__debug-message-content">
            {message.content}
          </Typography.Paragraph>
        )
      }
    }),
    [logActionRunId, onLoadArtifact, onLoadArtifacts, onOpenMessageLog, onOpenResumeTimeline]
  );
  const rememberScrollPosition = useCallback(
    (element: HTMLDivElement | null = messagesRef.current) => {
      if (!element) {
        return;
      }

      scrollSnapshotRef.current = {
        firstMessageId,
        initialized: messages.length > 0,
        lastMessageId,
        scrollHeight: element.scrollHeight,
        scrollTop: element.scrollTop
      };
    },
    [firstMessageId, lastMessageId, messages.length]
  );
  const scrollToBottom = useCallback(
    (force = false) => {
      if (!force && (status !== 'running' || !autoScrollEnabledRef.current)) {
        return;
      }

      const element = messagesRef.current;

      if (element) {
        element.scrollTop = element.scrollHeight;
        return;
      }

      bottomRef.current?.scrollIntoView({ block: 'end' });
    },
    [status]
  );

  useLayoutEffect(() => {
    const element = messagesRef.current;
    const previousSnapshot = scrollSnapshotRef.current;
    const activeAssistantChanged =
      activeAssistantMessageIdRef.current !== activeAssistantMessageId;

    if (activeAssistantChanged) {
      activeAssistantMessageIdRef.current = activeAssistantMessageId;
      autoScrollEnabledRef.current = true;
    }

    if (!element) {
      return;
    }

    if (messages.length === 0) {
      rememberScrollPosition(element);
      return;
    }

    const historyWasPrepended =
      previousSnapshot.initialized &&
      previousSnapshot.firstMessageId !== null &&
      previousSnapshot.firstMessageId !== firstMessageId &&
      previousSnapshot.lastMessageId === lastMessageId;

    if (historyWasPrepended) {
      const insertedHeight =
        element.scrollHeight - previousSnapshot.scrollHeight;
      element.scrollTop = previousSnapshot.scrollTop + insertedHeight;
    } else if (
      !previousSnapshot.initialized ||
      previousSnapshot.lastMessageId !== lastMessageId ||
      activeAssistantChanged
    ) {
      scrollToBottom(true);
    } else {
      scrollToBottom();
    }

    rememberScrollPosition(element);
  }, [
    activeAssistantContent,
    activeAssistantMessageId,
    firstMessageId,
    lastMessageId,
    messages.length,
    rememberScrollPosition,
    scrollToBottom
  ]);

  useLayoutEffect(() => {
    if (typeof ResizeObserver === 'undefined') {
      return;
    }

    const element = messageListRef.current;

    if (!element) {
      return;
    }

    const observer = new ResizeObserver(() => {
      scrollToBottom();
      rememberScrollPosition();
    });
    observer.observe(element);

    return () => observer.disconnect();
  }, [rememberScrollPosition, scrollToBottom]);

  function pauseAutoScroll() {
    if (status === 'running') {
      autoScrollEnabledRef.current = false;
    }

    rememberScrollPosition();
  }

  function handleMessagesScroll() {
    const element = messagesRef.current;

    if (!element) {
      return;
    }

    rememberScrollPosition(element);

    if (element.scrollTop <= HISTORY_LOAD_SCROLL_THRESHOLD_PX) {
      onReachTop?.();
    }
  }

  return (
    <div className="agent-flow-editor__debug-console-pane agent-flow-editor__debug-conversation-pane">
      <div
        ref={messagesRef}
        className="agent-flow-editor__debug-messages"
        data-testid="debug-conversation-messages"
        onPointerDown={pauseAutoScroll}
        onScroll={handleMessagesScroll}
        onTouchMove={pauseAutoScroll}
        onWheel={pauseAutoScroll}
      >
        <div
          ref={messageListRef}
          className="agent-flow-editor__debug-message-list"
        >
          {messages.length === 0 ? (
            <Empty
              description={i18nText(
                'agentFlow',
                'auto.rectification_operation_record_yet'
              )}
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          ) : (
            <Bubble.List
              autoScroll={false}
              items={bubbleItems}
              role={bubbleRoles}
              rootClassName="agent-flow-editor__debug-bubble-list"
            />
          )}
          <div
            ref={bottomRef}
            aria-hidden="true"
            className="agent-flow-editor__debug-bottom-sentinel"
            data-testid="debug-conversation-bottom-sentinel"
          />
        </div>
      </div>
      {showComposer ? (
        <DebugComposer
          disabled={composerUiOnly ? false : composerDisabled}
          footerActions={composerFooterActions}
          submitting={composerUiOnly ? false : stopAvailable}
          stopping={composerUiOnly ? false : stopping}
          value={
            composerUiOnly
              ? uiOnlyComposerValue
              : typeof queryField?.value === 'string'
                ? queryField.value
                : ''
          }
          onChange={composerUiOnly ? setUiOnlyComposerValue : onChangeQuery}
          onStop={composerUiOnly ? () => {} : onStopRun}
          onSubmit={
            composerUiOnly ? () => setUiOnlyComposerValue('') : onSubmitPrompt
          }
        />
      ) : null}
    </div>
  );
}
