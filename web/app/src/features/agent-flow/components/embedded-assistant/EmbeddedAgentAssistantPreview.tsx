import {
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  updateConsoleAssistantSettings,
  type ConsoleAssistantConversationPage,
  type ConsoleAssistantPreference,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';
import {
  CheckOutlined,
  CloseOutlined,
  HistoryOutlined,
  PlusOutlined,
  SettingOutlined
} from '@ant-design/icons';
import { Conversations, Sender } from '@ant-design/x';
import {
  Button,
  Dropdown,
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
  fetchRuntimeDebugArtifacts
} from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { getWindowWorkspaceViewport } from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';
import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole';
import { formatLlmTokenCount } from '../../lib/model-options';
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
    next.mcp_instance_ids.join('\u0000')
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
  onClose
}: {
  open: boolean;
  onClose: () => void;
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
  const historyExpansionSideRef = useRef<AssistantHistoryExpansionSide | null>(
    null
  );
  const historyLayoutRef = useRef<HTMLDivElement | null>(null);
  const historyWidthRef = useRef(historyWidth);
  const assistantWindowRectRef = useRef<WindowWorkspaceRect | null>(null);
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
    settings?.preference.application_id ?? null
  );

  useEffect(() => {
    setSettings(null);
    setSettingsOpen(false);
    setHistoryOpen(false);
    setHistoryFullView(false);
    historyExpansionSideRef.current = null;
    setHistoryPage(null);
  }, [workspaceId]);

  useEffect(() => {
    if (!open) {
      historyExpansionSideRef.current = null;
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
            preference: { application_id: null, mcp_instance_ids: [] },
            published_agent_flows: [],
            enabled_mcp_instances: [],
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
    if (historyOpen) {
      void loadHistory();
    }
  }, [historyOpen, loadHistory]);
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
    const side = historyExpansionSideRef.current;
    if (!rect || !side) {
      return historyWidthRef.current;
    }
    const viewport = getWindowWorkspaceViewport();
    const availableWidth =
      side === 'left'
        ? rect.left - (viewport.left + 8)
        : viewport.left + viewport.width - 8 - (rect.left + rect.width);
    return Math.max(
      ASSISTANT_HISTORY_MIN_WIDTH,
      historyWidthRef.current + availableWidth
    );
  }

  function openHistory() {
    const rect = assistantWindowRectRef.current;
    if (!rect || mobile) {
      historyExpansionSideRef.current = null;
      setHistoryFullView(true);
      setHistoryOpen(true);
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
      setHistoryFullView(true);
      setHistoryOpen(true);
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
    assistantWindowRectRef.current = nextRect;
    setRect(ASSISTANT_WINDOW_ID, nextRect);
    setHistoryFullView(false);
    setHistoryOpen(true);
  }

  function toggleHistory() {
    if (historyOpen) {
      closeHistory();
      return;
    }
    openHistory();
  }

  function closeHistory() {
    const side = historyExpansionSideRef.current;
    const rect = assistantWindowRectRef.current;
    if (side && rect) {
      const nextWidth = Math.max(
        400,
        rect.width - assistantHistoryOccupiedWidth()
      );
      const removedWidth = rect.width - nextWidth;
      const nextRect =
        side === 'left'
          ? { ...rect, left: rect.left + removedWidth, width: nextWidth }
          : { ...rect, width: nextWidth };
      assistantWindowRectRef.current = nextRect;
      setRect(ASSISTANT_WINDOW_ID, nextRect);
    }
    historyExpansionSideRef.current = null;
    setHistoryFullView(false);
    setHistoryOpen(false);
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
    if (!historyOpen) {
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
    setHistoryFullView(true);
  }, [historyFullView, historyOpen, mobile, windowEntry?.rect.width]);

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
      const rect = assistantWindowRectRef.current;
      const side = historyExpansionSideRef.current;
      if (nextWidth === currentWidth || !rect || !side) {
        return;
      }
      const widthDelta = nextWidth - currentWidth;
      const nextRect =
        side === 'left'
          ? {
              ...rect,
              left: rect.left - widthDelta,
              width: rect.width + widthDelta
            }
          : { ...rect, width: rect.width + widthDelta };
      historyWidthRef.current = nextWidth;
      assistantWindowRectRef.current = nextRect;
      setHistoryWidth(nextWidth);
      setRect(ASSISTANT_WINDOW_ID, nextRect);
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
            data-history-open={historyOpen}
            ref={historyLayoutRef}
          >
            {historyOpen ? (
              <aside
                aria-label={i18nText('appShell', 'auto.assistant_history')}
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
                      {i18nText('appShell', 'auto.assistant_history')}
                    </span>
                    <Button
                      aria-label={i18nText('agentFlow', 'auto.close', {
                        value1: i18nText('appShell', 'auto.assistant_history')
                      })}
                      icon={<CloseOutlined />}
                      size="small"
                      type="text"
                      onClick={closeHistory}
                    />
                  </div>
                  <div className="embedded-agent-assistant-preview__history-body">
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
                          !session.canChangeConversation || historyLoading,
                        group: assistantConversationGroup(item.updated_at),
                        key: assistantConversationKey(item),
                        label:
                          item.title ??
                          (item.conversation_id
                            ? i18nText(
                                'appShell',
                                'auto.assistant_new_conversation'
                              )
                            : i18nText(
                                'appShell',
                                'auto.assistant_legacy_snapshot'
                              ))
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
                            legacyFlowRunId: item.legacy_flow_run_id
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
                        onClick={() => void loadHistory(historyPage.page + 1)}
                      >
                        {i18nText('appShell', 'auto.assistant_load_more')}
                      </Button>
                    ) : null}
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
              hidden={(mobile || historyFullView) && historyOpen}
            >
              <AgentFlowDebugConsole
                clearDisabled={!session.canChangeConversation}
                composerFooterActions={
                  settings?.run_capabilities.model_selection_enabled ? (
                    <>
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
                                      formatLlmTokenCount(contextWindow) ?? '0',
                                    value1:
                                      formatLlmTokenCount(contextTokenUsage) ??
                                      '0'
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
                              const modelId = selection.slice('model:'.length);
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
                          {settings.run_capabilities.reasoning_effort_enabled &&
                          selectedReasoningEffort ? (
                            <span className="embedded-agent-assistant-preview__runtime-preferences-effort">
                              {selectedReasoningEffort}
                            </span>
                          ) : null}
                        </Sender.Switch>
                      </Dropdown>
                    </>
                  ) : null
                }
                headerActions={
                  <>
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
                  void session.closeSession();
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
                onStopRun={() => {
                  void session.stopRun();
                }}
                onSubmitPrompt={(prompt) => {
                  void session.submitPrompt(prompt);
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
