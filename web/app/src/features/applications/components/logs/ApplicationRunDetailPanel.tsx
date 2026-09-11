import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import { useQuery } from '@tanstack/react-query';
import { App, Button, Tooltip } from 'antd';
import { useEffect, useMemo, useRef, useState } from 'react';

import { AgentFlowDebugConsole } from '../../../agent-flow/components/debug-console/AgentFlowDebugConsole';
import type {
  AgentFlowDebugMessage,
  AgentFlowDebugMessageStatus,
  AgentFlowRunContext
} from '../../../agent-flow/api/runtime';
import type { AgentFlowDebugSessionStatus } from '../../../agent-flow/hooks/runtime/useAgentFlowDebugSession';
import { useClipboardCopy } from '../../../../shared/ui/clipboard/use-clipboard-copy';
import {
  applicationRunConversationMessagesQueryKey,
  applicationLogConversationMessagesQueryKey,
  fetchApplicationLogConversationMessages,
  fetchApplicationRunConversationMessages,
  type ApplicationRunConversationMessage,
  type ApplicationRunConversationMessagesPage
} from '../../api/runtime';
import { isActiveRunStatus } from '../../lib/run-status';
import './application-run-detail-panel.css';
import { i18nText } from '../../../../shared/i18n/text';

const ACTIVE_CONVERSATION_REFETCH_INTERVAL_MS = 1_000;
const RUN_CONVERSATION_PAGE_LIMIT = 5;

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0 ? value : null;
}

function markdownDisplayText(value: string): string {
  const hasEscapedNewline = value.includes('\\n');
  const hasRealNewline = value.includes('\n');

  if (!hasEscapedNewline || hasRealNewline) {
    return value;
  }

  return value.replaceAll('\\r\\n', '\n').replaceAll('\\n', '\n');
}

function mapRunStatusToMessageStatus(
  status: string
): AgentFlowDebugMessageStatus {
  switch (status) {
    case 'succeeded':
      return 'completed';
    case 'waiting_callback':
      return 'waiting_callback';
    case 'waiting_human':
      return 'waiting_human';
    case 'cancelled':
      return 'cancelled';
    case 'failed':
      return 'failed';
    default:
      return 'running';
  }
}

function mapRunStatusToSessionStatus(
  status: string
): AgentFlowDebugSessionStatus {
  switch (status) {
    case 'succeeded':
      return 'completed';
    case 'waiting_callback':
      return 'waiting_callback';
    case 'waiting_human':
      return 'waiting_human';
    case 'cancelled':
      return 'cancelled';
    case 'failed':
      return 'failed';
    case 'running':
      return 'running';
    default:
      return 'completed';
  }
}

function fallbackConversationAnswerContent(
  item: ApplicationRunConversationMessage
) {
  if (!item.is_current || item.status === 'succeeded') {
    return null;
  }

  switch (item.status) {
    case 'waiting_callback':
      return i18nText(
        'applications',
        'auto.run_waiting_callback_without_output'
      );
    case 'waiting_human':
      return i18nText('applications', 'auto.run_waiting_human_without_output');
    case 'running':
      return i18nText('applications', 'auto.run_running_without_output');
    case 'failed':
      return i18nText('applications', 'auto.run_failed_without_output');
    case 'cancelled':
      return i18nText('applications', 'auto.run_cancelled_without_output');
    default:
      return i18nText('applications', 'auto.run_status_without_output');
  }
}

const runConversationContext: AgentFlowRunContext = {
  environmentLabel: 'draft',
  remembered: false,
  fields: []
};

function RunIdSubtitle({ runId }: { runId: string }) {
  const { message } = App.useApp();
  const { copied, copy } = useClipboardCopy();

  async function handleCopyRunId() {
    try {
      await copy(runId);
      message.success(i18nText('applications', 'auto.id_copied'));
    } catch {
      message.error(i18nText('applications', 'auto.copy_failed'));
    }
  }

  return (
    <span className="application-run-detail__run-id">
      <span className="application-run-detail__run-id-value">{runId}</span>
      <Tooltip title={i18nText('applications', 'auto.copy_id')}>
        <Button
          aria-label={i18nText('applications', 'auto.copy_run_id')}
          className="application-run-detail__run-id-copy"
          icon={copied ? <CheckOutlined /> : <CopyOutlined />}
          onClick={handleCopyRunId}
          size="small"
          type="text"
        />
      </Tooltip>
    </span>
  );
}

function conversationItemDetailRunId(
  item: ApplicationRunConversationMessage
): string | null {
  return nonEmptyString(item.detail_run_id);
}

function conversationMessageRole(
  item: ApplicationRunConversationMessage
): AgentFlowDebugMessage['role'] | null {
  switch (item.role) {
    case 'tool':
      return 'assistant';
    case 'system':
    case 'user':
    case 'assistant':
      return item.role;
    default:
      return null;
  }
}

function mapConversationItemToMessages(
  item: ApplicationRunConversationMessage
): AgentFlowDebugMessage[] {
  const detailRunId = conversationItemDetailRunId(item);
  const canOpenDetail = item.can_open_detail !== false && Boolean(detailRunId);
  const messageRole = conversationMessageRole(item);
  const messageContent = nonEmptyString(item.content);
  const flowRunId = nonEmptyString(item.run_id);

  if (messageRole && messageContent) {
    return [
      {
        id: `conversation-${messageRole}-${item.message_id}`,
        role: messageRole,
        content:
          messageRole === 'system' || messageRole === 'assistant'
            ? markdownDisplayText(messageContent)
            : messageContent,
        status: mapRunStatusToMessageStatus(item.status),
        runId: flowRunId,
        detailRunId,
        canOpenDetail,
        rawOutput: null,
        traceSummary: []
      }
    ];
  }

  const messages: AgentFlowDebugMessage[] = [];
  const queryContent = nonEmptyString(item.query);
  const answerContent =
    nonEmptyString(item.answer) ?? fallbackConversationAnswerContent(item);

  if (queryContent) {
    messages.push({
      id: `conversation-user-${item.message_id}`,
      role: 'user',
      content: queryContent,
      status: mapRunStatusToMessageStatus(item.status),
      runId: flowRunId,
      detailRunId,
      canOpenDetail,
      rawOutput: null,
      traceSummary: []
    });
  }

  if (answerContent) {
    messages.push({
      id: `conversation-assistant-${item.message_id}`,
      role: 'assistant',
      content: markdownDisplayText(answerContent),
      status: mapRunStatusToMessageStatus(item.status),
      runId: flowRunId,
      detailRunId,
      canOpenDetail,
      rawOutput: null,
      traceSummary: []
    });
  }

  return messages;
}

function buildConversationPageMessages(
  page: ApplicationRunConversationMessagesPage | null
): AgentFlowDebugMessage[] {
  if (!page || page.items.length === 0) {
    return [];
  }

  return page.items.flatMap((item) => mapConversationItemToMessages(item));
}

function conversationSessionStatus(
  page: ApplicationRunConversationMessagesPage | null
): AgentFlowDebugSessionStatus {
  const currentItem =
    page?.items.find((item) => isActiveRunStatus(item.status)) ??
    [...(page?.items ?? [])].reverse().find((item) => item.is_current) ??
    page?.items.at(-1) ??
    null;

  return mapRunStatusToSessionStatus(currentItem?.status ?? 'succeeded');
}

function hasActiveConversationItem(
  page: ApplicationRunConversationMessagesPage | null
) {
  return Boolean(page?.items.some((item) => isActiveRunStatus(item.status)));
}

function conversationItemKey(item: ApplicationRunConversationMessage) {
  return item.message_id;
}

function mergeConversationPages({
  initialPage,
  previousPages
}: {
  initialPage: ApplicationRunConversationMessagesPage | null;
  previousPages: ApplicationRunConversationMessagesPage[];
}): ApplicationRunConversationMessagesPage | null {
  if (!initialPage) {
    return null;
  }

  const items: ApplicationRunConversationMessage[] = [];
  const existingIds = new Set<string>();
  for (const page of [...previousPages, initialPage]) {
    for (const item of page.items) {
      const key = conversationItemKey(item);
      if (existingIds.has(key)) {
        continue;
      }
      existingIds.add(key);
      items.push(item);
    }
  }

  const firstPage = previousPages[0] ?? initialPage;

  return {
    items,
    page: {
      has_before: firstPage.page.has_before,
      has_after:
        initialPage.page.has_after ||
        previousPages.some((page) => page.page.has_after),
      before_cursor: firstPage.page.before_cursor,
      after_cursor: initialPage.page.after_cursor
    }
  };
}

function RunConversation({
  applicationId,
  logConversationId,
  onClose,
  onOpenMessageLog,
  onOpenResumeTimeline,
  runId
}: {
  applicationId: string;
  logConversationId?: string | null;
  onClose: () => void;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
  runId: string;
}) {
  const [conversationScope, setConversationScope] = useState(false);
  const [previousConversationPages, setPreviousConversationPages] = useState<
    ApplicationRunConversationMessagesPage[]
  >([]);
  const loadingPreviousConversationRef = useRef(false);
  const conversationScopeGeneration = useRef(0);
  const initialConversationQuery = useQuery({
    queryKey:
      conversationScope && logConversationId
        ? applicationLogConversationMessagesQueryKey(
            applicationId,
            logConversationId,
            { aroundRunId: runId, limit: RUN_CONVERSATION_PAGE_LIMIT }
          )
        : applicationRunConversationMessagesQueryKey(applicationId, runId, {
            limit: RUN_CONVERSATION_PAGE_LIMIT
          }),
    queryFn: () =>
      conversationScope && logConversationId
        ? fetchApplicationLogConversationMessages(
            applicationId,
            logConversationId,
            { aroundRunId: runId, limit: RUN_CONVERSATION_PAGE_LIMIT }
          )
        : fetchApplicationRunConversationMessages(applicationId, runId, {
            limit: RUN_CONVERSATION_PAGE_LIMIT
          }),
    refetchOnWindowFocus: false
  });
  const refetchInitialConversation = initialConversationQuery.refetch;
  const conversationPage = useMemo(
    () =>
      mergeConversationPages({
        initialPage: initialConversationQuery.data ?? null,
        previousPages: previousConversationPages
      }),
    [initialConversationQuery.data, previousConversationPages]
  );
  const messages = useMemo(
    () => buildConversationPageMessages(conversationPage),
    [conversationPage]
  );

  useEffect(() => {
    if (!hasActiveConversationItem(conversationPage)) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void refetchInitialConversation();
    }, ACTIVE_CONVERSATION_REFETCH_INTERVAL_MS);

    return () => window.clearInterval(intervalId);
  }, [conversationPage, refetchInitialConversation]);

  function handleOpenMessageLog(message: AgentFlowDebugMessage) {
    if (message.canOpenDetail === false) {
      return;
    }

    const detailRunId =
      nonEmptyString(message.detailRunId) ?? nonEmptyString(message.runId);

    if (!detailRunId) {
      return;
    }

    onOpenMessageLog?.({
      ...message,
      detailRunId,
      canOpenDetail: true
    });
  }

  async function loadPreviousConversationPage() {
    const before = conversationPage?.page.before_cursor;

    if (
      loadingPreviousConversationRef.current ||
      !conversationPage ||
      !conversationPage.page.has_before ||
      !before
    ) {
      return;
    }

    loadingPreviousConversationRef.current = true;
    const generation = conversationScopeGeneration.current;
    try {
      const page =
        conversationScope && logConversationId
          ? await fetchApplicationLogConversationMessages(
              applicationId,
              logConversationId,
              { before, limit: RUN_CONVERSATION_PAGE_LIMIT }
            )
          : await fetchApplicationRunConversationMessages(
              applicationId,
              runId,
              { before, limit: RUN_CONVERSATION_PAGE_LIMIT }
            );
      if (generation === conversationScopeGeneration.current) {
        setPreviousConversationPages((current) => [page, ...current]);
      }
    } finally {
      loadingPreviousConversationRef.current = false;
    }
  }

  return (
    <div className="application-run-detail__conversation-pane">
      <AgentFlowDebugConsole
        ariaLabel={i18nText('applications', 'auto.run_details_preview')}
        closeLabel={i18nText('applications', 'auto.close_run_details')}
        composerUiOnly
        messages={messages}
        runContext={runConversationContext}
        showClearAction={false}
        showComposer
        status={conversationSessionStatus(conversationPage)}
        stopping={false}
        subtitle={
          <>
            <RunIdSubtitle runId={runId} />
            {logConversationId && (
              <Button
                type="link"
                size="small"
                onClick={() => {
                  conversationScopeGeneration.current += 1;
                  setPreviousConversationPages([]);
                  setConversationScope((current) => !current);
                }}
              >
                {i18nText(
                  'applications',
                  conversationScope
                    ? 'auto.show_current_task'
                    : 'auto.show_log_conversation'
                )}
              </Button>
            )}
          </>
        }
        title={i18nText('applications', 'auto.run_details')}
        onChangeRunContextValue={() => {}}
        onClearSession={() => {}}
        onClose={onClose}
        onOpenMessageLog={(message) => {
          void handleOpenMessageLog(message);
        }}
        onOpenResumeTimeline={onOpenResumeTimeline}
        onReachConversationTop={() => {
          void loadPreviousConversationPage();
        }}
        onStopRun={() => {}}
        onSubmitPrompt={() => {}}
      />
    </div>
  );
}

export function ApplicationRunDetailPanel({
  applicationId,
  logConversationId,
  onClose,
  onOpenMessageLog,
  onOpenResumeTimeline,
  runId
}: {
  applicationId: string;
  logConversationId?: string | null;
  onClose: () => void;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
  runId: string | null;
}) {
  if (!runId) {
    return null;
  }

  return (
    <aside
      aria-label={i18nText('applications', 'auto.run_details')}
      className="application-run-detail application-run-detail--loaded"
    >
      <div className="application-run-detail__body">
        <div className="application-run-detail__content">
          <RunConversation
            key={runId}
            applicationId={applicationId}
            logConversationId={logConversationId}
            onClose={onClose}
            onOpenMessageLog={onOpenMessageLog}
            onOpenResumeTimeline={onOpenResumeTimeline}
            runId={runId}
          />
        </div>
      </div>
    </aside>
  );
}
