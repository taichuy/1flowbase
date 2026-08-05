import { MessageOutlined, ReloadOutlined, SettingOutlined } from '@ant-design/icons';
import {
  getConsoleAssistantSettings,
  startConsoleAssistantRun,
  updateConsoleAssistantSettings,
  type ConsoleAssistantPreference,
  type ConsoleAssistantSettings
} from '@1flowbase/api-client';
import { Button, Drawer, Empty, Form, Input, Modal, Select, Space, Spin, Tooltip, Typography } from 'antd';
import { useEffect, useState } from 'react';

import { i18nText } from '../shared/i18n/text';
import { useAuthStore } from '../state/auth-store';

interface AssistantMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

function assistantAnswer(
  answer: string | null,
  output: unknown,
  error: unknown | null
) {
  if (error) {
    return i18nText('appShell', 'auto.assistant_run_failed');
  }
  if (answer) {
    return answer;
  }
  if (typeof output === 'object' && output !== null) {
    const value = output as Record<string, unknown>;
    for (const key of ['text', 'answer', 'output']) {
      if (typeof value[key] === 'string') {
        return value[key];
      }
    }
  }
  return i18nText('appShell', 'auto.assistant_no_answer');
}

export function AssistantChromeAction() {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore((state) => state.actor?.current_workspace_id);
  const [open, setOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<ConsoleAssistantSettings | null>(null);
  const [messages, setMessages] = useState<AssistantMessage[]>([]);
  const [prompt, setPrompt] = useState('');
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [isComposing, setIsComposing] = useState(false);
  const [form] = Form.useForm<ConsoleAssistantPreference>();

  useEffect(() => {
    setSettings(null);
    setMessages([]);
  }, [workspaceId]);

  useEffect(() => {
    if (!open || settings) return;
    void getConsoleAssistantSettings().then((value) => {
      setSettings(value);
      form.setFieldsValue(value.preference);
    });
  }, [form, open, settings]);

  async function submit() {
    const query = prompt.trim();
    if (!query || !csrfToken || loading) return;
    const history = messages.map(({ role, content }) => ({ role, content }));
    const userMessage = { id: crypto.randomUUID(), role: 'user' as const, content: query };
    setMessages((current) => [...current, userMessage]);
    setPrompt('');
    setLoading(true);
    try {
      const run = await startConsoleAssistantRun({ query, history }, csrfToken);
      setMessages((current) => [...current, {
        id: run.id,
        role: 'assistant',
        content: assistantAnswer(run.answer, run.output_payload, run.error_payload)
      }]);
    } catch {
      setMessages((current) => [...current, {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: i18nText('appShell', 'auto.assistant_run_failed')
      }]);
    } finally {
      setLoading(false);
    }
  }

  async function saveSettings() {
    if (!csrfToken) return;
    const preference = await form.validateFields();
    setSaving(true);
    try {
      const value = await updateConsoleAssistantSettings(preference, csrfToken);
      setSettings(value);
      setMessages([]);
      setSettingsOpen(false);
    } finally {
      setSaving(false);
    }
  }

  return <>
    <Tooltip title={i18nText('appShell', 'auto.assistant')}>
      <Button
        aria-label={i18nText('appShell', 'auto.assistant')}
        icon={<MessageOutlined />}
        type="text"
        onClick={() => setOpen(true)}
      />
    </Tooltip>
    <Drawer
      className="assistant-chrome-drawer"
      closable
      extra={<Space>
        <Button aria-label={i18nText('appShell', 'auto.assistant_reset')} icon={<ReloadOutlined />} type="text" onClick={() => setMessages([])} />
        <Button aria-label={i18nText('appShell', 'auto.assistant_settings')} icon={<SettingOutlined />} type="text" onClick={() => setSettingsOpen(true)} />
      </Space>}
      open={open}
      title={i18nText('appShell', 'auto.assistant')}
      width={440}
      onClose={() => setOpen(false)}
    >
      <div className="assistant-chrome-conversation">
        {messages.length === 0 ? <Empty description={i18nText('appShell', 'auto.assistant_empty')} /> : messages.map((message) => <article key={message.id} className={`assistant-chrome-message assistant-chrome-message--${message.role}`}><Typography.Paragraph>{message.content}</Typography.Paragraph></article>)}
        {loading ? <Spin size="small" /> : null}
      </div>
      <Input.TextArea
        autoSize={{ minRows: 2, maxRows: 5 }}
        disabled={!settings?.preference.application_id || loading}
        placeholder={i18nText('appShell', 'auto.assistant_input')}
        value={prompt}
        onChange={(event) => setPrompt(event.target.value)}
        onCompositionStart={() => setIsComposing(true)}
        onCompositionEnd={() => setIsComposing(false)}
        onPressEnter={(event) => {
          if (event.shiftKey || isComposing || event.nativeEvent.isComposing) {
            return;
          }
          event.preventDefault();
          void submit();
        }}
      />
      <Button block disabled={!prompt.trim() || !settings?.preference.application_id || loading} loading={loading} type="primary" onClick={() => void submit()}>{i18nText('appShell', 'auto.assistant_send')}</Button>
    </Drawer>
    <Modal confirmLoading={saving} open={settingsOpen} title={i18nText('appShell', 'auto.assistant_settings')} onCancel={() => setSettingsOpen(false)} onOk={() => void saveSettings()}>
      <Form form={form} layout="vertical">
        <Form.Item label={i18nText('appShell', 'auto.assistant_flow')} name="application_id" rules={[{ required: true }]}>
          <Select allowClear options={settings?.published_agent_flows.map((flow) => ({ value: flow.application_id, label: flow.name })) ?? []} />
        </Form.Item>
        <Form.Item label={i18nText('appShell', 'auto.assistant_mcp')} name="mcp_instance_ids">
          <Select mode="multiple" options={settings?.enabled_mcp_instances.map((instance) => ({ value: instance.instance_id, label: instance.name })) ?? []} />
        </Form.Item>
      </Form>
    </Modal>
  </>;
}
