import CheckOutlined from '@ant-design/icons/es/icons/CheckOutlined';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import MessageOutlined from '@ant-design/icons/es/icons/MessageOutlined';
import { useQuery } from '@tanstack/react-query';
import { App, Button, Tooltip, theme } from 'antd';
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
  type ApplicationRunConversationMessagesPage,
  type ApplicationRunConversationOutputState
} from '../../api/runtime';
import { isActiveRunStatus } from '../../lib/run-status';
import './application-run-detail-panel.css';
import { i18nText } from '../../../../shared/i18n/text';

const ACTIVE_CONVERSATION_REFETCH_INTERVAL_MS = 1_000;
const RUN_CONVERSATION_PAGE_LIMIT = 5;
const RUN_CONVERSATION_CATCH_UP_PAGE_LIMIT = 20;

/// A conversation keeps refreshing while the call can still produce facts.
/// Waiting states are not "active" for the run list, but new tool results and
/// new turns arrive while a call waits, so they must keep refreshing here.
const CONVERSATION_REFRESHABLE_STATUSES = new Set([
  'queued',
  'running',
  'paused',
  'waiting_callback',
  'waiting_human'
]);

function isConversationRefreshableStatus(status: string | null | undefined) {
  return CONVERSATION_REFRESHABLE_STATUSES.has(
    status?.trim().toLowerCase() ?? ''
  );
}

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

/// The system/developer context is injected as the first turns of the
/// conversation. The label states which layer it came from without adding
/// another surface beside the chat.
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
    case 'developer':
      return 'system';
    case 'system':
    case 'user':
    case 'assistant':
      return item.role;
    default:
      return null;
  }
}

function mapConversationItemToMessages(
  item: ApplicationRunConversationMessage,
  _outputState: ApplicationRunConversationOutputState | null
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
  const answerContent = nonEmptyString(item.answer);

  if (queryContent) {
    messages.push({
      id: `conversation-user-${item.message_id}`,
      role: 'user',
      content: queryContent,
      status: mapRunStatusToMessageStatus(item.status),
      runId: detailRunId ?? flowRunId,
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

function buildConversationMessages(
  items: ApplicationRunConversationMessage[],
  outputState: ApplicationRunConversationOutputState | null
): AgentFlowDebugMessage[] {
  if (items.length === 0) {
    return [];
  }

  return items.flatMap((item) =>
    mapConversationItemToMessages(item, outputState)
  );
}

function conversationSessionStatus(
  items: ApplicationRunConversationMessage[],
  outputState: ApplicationRunConversationOutputState | null
): AgentFlowDebugSessionStatus {
  const itemStatus =
    items.find((item) => isActiveRunStatus(item.status))?.status ??
    items.find((item) => isConversationRefreshableStatus(item.status))
      ?.status ??
    [...items].reverse().find((item) => item.is_current)?.status ??
    items.at(-1)?.status;

  return mapRunStatusToSessionStatus(
    itemStatus ?? outputState?.status ?? 'succeeded'
  );
}

function conversationRefreshable(
  items: ApplicationRunConversationMessage[],
  outputState: ApplicationRunConversationOutputState | null
) {
  // An empty page must not stop the refresh: the call status is the fact that
  // decides whether new messages can still appear.
  return (
    isConversationRefreshableStatus(outputState?.status) ||
    items.some((item) => isConversationRefreshableStatus(item.status))
  );
}

function conversationItemKey(item: ApplicationRunConversationMessage) {
  return item.message_id;
}

/// Order merged pages by the stream position the backend assigned. Items
/// without a position keep their relative order, which is the order the
/// per-run conversation summary returns them in.
function compareConversationItems(
  left: ApplicationRunConversationMessage,
  right: ApplicationRunConversationMessage
) {
  // A context belongs to its business turn, including across history pages.
  const timeOrder = left.started_at.localeCompare(right.started_at);
  if (timeOrder !== 0) return timeOrder;
  const leftContext = left.context_source ? 0 : 1;
  const rightContext = right.context_source ? 0 : 1;
  if (leftContext !== rightContext) {
    return leftContext - rightContext;
  }

  const leftSequence = left.sequence ?? null;
  const rightSequence = right.sequence ?? null;

  if (leftSequence !== null && rightSequence !== null) {
    return (
      leftSequence - rightSequence ||
      left.message_id.localeCompare(right.message_id)
    );
  }

  if (leftSequence !== null) {
    return -1;
  }

  if (rightSequence !== null) {
    return 1;
  }

  return 0;
}

/// Merge every page held for this run. A later source wins for the same message
/// id, so a refreshed item replaces the stale copy instead of being ignored.
function mergeConversationItems({
  latestPage,
  historyPages,
  caughtUpPages
}: {
  latestPage: ApplicationRunConversationMessagesPage | null;
  historyPages: ApplicationRunConversationMessagesPage[];
  caughtUpPages: ApplicationRunConversationMessagesPage[];
}): ApplicationRunConversationMessage[] {
  const byId = new Map<string, ApplicationRunConversationMessage>();

  for (const page of historyPages) {
    for (const item of page.items) {
      byId.set(conversationItemKey(item), item);
    }
  }
  for (const page of caughtUpPages) {
    for (const item of page.items) {
      byId.set(conversationItemKey(item), item);
    }
  }
  for (const item of latestPage?.items ?? []) {
    byId.set(conversationItemKey(item), item);
  }

  return [...byId.values()].sort(compareConversationItems);
}

function RunConversation({
  applicationId,
  requested_model_id,
  reasoning_effort,
  logConversationId,
  onClose,
  onOpenMessageLog,
  onOpenResumeTimeline,
  runId
}: {
  applicationId: string;
  requested_model_id?: string | null;
  reasoning_effort?: string | null;
  logConversationId?: string | null;
  onClose: () => void;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
  onOpenResumeTimeline?: (message: AgentFlowDebugMessage) => void;
  runId: string;
}) {
  const { token } = theme.useToken();
  const [conversationScope, setConversationScope] = useState(
    Boolean(logConversationId)
  );
  const [previousConversationPages, setPreviousConversationPages] = useState<
    ApplicationRunConversationMessagesPage[]
  >([]);
  const [caughtUpConversationPages, setCaughtUpConversationPages] = useState<
    ApplicationRunConversationMessagesPage[]
  >([]);
  const loadingPreviousConversationRef = useRef(false);
  const conversationScopeGeneration = useRef(0);
  const observedNewestCursorRef = useRef<string | null>(null);
  const initialConversationQuery = useQuery({
    queryKey:
      conversationScope && logConversationId
        ? applicationLogConversationMessagesQueryKey(
            applicationId,
            logConversationId,
            { limit: RUN_CONVERSATION_PAGE_LIMIT }
          )
        : applicationRunConversationMessagesQueryKey(applicationId, runId, {
            limit: RUN_CONVERSATION_PAGE_LIMIT
          }),
    queryFn: () =>
      conversationScope && logConversationId
        ? fetchApplicationLogConversationMessages(
            applicationId,
            logConversationId,
            { limit: RUN_CONVERSATION_PAGE_LIMIT }
          )
        : fetchApplicationRunConversationMessages(applicationId, runId, {
            limit: RUN_CONVERSATION_PAGE_LIMIT
          }),
    refetchOnMount: 'always',
    refetchOnWindowFocus: false
  });
  const refetchInitialConversation = initialConversationQuery.refetch;
  const latestConversationPage = initialConversationQuery.data ?? null;
  const conversationItems = useMemo(
    () =>
      mergeConversationItems({
        latestPage: latestConversationPage,
        historyPages: previousConversationPages,
        caughtUpPages: caughtUpConversationPages
      }),
    [
      latestConversationPage,
      previousConversationPages,
      caughtUpConversationPages
    ]
  );
  const outputState = latestConversationPage?.output_state ?? null;
  const messages = useMemo(
    () => buildConversationMessages(conversationItems, outputState),
    [conversationItems, outputState]
  );
  const refreshable = conversationRefreshable(conversationItems, outputState);
  // The scope switch is a conversation-level action: it belongs to the last
  // turn, so it appears once, at the end of the conversation.
  const lastConversationMessageId = useMemo(
    () =>
      [...messages].reverse().find((message) => message.role !== 'system')
        ?.id ?? null,
    [messages]
  );
  const newestCursor = latestConversationPage?.page.newest_cursor ?? null;

  useEffect(() => {
    if (!refreshable) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void refetchInitialConversation();
    }, ACTIVE_CONVERSATION_REFETCH_INTERVAL_MS);

    return () => window.clearInterval(intervalId);
  }, [refreshable, refetchInitialConversation]);

  // The newest page holds five business turns. When more than five arrived between two
  // refreshes, read forward from the previously observed position until the
  // backlog is drained, so the newest page never hides the items in between.
  useEffect(() => {
    const previousNewestCursor = observedNewestCursorRef.current;
    observedNewestCursorRef.current = newestCursor;

    if (
      !newestCursor ||
      !previousNewestCursor ||
      previousNewestCursor === newestCursor
    ) {
      return;
    }

    const generation = conversationScopeGeneration.current;
    let cancelled = false;

    void (async () => {
      const collected: ApplicationRunConversationMessagesPage[] = [];
      let after: string | null = previousNewestCursor;
      while (after && collected.length < RUN_CONVERSATION_CATCH_UP_PAGE_LIMIT) {
        const page: ApplicationRunConversationMessagesPage =
          conversationScope && logConversationId
            ? await fetchApplicationLogConversationMessages(
                applicationId,
                logConversationId,
                {
                  after,
                  limit: RUN_CONVERSATION_PAGE_LIMIT
                }
              )
            : await fetchApplicationRunConversationMessages(
                applicationId,
                runId,
                {
                  after,
                  limit: RUN_CONVERSATION_PAGE_LIMIT
                }
              );
        if (cancelled || generation !== conversationScopeGeneration.current) {
          return;
        }
        collected.push(page);
        after = page.page.has_after ? (page.page.after_cursor ?? null) : null;
      }
      if (!cancelled && collected.length > 0) {
        setCaughtUpConversationPages((current) => [...current, ...collected]);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    applicationId,
    conversationScope,
    logConversationId,
    newestCursor,
    runId
  ]);

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

  function resetConversationPages() {
    conversationScopeGeneration.current += 1;
    observedNewestCursorRef.current = null;
    setPreviousConversationPages([]);
    setCaughtUpConversationPages([]);
  }

  async function loadPreviousConversationPage() {
    // History is anchored at the oldest loaded page, so paging back never
    // re-reads a page the conversation already shows.
    const historyAnchor =
      previousConversationPages[0] ?? latestConversationPage;
    const before = historyAnchor?.page.before_cursor;

    if (
      loadingPreviousConversationRef.current ||
      !historyAnchor ||
      !historyAnchor.page.has_before ||
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
        assistantMessageActions={
          logConversationId
            ? (message) =>
                message.id !== lastConversationMessageId ? null : (
                  // The conversation scope switch lives in the message action
                  // row of the last turn, next to the call log and resume
                  // timeline actions.
                  <Tooltip
                    title={i18nText(
                      'applications',
                      conversationScope
                        ? 'auto.show_current_task'
                        : 'auto.show_log_conversation'
                    )}
                  >
                    <Button
                      aria-label={i18nText(
                        'applications',
                        conversationScope
                          ? 'auto.show_current_task'
                          : 'auto.show_log_conversation'
                      )}
                      icon={<MessageOutlined />}
                      size="small"
                      type={conversationScope ? 'default' : 'text'}
                      onClick={() => {
                        resetConversationPages();
                        setConversationScope((current) => !current);
                      }}
                    />
                  </Tooltip>
                )
            : undefined
        }
        composerUiOnly
        messages={messages}
        runContext={runConversationContext}
        showClearAction={false}
        showComposer={false}
        status={conversationSessionStatus(conversationItems, outputState)}
        stopping={false}
        subtitle={<RunIdSubtitle runId={runId} />}
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
        <div
          className="application-run-detail__model-summary"
          style={{
            borderRadius: token.borderRadius * 2,
            borderColor: token.colorBorder,
            boxShadow: token.boxShadowTertiary
          }}
        >
          <Tooltip title={i18nText('applications', 'auto.requested_model')}>
            <span className="application-run-detail__model-name">
              {requested_model_id || '—'}
            </span>
          </Tooltip>
          <Tooltip title={i18nText('applications', 'auto.reasoning_effort')}>
            <span className="application-run-detail__reasoning-effort">
              {reasoning_effort || '—'}
            </span>
          </Tooltip>
        </div>
    </div>
  );
}

export function ApplicationRunDetailPanel({
  applicationId,
  requested_model_id,
  reasoning_effort,
  logConversationId,
  onClose,
  onOpenMessageLog,
  onOpenResumeTimeline,
  runId
}: {
  applicationId: string;
  requested_model_id?: string | null;
  reasoning_effort?: string | null;
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
            key={`${runId}:${logConversationId ?? ''}`}
            applicationId={applicationId}
            requested_model_id={requested_model_id}
            reasoning_effort={reasoning_effort}
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
