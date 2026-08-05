import {
  getConsoleAssistantSettings,
  updateConsoleAssistantSettings,
  type ConsoleAssistantPreference,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';
import { Button, Form, Modal, Select } from 'antd';
import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';

import { useEmbeddedAssistantSession } from '../../hooks/useEmbeddedAssistantSession';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';
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
  return (
    current.mcp_instance_ids.join('\u0000') !==
    next.mcp_instance_ids.join('\u0000')
  );
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
  const [form] = Form.useForm<ConsoleAssistantPreference>();
  const session = useEmbeddedAssistantSession(
    settings?.preference.application_id ?? null
  );

  useEffect(() => {
    setSettings(null);
    setSettingsOpen(false);
  }, [workspaceId]);

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
            enabled_mcp_instances: []
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

  if (typeof document === 'undefined') {
    return null;
  }

  return createPortal(
    <>
      {open ? (
        <aside
          aria-label={i18nText('appShell', 'auto.assistant')}
          className="embedded-agent-assistant-preview"
        >
          <AgentFlowDebugConsole
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
        </aside>
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
        </Form>
      </Modal>
    </>,
    document.body
  );
}
