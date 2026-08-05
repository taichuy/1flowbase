import {
  getConsoleAssistantSettings,
  updateConsoleAssistantSettings,
  type ConsoleAssistantPreference,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';
import { Sender } from '@ant-design/x';
import { Button, Dropdown, Form, Modal, Select } from 'antd';
import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';

import { useEmbeddedAssistantSession } from '../../hooks/useEmbeddedAssistantSession';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
import { WindowWorkspaceWindow } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { getWindowWorkspaceViewport } from '../../../../shared/ui/window-workspace/window-workspace-geometry';
import { useWindowWorkspace } from '../../../../shared/ui/window-workspace/WindowWorkspaceProvider';
import type { WindowWorkspaceRect } from '../../../../shared/ui/window-workspace/window-workspace-state';
import { AgentFlowDebugConsole } from '../debug-console/AgentFlowDebugConsole';
import '../editor/styles/shell.css';
import './embedded-assistant.css';

function hasChangedPreference(
  current: ConsoleAssistantPreference | undefined,
  next: ConsoleAssistantPreference
) {
  if (!current || current.application_id !== next.application_id) {
    return true;
  }
  return current.mcp_instance_ids.join('\u0000') !== next.mcp_instance_ids.join('\u0000');
}

const ASSISTANT_WINDOW_ID = 'embedded-agent-assistant-preview';

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
  const [saving, setSaving] = useState(false);
  const [mobile, setMobile] = useState(false);
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
  }, [workspaceId]);

  useEffect(() => {
    if (!open) {
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
  const selectedModel =
    settings?.run_capabilities.models.find(
      (model) => model.id === settings.preference.model
    ) ?? settings?.run_capabilities.models[0];
  const selectedReasoningEffort =
    settings?.preference.reasoning_effort ??
    selectedModel?.default_reasoning_effort ??
    selectedModel?.reasoning_efforts[0];
  const windowEntry = windowWorkspaceState.windows.find(
    (entry) => entry.id === ASSISTANT_WINDOW_ID
  );

  useEffect(() => {
    if (mobile && windowEntry && !windowEntry.maximized) {
      toggleMaximized(ASSISTANT_WINDOW_ID, getWindowWorkspaceViewport());
    }
  }, [mobile, toggleMaximized, windowEntry]);

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
            Math.max(...windowWorkspaceState.windows.map((entry) => entry.z_index))
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
          zIndex={1050 + windowEntry.z_index}
          onActivate={() => activate(ASSISTANT_WINDOW_ID)}
          onRectChange={(rect) => setRect(ASSISTANT_WINDOW_ID, rect)}
        >
          <AgentFlowDebugConsole
            composerFooterActions={
              settings?.run_capabilities.model_selection_enabled ? (
                <>
                  <Dropdown
                    getPopupContainer={(triggerNode) =>
                      triggerNode.closest('.embedded-agent-assistant-preview') ??
                      document.body
                    }
                    placement="topLeft"
                    trigger={['click']}
                    menu={{
                      items: settings.run_capabilities.models.map((model) => ({
                        key: model.id,
                        label: model.name ?? model.id
                      })),
                      selectedKeys: selectedModel ? [selectedModel.id] : [],
                      onClick: ({ key }) => {
                        const model = settings.run_capabilities.models.find(
                          (candidate) => candidate.id === key
                        );
                        if (!model) {
                          return;
                        }
                        void updateRuntimePreference({
                          model: model.id,
                          reasoning_effort: model.default_reasoning_effort ?? null
                        });
                      }
                    }}
                  >
                    <Sender.Switch value={false}>
                      {selectedModel?.name ??
                        selectedModel?.id ??
                        i18nText('appShell', 'auto.assistant_model')}
                    </Sender.Switch>
                  </Dropdown>
                  {settings.run_capabilities.reasoning_effort_enabled &&
                  selectedModel?.reasoning_efforts.length ? (
                    <Dropdown
                      getPopupContainer={(triggerNode) =>
                        triggerNode.closest('.embedded-agent-assistant-preview') ??
                        document.body
                      }
                      placement="topLeft"
                      trigger={['click']}
                      menu={{
                        items: selectedModel.reasoning_efforts.map((effort) => ({
                          key: effort,
                          label: effort
                        })),
                        selectedKeys: selectedReasoningEffort
                          ? [selectedReasoningEffort]
                          : [],
                        onClick: ({ key }) => {
                          const reasoning_effort = String(key);
                          if (
                            !selectedModel.reasoning_efforts.includes(
                              reasoning_effort
                            )
                          ) {
                            return;
                          }
                          void updateRuntimePreference({
                            model: selectedModel.id,
                            reasoning_effort
                          });
                        }
                      }}
                    >
                      <Sender.Switch value={false}>
                        {selectedReasoningEffort ?? '-'}
                      </Sender.Switch>
                    </Dropdown>
                  ) : null}
                </>
              ) : null
            }
            headerActions={
              <Button
                aria-label={i18nText('appShell', 'auto.assistant_settings')}
                disabled={!settings}
                loading={!settings}
                size="small"
                type="text"
                onClick={() => setSettingsOpen(true)}
              >
                AI
              </Button>
            }
            messages={session.messages}
            runContext={session.runContext}
            status={session.status}
            stopping={session.stopping}
            subtitle={selectedFlow?.name}
            title={i18nText('appShell', 'auto.assistant')}
            onChangeRunContextValue={session.setRunContextValue}
            onClearSession={session.clearSession}
            onClose={onClose}
            onStopRun={() => {
              void session.stopRun();
            }}
            onSubmitPrompt={(prompt) => {
              void session.submitPrompt(prompt);
            }}
          />
        </WindowWorkspaceWindow>
      ) : null}
      <Modal
        confirmLoading={saving}
        open={open && settingsOpen}
        title={i18nText('appShell', 'auto.assistant_settings')}
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
