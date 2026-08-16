import {
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  subscribeConsoleAssistantConversationsWebSocket,
  updateConsoleAssistantSettings,
  type ConsoleAssistantConversationPage,
  type ConsoleAssistantConversationSummary,
  type ConsoleAssistantPreference,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';
import {
  CheckOutlined,
  BranchesOutlined,
  ClockCircleOutlined,
  CloseOutlined,
  ExclamationCircleOutlined,
  HistoryOutlined,
  LoadingOutlined,
  PlusOutlined,
  SelectOutlined,
  SettingOutlined,
  WarningOutlined
} from '@ant-design/icons';
import { Conversations, Sender } from '@ant-design/x';
import {
  Button,
  Checkbox,
  Dropdown,
  Flex,
  Form,
  Modal,
  Progress,
  Select,
  Tooltip,
  type MenuProps
} from 'antd';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent
} from 'react';
import { createPortal } from 'react-dom';

import { useEmbeddedAssistantSession } from '../../hooks/useEmbeddedAssistantSession';
import {
  fetchRuntimeDebugArtifact,
  fetchRuntimeDebugArtifacts,
  type AgentFlowDebugMessage
} from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { getWindowWorkspaceViewport } from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';
import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole';
import { PageReferenceDraftRow } from '../debug-console/conversation/PageReferenceTag';
import { formatLlmTokenCount } from '../../lib/model-options';
import { useAssistantPageReferenceSelection } from './useAssistantPageReferenceSelection';
import {
  AssistantRunNodePanel,
  AssistantRunTimeline
} from './AssistantRunActivityPanel';
import '../editor/styles/shell.css';
import './embedded-assistant.css';

function hasChangedPreference(
  current: ConsoleAssistantPreference | undefined,
  next: ConsoleAssistantPreference
) {
  if (!current || current.application_id !== next.application_id) {
    return true;
  }
  return (
    current.mcp_instance_ids.join('\u0000') !==
      next.mcp_instance_ids.join('\u0000') ||
    current.enabled_client_tools.join('\u0000') !==
      next.enabled_client_tools.join('\u0000')
  );
}

const ASSISTANT_WINDOW_ID = 'embedded-agent-assistant-preview';
const ASSISTANT_HISTORY_DEFAULT_WIDTH = 280;
const ASSISTANT_HISTORY_MIN_WIDTH = 180;
const ASSISTANT_CONVERSATION_MIN_WIDTH = 220;
const ASSISTANT_HISTORY_RESIZE_WIDTH = 12;

type AssistantHistoryExpansionSide = 'left' | 'right';

function assistantConversationKey(item: {
  conversation_id: string | null;
  legacy_flow_run_id: string | null;
}) {
  return item.conversation_id
    ? `conversation:${item.conversation_id}`
    : `legacy:${item.legacy_flow_run_id}`;
}

function assistantConversationGroup(updatedAt: string) {
  const date = new Date(updatedAt);
  return Number.isNaN(date.getTime())
    ? i18nText('appShell', 'auto.assistant_history')
    : new Intl.DateTimeFormat(undefined, {
        month: 'short',
        day: 'numeric'
      }).format(date);
}

function upsertAssistantConversation(
  page: ConsoleAssistantConversationPage,
  item: ConsoleAssistantConversationSummary,
  eventType: 'conversation.created' | 'conversation.updated'
): ConsoleAssistantConversationPage {
  const itemKey = assistantConversationKey(item);
  const exists = page.items.some(
    (candidate) => assistantConversationKey(candidate) === itemKey
  );
  const items = [
    item,
    ...page.items.filter(
      (candidate) => assistantConversationKey(candidate) !== itemKey
    )
  ].sort((left, right) => {
    const updatedAtOrder = right.updated_at.localeCompare(left.updated_at);
    return updatedAtOrder !== 0
      ? updatedAtOrder
      : assistantConversationKey(right).localeCompare(
          assistantConversationKey(left)
        );
  });
  return {
    ...page,
    items,
    total:
      !exists && eventType === 'conversation.created'
        ? page.total + 1
        : page.total
  };
}

function assistantRunStatusIndicator(status: string | null) {
  const commonProps = {
    'data-assistant-run-status': status ?? undefined
  };
  if (['queued', 'running', 'waiting_callback'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_running');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--running"
        >
          <LoadingOutlined spin />
        </span>
      </Tooltip>
    );
  }
  if (['waiting_human', 'paused'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_waiting');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--waiting"
        >
          <ClockCircleOutlined />
        </span>
      </Tooltip>
    );
  }
  if (['failed', 'incomplete'].includes(status ?? '')) {
    const label = i18nText('appShell', 'auto.assistant_status_failed');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--failed"
        >
          <ExclamationCircleOutlined />
        </span>
      </Tooltip>
    );
  }
  if (status === 'cancelled') {
    const label = i18nText('appShell', 'auto.assistant_status_cancelled');
    return (
      <Tooltip title={label}>
        <span
          {...commonProps}
          aria-label={label}
          className="embedded-agent-assistant-preview__history-item-status embedded-agent-assistant-preview__history-item-status--cancelled"
        >
          <CloseOutlined />
        </span>
      </Tooltip>
    );
  }
  return null;
}

function initialAssistantWindowRect(): WindowWorkspaceRect {
  const viewport = getWindowWorkspaceViewport();
  const width = Math.min(560, Math.max(400, viewport.width - 32));
  return {
    left: Math.max(8, viewport.left + viewport.width - width - 16),
    top: Math.max(viewport.top + 8, 56),
    width,
    height: Math.min(Math.max(480, viewport.height - 24), viewport.height - 16)
  };
}

export function EmbeddedAgentAssistantPreview({
  open,
  pageKey,
  onClose,
  clientTools
}: {
  open: boolean;
  pageKey: string;
  onClose: () => void;
  clientTools?: import('@1flowbase/api-client').ConsoleAssistantClientTools;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const [settings, setSettings] = useState<ConsoleAssistantSettings | null>(
    null
  );
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [activityMessageId, setActivityMessageId] = useState<string | null>(
    null
  );
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyPage, setHistoryPage] =
    useState<ConsoleAssistantConversationPage | null>(null);
  const [saving, setSaving] = useState(false);
  const [mobile, setMobile] = useState(false);
  const [historyWidth, setHistoryWidth] = useState(
    ASSISTANT_HISTORY_DEFAULT_WIDTH
  );
  const [isResizingHistory, setIsResizingHistory] = useState(false);
  const [historyFullView, setHistoryFullView] = useState(false);
  const historyResizeCleanupRef = useRef<(() => void) | null>(null);
  const historySubscriptionRef = useRef<AbortController | null>(null);
  const historyExpansionSideRef = useRef<AssistantHistoryExpansionSide | null>(
    null
  );
  const historyExpansionWidthRef = useRef(0);
  const historyLayoutRef = useRef<HTMLDivElement | null>(null);
  const historyWidthRef = useRef(historyWidth);
  const assistantWindowRectRef = useRef<WindowWorkspaceRect | null>(null);
  const assistantOpenRef = useRef(open);
  const [form] = Form.useForm<ConsoleAssistantPreference>();
  const {
    activate,
    close,
    open: openWindow,
    setRect,
    state: windowWorkspaceState,
    toggleMaximized
  } = useWindowWorkspace();
  const session = useEmbeddedAssistantSession(
    settings?.preference.application_id ?? null,
    clientTools
  );
  const activityMessage =
    session.messages.find(
      (message) =>
        message.id === activityMessageId && message.role === 'assistant'
    ) ?? null;
  const latestRunMessage =
    [...session.messages]
      .reverse()
      .find((message) => message.role === 'assistant' && message.runId) ?? null;
  const sidePanelOpen = historyOpen || activityMessage !== null;

  useEffect(() => {
    const wasOpen = assistantOpenRef.current;
    assistantOpenRef.current = open;
    if (wasOpen === open) {
      return;
    }
    if (open) {
      session.resumeActiveRun();
    } else {
      session.disconnectSession();
    }
  }, [open, session.disconnectSession, session.resumeActiveRun]);
  const pageReferenceSelection = useAssistantPageReferenceSelection({
    active: open,
    duplicateMessage: i18nText(
      'appShell',
      'auto.assistant_page_reference_duplicate'
    ),
    maxBytes: settings?.page_reference_max_bytes ?? 0,
    maxCount: settings?.page_reference_max_count ?? 0,
    maxTotalBytes: settings?.page_reference_max_total_bytes ?? 0,
    pageKey: `${workspaceId ?? ''}:${pageKey}`,
    selectionHint: i18nText(
      'appShell',
      'auto.assistant_page_reference_selection_hint'
    ),
    tooManyMessage: useCallback(
      (maxCount: number) =>
        i18nText('appShell', 'auto.assistant_page_reference_too_many', {
          value1: maxCount
        }),
      []
    ),
    tooLargeMessage: useCallback(
      (actualBytes: number, maxBytes: number) =>
        i18nText('appShell', 'auto.assistant_page_reference_too_large', {
          value1: actualBytes,
          value2: maxBytes
        }),
      []
    ),
    totalTooLargeMessage: useCallback(
      (actualBytes: number, maxBytes: number) =>
        i18nText('appShell', 'auto.assistant_page_reference_total_too_large', {
          value1: actualBytes,
          value2: maxBytes
        }),
      []
    ),
    unsupportedIsolatedFrameMessage: i18nText(
      'appShell',
      'auto.assistant_page_reference_isolated_frame_unsupported'
    )
  });

  useEffect(() => {
    setSettings(null);
    setSettingsOpen(false);
    setHistoryOpen(false);
    setActivityMessageId(null);
    setHistoryFullView(false);
    historyExpansionSideRef.current = null;
    historyExpansionWidthRef.current = 0;
    setHistoryPage(null);
  }, [workspaceId]);

  useEffect(() => {
    if (!open) {
      historyExpansionSideRef.current = null;
      historyExpansionWidthRef.current = 0;
      close(ASSISTANT_WINDOW_ID);
      return;
    }
    openWindow({
      id: ASSISTANT_WINDOW_ID,
      owner: 'embedded-agent-assistant',
      parent_id: null,
      rect: initialAssistantWindowRect(),
      dirty: false
    });
    return () => close(ASSISTANT_WINDOW_ID);
  }, [close, open, openWindow]);

  useEffect(() => {
    const updateMobile = () => setMobile(window.innerWidth <= 640);
    updateMobile();
    window.addEventListener('resize', updateMobile);
    return () => window.removeEventListener('resize', updateMobile);
  }, []);

  useEffect(() => {
    if (!open || settings) {
      return;
    }
    let disposed = false;
    void getConsoleAssistantSettings()
      .then((nextSettings) => {
        if (disposed) {
          return;
        }
        setSettings(nextSettings);
      })
      .catch(() => {
        if (!disposed) {
          setSettings({
            preference: {
              application_id: null,
              mcp_instance_ids: [],
              enabled_client_tools: [
                'get_client_context',
                'refresh_client_view'
              ]
            },
            published_agent_flows: [],
            enabled_mcp_instances: [],
            page_reference_max_bytes: 0,
            page_reference_max_count: 0,
            page_reference_max_total_bytes: 0,
            run_capabilities: {
              model_selection_enabled: false,
              reasoning_effort_enabled: false,
              models: []
            }
          });
        }
      });
    return () => {
      disposed = true;
    };
  }, [form, open, settings]);

  useEffect(() => {
    if (!settings || !settingsOpen) {
      return;
    }
    form.setFieldsValue(settings.preference);
  }, [form, settings, settingsOpen]);

  const selectedFlow = settings?.published_agent_flows.find(
    (flow) => flow.application_id === settings.preference.application_id
  );
  const applicationId = settings?.preference.application_id ?? null;
  const renderAssistantMessageMain = useCallback(
    (message: AgentFlowDebugMessage) =>
      applicationId && message.presentation === 'answer' ? (
        <AssistantRunTimeline applicationId={applicationId} message={message} />
      ) : undefined,
    [applicationId]
  );

  const loadHistory = useCallback(
    async (page = 1) => {
      if (!applicationId) {
        return;
      }
      setHistoryLoading(true);
      try {
        const next = await listConsoleAssistantConversations(applicationId, {
          page,
          pageSize: 20
        });
        setHistoryPage((current) =>
          page === 1
            ? next
            : {
                ...next,
                items: [...(current?.items ?? []), ...next.items]
              }
        );
      } finally {
        setHistoryLoading(false);
      }
    },
    [applicationId]
  );

  useEffect(() => {
    historySubscriptionRef.current?.abort();
    historySubscriptionRef.current = null;
    if (!historyOpen || !applicationId || !csrfToken) {
      return;
    }
    setHistoryPage(null);
    setHistoryLoading(true);
    let disposed = false;
    void subscribeConsoleAssistantConversationsWebSocket(
      applicationId,
      csrfToken,
      {
        getAbortController: (controller) => {
          if (disposed) {
            controller.abort();
            return;
          }
          historySubscriptionRef.current = controller;
        },
        onSnapshot: (page) => {
          if (!disposed) {
            setHistoryPage(page);
            setHistoryLoading(false);
          }
        },
        onConversation: (item, eventType) => {
          if (!disposed) {
            setHistoryPage((current) =>
              current
                ? upsertAssistantConversation(current, item, eventType)
                : current
            );
          }
        }
      }
    ).catch(() => {
      if (!disposed) {
        setHistoryLoading(false);
        void loadHistory(1);
      }
    });
    return () => {
      disposed = true;
      historySubscriptionRef.current?.abort();
      historySubscriptionRef.current = null;
    };
  }, [applicationId, csrfToken, historyOpen, loadHistory]);
  const selectedModel =
    settings?.run_capabilities.models.find(
      (model) => model.id === settings.preference.model
    ) ?? settings?.run_capabilities.models[0];
  const selectedReasoningEffort =
    settings?.preference.reasoning_effort ??
    selectedModel?.default_reasoning_effort ??
    selectedModel?.reasoning_efforts[0];
  const contextWindow =
    session.contextSnapshot?.effective_context_window ??
    selectedModel?.context_window ??
    null;
  const contextTokenUsage = session.contextSnapshot?.input_tokens ?? null;
  const measuredContextTokenUsage = contextTokenUsage ?? 0;
  const contextUsagePercent =
    contextWindow && contextWindow > 0
      ? Math.min(
          100,
          Math.round((measuredContextTokenUsage / contextWindow) * 1000) / 10
        )
      : 0;
  const contextVisualPercent =
    measuredContextTokenUsage > 0 ? Math.max(1, contextUsagePercent) : 0;
  const remainingContextPercent = Math.max(
    0,
    Math.round((100 - contextUsagePercent) * 10) / 10
  );
  const windowEntry = windowWorkspaceState.windows.find(
    (entry) => entry.id === ASSISTANT_WINDOW_ID
  );
  assistantWindowRectRef.current = windowEntry?.rect ?? null;
  historyWidthRef.current = historyWidth;
  const assistantWindowZIndex = 1050 + (windowEntry?.z_index ?? 0);
  const assistantSettingsModalZIndex = 1100 + (windowEntry?.z_index ?? 0);

  function assistantHistoryOccupiedWidth() {
    return historyWidthRef.current + ASSISTANT_HISTORY_RESIZE_WIDTH;
  }

  function assistantHistoryMaxWidth() {
    const rect = assistantWindowRectRef.current;
    if (!rect) {
      return historyWidthRef.current;
    }
    return Math.max(
      ASSISTANT_HISTORY_MIN_WIDTH,
      rect.width -
        ASSISTANT_HISTORY_RESIZE_WIDTH -
        ASSISTANT_CONVERSATION_MIN_WIDTH
    );
  }

  function expandSidePanel() {
    const rect = assistantWindowRectRef.current;
    if (!rect || mobile) {
      historyExpansionSideRef.current = null;
      historyExpansionWidthRef.current = 0;
      setHistoryFullView(true);
      return;
    }
    const viewport = getWindowWorkspaceViewport();
    const occupiedWidth = assistantHistoryOccupiedWidth();
    const leftSpace = rect.left - (viewport.left + 8);
    const rightSpace =
      viewport.left + viewport.width - 8 - (rect.left + rect.width);
    const side: AssistantHistoryExpansionSide | null =
      leftSpace >= occupiedWidth
        ? 'left'
        : rightSpace >= occupiedWidth
          ? 'right'
          : null;

    if (!side) {
      historyExpansionSideRef.current = null;
      historyExpansionWidthRef.current = 0;
      setHistoryFullView(true);
      return;
    }

    const nextRect =
      side === 'left'
        ? {
            ...rect,
            left: rect.left - occupiedWidth,
            width: rect.width + occupiedWidth
          }
        : { ...rect, width: rect.width + occupiedWidth };
    historyExpansionSideRef.current = side;
    historyExpansionWidthRef.current = occupiedWidth;
    assistantWindowRectRef.current = nextRect;
    setRect(ASSISTANT_WINDOW_ID, nextRect);
    setHistoryFullView(false);
  }

  function openHistory() {
    if (!sidePanelOpen) {
      expandSidePanel();
    }
    setActivityMessageId(null);
    setHistoryOpen(true);
  }

  function toggleHistory() {
    if (historyOpen) {
      closeHistory();
      return;
    }
    openHistory();
  }

  function collapseSidePanel() {
    const side = historyExpansionSideRef.current;
    const rect = assistantWindowRectRef.current;
    if (side && rect) {
      const expansionWidth = historyExpansionWidthRef.current;
      const nextWidth = Math.max(400, rect.width - expansionWidth);
      const removedWidth = rect.width - nextWidth;
      const nextRect =
        side === 'left'
          ? { ...rect, left: rect.left + removedWidth, width: nextWidth }
          : { ...rect, width: nextWidth };
      assistantWindowRectRef.current = nextRect;
      setRect(ASSISTANT_WINDOW_ID, nextRect);
    }
    historyExpansionSideRef.current = null;
    historyExpansionWidthRef.current = 0;
    setHistoryFullView(false);
  }

  function closeHistory() {
    collapseSidePanel();
    setHistoryOpen(false);
  }

  function openActivity(messageId: string) {
    if (!sidePanelOpen) {
      expandSidePanel();
    }
    setHistoryOpen(false);
    setActivityMessageId(messageId);
  }

  function closeActivity() {
    collapseSidePanel();
    setActivityMessageId(null);
  }
  const runtimePreferenceMenuItems: MenuProps['items'] = settings
    ? [
        {
          key: 'model',
          label: (
            <span className="embedded-agent-assistant-preview__runtime-menu-row">
              <span>{i18nText('appShell', 'auto.assistant_model')}</span>
              <span className="embedded-agent-assistant-preview__runtime-menu-value">
                {selectedModel?.name ?? selectedModel?.id ?? '-'}
              </span>
            </span>
          ),
          children: settings.run_capabilities.models.map((model) => ({
            key: `model:${model.id}`,
            label: (
              <span className="embedded-agent-assistant-preview__runtime-menu-option">
                <span>{model.name ?? model.id}</span>
                {model.id === selectedModel?.id ? <CheckOutlined /> : null}
              </span>
            )
          }))
        },
        ...(settings.run_capabilities.reasoning_effort_enabled &&
        selectedModel?.reasoning_efforts.length
          ? [
              {
                key: 'reasoning-effort',
                label: (
                  <span className="embedded-agent-assistant-preview__runtime-menu-row">
                    <span>
                      {i18nText('appShell', 'auto.assistant_reasoning_effort')}
                    </span>
                    <span className="embedded-agent-assistant-preview__runtime-menu-value">
                      {selectedReasoningEffort ?? '-'}
                    </span>
                  </span>
                ),
                children: selectedModel.reasoning_efforts.map((effort) => ({
                  key: `reasoning-effort:${effort}`,
                  label: (
                    <span className="embedded-agent-assistant-preview__runtime-menu-option">
                      <span>{effort}</span>
                      {effort === selectedReasoningEffort ? (
                        <CheckOutlined />
                      ) : null}
                    </span>
                  )
                }))
              }
            ]
          : []),
        { type: 'divider' },
        {
          key: 'reset-defaults',
          label: i18nText('appShell', 'auto.assistant_reset_defaults')
        }
      ]
    : [];

  useEffect(() => {
    if (mobile && windowEntry && !windowEntry.maximized) {
      toggleMaximized(ASSISTANT_WINDOW_ID, getWindowWorkspaceViewport());
    }
  }, [mobile, toggleMaximized, windowEntry]);

  useEffect(() => {
    if (!sidePanelOpen) {
      return;
    }
    const rect = assistantWindowRectRef.current;
    const chatWidth = rect ? rect.width - assistantHistoryOccupiedWidth() : 0;
    if (
      !mobile &&
      !historyFullView &&
      chatWidth >= ASSISTANT_CONVERSATION_MIN_WIDTH
    ) {
      return;
    }
    historyExpansionSideRef.current = null;
    historyExpansionWidthRef.current = 0;
    setHistoryFullView(true);
  }, [historyFullView, mobile, sidePanelOpen, windowEntry?.rect.width]);

  useEffect(() => () => historyResizeCleanupRef.current?.(), []);

  function startHistoryResize(event: ReactMouseEvent<HTMLDivElement>) {
    if (mobile) {
      return;
    }
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = historyWidth;
    const maxWidth = assistantHistoryMaxWidth();
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    historyResizeCleanupRef.current?.();
    setIsResizingHistory(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const onMouseMove = (moveEvent: MouseEvent) => {
      const nextWidth = Math.min(
        Math.max(
          ASSISTANT_HISTORY_MIN_WIDTH,
          startWidth + moveEvent.clientX - startX
        ),
        maxWidth
      );
      const currentWidth = historyWidthRef.current;
      if (nextWidth === currentWidth) {
        return;
      }
      historyWidthRef.current = nextWidth;
      setHistoryWidth(nextWidth);
    };
    const cleanup = () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', cleanup);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      historyResizeCleanupRef.current = null;
      setIsResizingHistory(false);
    };

    historyResizeCleanupRef.current = cleanup;
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', cleanup);
  }

  async function saveSettings() {
    if (!csrfToken) {
      return;
    }
    const preference = await form.validateFields();
    setSaving(true);
    try {
      const nextSettings = await updateConsoleAssistantSettings(
        preference,
        csrfToken
      );
      const changed = hasChangedPreference(settings?.preference, preference);
      setSettings(nextSettings);
      if (changed) {
        session.clearSession();
      }
      setSettingsOpen(false);
    } finally {
      setSaving(false);
    }
  }

  async function updateRuntimePreference(
    patch: Pick<ConsoleAssistantPreference, 'model' | 'reasoning_effort'>
  ) {
    if (!csrfToken || !settings) {
      return;
    }
    setSaving(true);
    try {
      setSettings(
        await updateConsoleAssistantSettings(
          { ...settings.preference, ...patch },
          csrfToken
        )
      );
    } finally {
      setSaving(false);
    }
  }

  if (typeof document === 'undefined') {
    return null;
  }

  return createPortal(
    <>
      {open && windowEntry ? (
        <WindowWorkspaceWindow
          active={
            windowEntry.z_index ===
            Math.max(
              ...windowWorkspaceState.windows.map((entry) => entry.z_index)
            )
          }
          bodyClassName="embedded-agent-assistant-preview__body"
          className="embedded-agent-assistant-preview"
          dragHandleSelector=".agent-flow-editor__dock-panel-header"
          initialRect={() => windowEntry.rect}
          minHeight={320}
          minWidth={400}
          rect={windowEntry.rect}
          resizeEdges={['left', 'right', 'bottom']}
          resizeLabel={(edge) =>
            `${i18nText('appShell', 'auto.assistant')} ${edge}`
          }
          testId={ASSISTANT_WINDOW_ID}
          title={i18nText('appShell', 'auto.assistant')}
          zIndex={assistantWindowZIndex}
          onActivate={() => activate(ASSISTANT_WINDOW_ID)}
          onRectChange={(rect) => {
            assistantWindowRectRef.current = rect;
            setRect(ASSISTANT_WINDOW_ID, rect);
          }}
        >
          <div
            className="embedded-agent-assistant-preview__layout"
            data-history-full={historyFullView}
            data-history-open={sidePanelOpen}
            ref={historyLayoutRef}
          >
            {sidePanelOpen ? (
              <aside
                aria-label={
                  activityMessage
                    ? i18nText('appShell', 'auto.assistant_activity')
                    : i18nText('appShell', 'auto.assistant_history')
                }
                className="embedded-agent-assistant-preview__history"
                data-resizing={isResizingHistory ? 'true' : 'false'}
                data-testid="embedded-agent-assistant-history"
                style={
                  mobile || historyFullView
                    ? undefined
                    : { width: `${historyWidth}px` }
                }
              >
                <div className="embedded-agent-assistant-preview__history-content">
                  <div className="embedded-agent-assistant-preview__history-header">
                    <span className="embedded-agent-assistant-preview__history-title">
                      {activityMessage
                        ? i18nText('appShell', 'auto.assistant_activity')
                        : i18nText('appShell', 'auto.assistant_history')}
                    </span>
                    <Button
                      aria-label={i18nText('agentFlow', 'auto.close', {
                        value1: activityMessage
                          ? i18nText('appShell', 'auto.assistant_activity')
                          : i18nText('appShell', 'auto.assistant_history')
                      })}
                      icon={<CloseOutlined />}
                      size="small"
                      type="text"
                      onClick={activityMessage ? closeActivity : closeHistory}
                    />
                  </div>
                  <div className="embedded-agent-assistant-preview__history-body">
                    {activityMessage && applicationId ? (
                      <AssistantRunNodePanel
                        applicationId={applicationId}
                        message={activityMessage}
                      />
                    ) : (
                      <>
                        <Conversations
                          activeKey={
                            session.conversationId
                              ? `conversation:${session.conversationId}`
                              : session.legacyFlowRunId
                                ? `legacy:${session.legacyFlowRunId}`
                                : undefined
                          }
                          creation={{
                            align: 'start',
                            disabled: !session.canChangeConversation,
                            icon: <PlusOutlined />,
                            label: i18nText(
                              'appShell',
                              'auto.assistant_new_conversation'
                            ),
                            onClick: () => {
                              void session
                                .startNewConversation()
                                .then((created) => {
                                  if (created) {
                                    closeHistory();
                                  }
                                });
                            }
                          }}
                          groupable
                          items={historyPage?.items.map((item) => ({
                            disabled:
                              historyLoading || session.restoringHistory,
                            group: assistantConversationGroup(item.updated_at),
                            key: assistantConversationKey(item),
                            label: (
                              <span className="embedded-agent-assistant-preview__history-item">
                                <span className="embedded-agent-assistant-preview__history-item-title">
                                  {item.title ??
                                    (item.conversation_id
                                      ? i18nText(
                                          'appShell',
                                          'auto.assistant_new_conversation'
                                        )
                                      : i18nText(
                                          'appShell',
                                          'auto.assistant_legacy_snapshot'
                                        ))}
                                </span>
                                {assistantRunStatusIndicator(
                                  item.latest_flow_run_status
                                )}
                              </span>
                            )
                          }))}
                          onActiveChange={(key) => {
                            const item = historyPage?.items.find(
                              (candidate) =>
                                assistantConversationKey(candidate) === key
                            );
                            if (!item) {
                              return;
                            }
                            void session
                              .restoreConversation({
                                conversationId: item.conversation_id,
                                legacyFlowRunId: item.legacy_flow_run_id,
                                latestFlowRunId: item.latest_flow_run_id,
                                latestFlowRunStatus: item.latest_flow_run_status
                              })
                              .then((restored) => {
                                if (restored && item.conversation_id) {
                                  closeHistory();
                                }
                              });
                          }}
                        />
                        {session.legacyFlowRunId ? (
                          <Button
                            block
                            disabled={!session.canChangeConversation}
                            onClick={() => {
                              void session
                                .startNewConversation(
                                  session.legacyFlowRunId ?? undefined
                                )
                                .then((created) => {
                                  if (created) {
                                    closeHistory();
                                  }
                                });
                            }}
                          >
                            {i18nText(
                              'appShell',
                              'auto.assistant_continue_legacy_snapshot'
                            )}
                          </Button>
                        ) : null}
                        {historyPage &&
                        historyPage.items.length < historyPage.total ? (
                          <Button
                            block
                            disabled={historyLoading}
                            onClick={() =>
                              void loadHistory(historyPage.page + 1)
                            }
                          >
                            {i18nText('appShell', 'auto.assistant_load_more')}
                          </Button>
                        ) : null}
                      </>
                    )}
                  </div>
                </div>
                <div
                  aria-label={`${i18nText(
                    'appShell',
                    'auto.assistant_history'
                  )} width`}
                  aria-orientation="vertical"
                  className="embedded-agent-assistant-preview__history-resize"
                  data-testid="embedded-agent-assistant-history-resize"
                  hidden={mobile || historyFullView}
                  role="separator"
                  onMouseDown={startHistoryResize}
                />
              </aside>
            ) : null}
            <div
              className="embedded-agent-assistant-preview__conversation"
              hidden={(mobile || historyFullView) && sidePanelOpen}
            >
              <AgentFlowDebugConsole
                assistantMessageMainRender={renderAssistantMessageMain}
                clearDisabled={!session.canEditCurrentConversation}
                composerHeader={
                  pageReferenceSelection.references.length > 0 ||
                  pageReferenceSelection.error ? (
                    <div>
                      {pageReferenceSelection.references.map(
                        (reference, index) => (
                          <PageReferenceDraftRow
                            key={`${reference.page_url}:${reference.outer_html}`}
                            reference={reference}
                            removeLabel={i18nText(
                              'appShell',
                              'auto.assistant_remove_page_reference'
                            )}
                            onRemove={() =>
                              pageReferenceSelection.removeReference(index)
                            }
                          />
                        )
                      )}
                      {pageReferenceSelection.error ? (
                        <div
                          className="embedded-agent-assistant-preview__page-reference-error"
                          role="alert"
                        >
                          <WarningOutlined />
                          <span>{pageReferenceSelection.error}</span>
                        </div>
                      ) : null}
                    </div>
                  ) : undefined
                }
                composerFooterActions={
                  <Flex
                    align="center"
                    className="embedded-agent-assistant-preview__composer-actions"
                    gap={8}
                    justify="space-between"
                  >
                    <Flex align="center" gap={4}>
                      <Tooltip
                        title={i18nText(
                          'appShell',
                          'auto.assistant_select_page_content'
                        )}
                      >
                        <Button
                          aria-label={i18nText(
                            'appShell',
                            'auto.assistant_select_page_content'
                          )}
                          disabled={
                            !settings || !session.canEditCurrentConversation
                          }
                          icon={<SelectOutlined />}
                          size="small"
                          type={
                            pageReferenceSelection.selecting
                              ? 'primary'
                              : 'text'
                          }
                          onClick={
                            pageReferenceSelection.selecting
                              ? pageReferenceSelection.cancelSelection
                              : pageReferenceSelection.startSelection
                          }
                        />
                      </Tooltip>
                    </Flex>
                    {settings?.run_capabilities.model_selection_enabled ? (
                      <Flex align="center" gap={8}>
                        {contextWindow ? (
                          <Tooltip
                            color="#ffffff"
                            styles={{
                              container: {
                                border: '1px solid var(--border-subtle)',
                                borderRadius: '0.5rem',
                                boxShadow: 'var(--shadow-float)',
                                padding: '0.5rem 0.625rem'
                              }
                            }}
                            title={
                              <span className="embedded-agent-assistant-preview__context-tooltip">
                                <span>
                                  {i18nText(
                                    'appShell',
                                    'auto.assistant_context_remaining_percent',
                                    {
                                      value1: remainingContextPercent
                                    }
                                  )}
                                </span>
                                <span className="embedded-agent-assistant-preview__context-tooltip-total">
                                  {i18nText(
                                    'appShell',
                                    'auto.assistant_context_total',
                                    {
                                      value2:
                                        formatLlmTokenCount(contextWindow) ??
                                        '0',
                                      value1:
                                        formatLlmTokenCount(
                                          contextTokenUsage
                                        ) ?? '0'
                                    }
                                  )}
                                </span>
                              </span>
                            }
                          >
                            <span className="embedded-agent-assistant-preview__context-progress">
                              <Progress
                                percent={contextVisualPercent}
                                showInfo={false}
                                size={18}
                                trailColor="var(--border-default)"
                                type="circle"
                              />
                            </span>
                          </Tooltip>
                        ) : null}
                        <Dropdown
                          overlayStyle={{ zIndex: 1100 + windowEntry.z_index }}
                          placement="topLeft"
                          trigger={['click']}
                          menu={{
                            items: runtimePreferenceMenuItems,
                            onClick: ({ key }) => {
                              const selection = String(key);
                              if (selection === 'reset-defaults') {
                                void updateRuntimePreference({
                                  model: null,
                                  reasoning_effort: null
                                });
                                return;
                              }
                              if (selection.startsWith('model:')) {
                                const modelId = selection.slice(
                                  'model:'.length
                                );
                                const model =
                                  settings.run_capabilities.models.find(
                                    (candidate) => candidate.id === modelId
                                  );
                                if (model) {
                                  void updateRuntimePreference({
                                    model: model.id,
                                    reasoning_effort:
                                      model.default_reasoning_effort ?? null
                                  });
                                }
                                return;
                              }
                              if (selection.startsWith('reasoning-effort:')) {
                                const reasoning_effort = selection.slice(
                                  'reasoning-effort:'.length
                                );
                                if (
                                  selectedModel?.reasoning_efforts.includes(
                                    reasoning_effort
                                  )
                                ) {
                                  void updateRuntimePreference({
                                    model: selectedModel.id,
                                    reasoning_effort
                                  });
                                }
                              }
                            }
                          }}
                        >
                          <Sender.Switch
                            rootClassName="embedded-agent-assistant-preview__runtime-preferences"
                            value={false}
                          >
                            <span>
                              {selectedModel?.name ?? selectedModel?.id ?? '-'}
                            </span>
                            {settings.run_capabilities
                              .reasoning_effort_enabled &&
                            selectedReasoningEffort ? (
                              <span className="embedded-agent-assistant-preview__runtime-preferences-effort">
                                {selectedReasoningEffort}
                              </span>
                            ) : null}
                          </Sender.Switch>
                        </Dropdown>
                      </Flex>
                    ) : null}
                  </Flex>
                }
                headerActions={
                  <>
                    <Button
                      aria-label={i18nText(
                        'appShell',
                        'auto.assistant_activity'
                      )}
                      disabled={!latestRunMessage}
                      icon={<BranchesOutlined />}
                      size="small"
                      type="text"
                      onClick={() => {
                        if (latestRunMessage) {
                          openActivity(latestRunMessage.id);
                        }
                      }}
                    />
                    <Button
                      aria-label={i18nText(
                        'appShell',
                        'auto.assistant_history'
                      )}
                      disabled={!settings}
                      icon={<HistoryOutlined />}
                      size="small"
                      type="text"
                      onClick={toggleHistory}
                    />
                    <Button
                      aria-label={i18nText(
                        'appShell',
                        'auto.assistant_settings'
                      )}
                      disabled={!settings}
                      loading={!settings}
                      size="small"
                      type="text"
                      icon={<SettingOutlined />}
                      onClick={() => setSettingsOpen(true)}
                    />
                  </>
                }
                messages={session.messages}
                runContext={session.runContext}
                status={session.status}
                stopping={session.stopping}
                subtitle={selectedFlow?.name}
                title={i18nText('appShell', 'auto.assistant')}
                onChangeRunContextValue={session.setRunContextValue}
                onClearSession={session.clearSession}
                onClose={() => {
                  onClose();
                }}
                onLoadArtifact={
                  applicationId
                    ? (artifactRef) =>
                        fetchRuntimeDebugArtifact(applicationId, artifactRef)
                    : undefined
                }
                onLoadArtifacts={
                  applicationId
                    ? (artifactRefs) =>
                        fetchRuntimeDebugArtifacts(applicationId, artifactRefs)
                    : undefined
                }
                onOpenMessageLog={(message) => openActivity(message.id)}
                onStopRun={() => {
                  void session.stopRun();
                }}
                onSubmitPrompt={(prompt) => {
                  const pageReferences = pageReferenceSelection.references;
                  pageReferenceSelection.clearReferences();
                  void session.submitPrompt(prompt, pageReferences);
                }}
              />
            </div>
          </div>
        </WindowWorkspaceWindow>
      ) : null}
      <Modal
        confirmLoading={saving}
        open={open && settingsOpen}
        title={i18nText('appShell', 'auto.assistant_settings')}
        zIndex={assistantSettingsModalZIndex}
        onCancel={() => setSettingsOpen(false)}
        onOk={() => void saveSettings()}
      >
        <Form form={form} layout="vertical">
          <Form.Item
            label={i18nText('appShell', 'auto.assistant_flow')}
            name="application_id"
            rules={[{ required: true }]}
          >
            <Select
              allowClear
              options={
                settings?.published_agent_flows.map((flow) => ({
                  value: flow.application_id,
                  label: flow.name
                })) ?? []
              }
              onChange={(applicationId) => {
                if (applicationId !== settings?.preference.application_id) {
                  form.setFieldsValue({ model: null, reasoning_effort: null });
                }
              }}
            />
          </Form.Item>
          <Form.Item
            label={i18nText('appShell', 'auto.assistant_mcp')}
            name="mcp_instance_ids"
          >
            <Select
              mode="multiple"
              options={
                settings?.enabled_mcp_instances.map((instance) => ({
                  value: instance.instance_id,
                  label: instance.name
                })) ?? []
              }
            />
          </Form.Item>
          <Form.Item
            label={i18nText('appShell', 'auto.assistant_client_tools')}
            name="enabled_client_tools"
          >
            <Checkbox.Group
              options={[
                {
                  value: 'get_client_context',
                  label: i18nText(
                    'appShell',
                    'auto.assistant_client_context_tool'
                  )
                },
                {
                  value: 'refresh_client_view',
                  label: i18nText(
                    'appShell',
                    'auto.assistant_client_refresh_tool'
                  )
                }
              ]}
            />
          </Form.Item>
          <Button
            disabled={!settings || saving}
            onClick={() =>
              void updateRuntimePreference({
                model: null,
                reasoning_effort: null
              })
            }
          >
            {i18nText('appShell', 'auto.assistant_reset_defaults')}
          </Button>
        </Form>
      </Modal>
    </>,
    document.body
  );
}
