import {
  CheckOutlined,
  CopyOutlined,
  DeleteOutlined,
  KeyOutlined,
  SaveOutlined
} from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Form,
  Input,
  Modal,
  Space,
  Tabs,
  Typography
} from 'antd';
import { Children, isValidElement, useEffect, useMemo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

import type { ConsoleMcpInstance } from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';
import { copyTextToClipboard } from '../../../../shared/ui/clipboard/copy-text';
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
import { useAuthStore } from '../../../../state/auth-store';
import {
  deleteSettingsMcpClientCredential,
  fetchSettingsMcpClientCredential,
  saveSettingsMcpClientCredential
} from '../../api/mcp-management';

function buildMcpServerConfiguration(endpoint: string, apiKey: string) {
  return {
    type: 'http',
    url: endpoint,
    headers: {
      Authorization: `Bearer ${apiKey}`
    }
  };
}

function quotePosix(value: string) {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

function quotePowerShell(value: string) {
  return `'${value.replaceAll("'", "''")}'`;
}

function quoteWindowsCommand(value: string) {
  return `"${value.replaceAll('"', '""')}"`;
}

function buildCodexCommands(instanceName: string, endpoint: string, apiKey: string) {
  const variableName = 'FLOWBASE_MCP_API_KEY';
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: `printf ${quotePosix(`\nexport ${variableName}=${quotePosix(apiKey)}\n`)} >> ~/.profile && export ${variableName}=${quotePosix(apiKey)} && codex mcp add ${quotePosix(instanceName)} --url ${quotePosix(endpoint)} --bearer-token-env-var ${variableName}`
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: `[Environment]::SetEnvironmentVariable(${quotePowerShell(variableName)}, ${quotePowerShell(apiKey)}, 'User'); $env:${variableName}=${quotePowerShell(apiKey)}; codex mcp add ${quotePowerShell(instanceName)} --url ${quotePowerShell(endpoint)} --bearer-token-env-var ${variableName}`
    },
    {
      title: 'Windows CMD',
      language: 'bat',
      command: `setx ${variableName} ${quoteWindowsCommand(apiKey)} >nul && set "${variableName}=${apiKey}" && codex mcp add ${quoteWindowsCommand(instanceName)} --url ${quoteWindowsCommand(endpoint)} --bearer-token-env-var ${variableName}`
    }
  ];
}

function buildClaudeCodeCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  return [
    {
      title: 'macOS / Linux / Windows',
      language: 'shell',
      command: `claude mcp add --scope user --transport http ${quoteWindowsCommand(instanceName)} ${quoteWindowsCommand(endpoint)} --header ${quoteWindowsCommand(`Authorization: Bearer ${apiKey}`)}`
    }
  ];
}

function buildOpenCodeCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  return [
    {
      title: 'macOS / Linux / Windows',
      language: 'shell',
      command: `opencode mcp add ${quoteWindowsCommand(instanceName)} --url ${quoteWindowsCommand(endpoint)} --header ${quoteWindowsCommand(`Authorization=Bearer ${apiKey}`)}`
    }
  ];
}

function buildCommandMarkdown(
  commands: Array<{ title: string; language: string; command: string }>
) {
  return commands
    .map(
      ({ title, language, command }) =>
        `### ${title}\n\n\`\`\`${language}\n${command}\n\`\`\``
    )
    .join('\n\n');
}

function CopyableMarkdown({ content }: { content: string }) {
  const [copiedCommand, setCopiedCommand] = useState('');

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        pre: ({ children }) => {
          const codeElement = Children.only(children);
          const command =
            isValidElement<{ children?: unknown }>(codeElement) &&
            typeof codeElement.props.children === 'string'
              ? codeElement.props.children.replace(/\n$/, '')
              : '';

          return (
            <div aria-label={i18nText(
                'settingsMcpManagement',
                'auto.command_block'
              )} role="region" style={{ position: 'relative' }}>
              <pre style={{ paddingRight: 48 }}>{children}</pre>
              <Button
                type="text"
                size="small"
                aria-label={i18nText(
                  'settingsMcpManagement',
                  'auto.copy_command'
                )}
                icon={
                  copiedCommand === command ? <CheckOutlined /> : <CopyOutlined />
                }
                style={{ position: 'absolute', right: 8, top: 8 }}
                onClick={() => {
                  void copyTextToClipboard(command).then(() => setCopiedCommand(command));
                }}
              />
            </div>
          );
        }
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

export function McpClientConfigurationModal({
  instance,
  onClose
}: {
  instance: ConsoleMcpInstance | null;
  onClose: () => void;
}) {
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const [apiKey, setApiKey] = useState('');
  const [saved, setSaved] = useState(false);
  const credentialQueryKey = [
    'settings',
    'mcp-management',
    'client-credential',
    instance?.instance_id
  ] as const;
  const credentialQuery = useQuery({
    queryKey: credentialQueryKey,
    queryFn: () => fetchSettingsMcpClientCredential(instance!.instance_id),
    enabled: Boolean(instance)
  });

  useEffect(() => {
    if (!instance) {
      setApiKey('');
      setSaved(false);
      return;
    }
    if (credentialQuery.data?.saved && credentialQuery.data.api_key) {
      setApiKey(credentialQuery.data.api_key);
      setSaved(true);
    }
  }, [credentialQuery.data, instance]);

  const saveMutation = useMutation({
    mutationFn: () =>
      saveSettingsMcpClientCredential(instance!.instance_id, apiKey, csrfToken),
    onSuccess: () => {
      queryClient.setQueryData(credentialQueryKey, {
        saved: true,
        api_key: apiKey
      });
      setSaved(true);
      message.success(i18nText('settingsMcpManagement', 'auto.api_key_saved'));
    }
  });
  const deleteMutation = useMutation({
    mutationFn: () =>
      deleteSettingsMcpClientCredential(instance!.instance_id, csrfToken),
    onSuccess: () => {
      queryClient.setQueryData(credentialQueryKey, { saved: false });
      setApiKey('');
      setSaved(false);
      message.success(
        i18nText('settingsMcpManagement', 'auto.saved_api_key_deleted')
      );
    }
  });

  const endpoint = instance
    ? `${window.location.origin}/api/mcp/${encodeURIComponent(instance.instance_id)}`
    : '';
  const configuration = useMemo(
    () => buildMcpServerConfiguration(endpoint, apiKey),
    [apiKey, endpoint]
  );
  const commandTabs = useMemo(() => {
    if (!instance || !apiKey.trim()) {
      return null;
    }

    return {
      codex: buildCommandMarkdown(
        buildCodexCommands(instance.instance_id, endpoint, apiKey)
      ),
      claudeCode: buildCommandMarkdown(
        buildClaudeCodeCommands(instance.instance_id, endpoint, apiKey)
      ),
      openCode: buildCommandMarkdown(
        buildOpenCodeCommands(instance.instance_id, endpoint, apiKey)
      )
    };
  }, [apiKey, endpoint, instance]);
  const closeModal = () => {
    setApiKey('');
    onClose();
  };
  const renderAgentCommands = (content: string | undefined) =>
    content ? (
      <CopyableMarkdown content={content} />
    ) : (
      <Typography.Text type="secondary">
        {i18nText(
          'settingsMcpManagement',
          'auto.enter_api_key_to_generate_commands'
        )}
      </Typography.Text>
    );

  return (
    <Modal
      title={i18nText(
        'settingsMcpManagement',
        'auto.client_configuration_title'
      )}
      open={Boolean(instance)}
      onCancel={closeModal}
      footer={[
        <Button key="close" onClick={closeModal}>
          {i18nText('settings', 'auto.off')}
        </Button>,
        <Button
          key="delete"
          danger
          icon={<DeleteOutlined />}
          disabled={!saved}
          loading={deleteMutation.isPending}
          onClick={() => deleteMutation.mutate()}
        >
          {i18nText('settingsMcpManagement', 'auto.clear_credential')}
        </Button>,
        <Button
          key="save"
          type="primary"
          icon={<SaveOutlined />}
          disabled={!apiKey.trim()}
          loading={saveMutation.isPending}
          onClick={() => saveMutation.mutate()}
        >
          {i18nText('settingsMcpManagement', 'auto.save_api_key')}
        </Button>
      ]}
      width={720}
      destroyOnHidden
    >
      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        <Alert
          type="info"
          showIcon
          message={i18nText(
            'settingsMcpManagement',
            'auto.api_key_encrypted_storage_notice'
          )}
          action={
            <Button
              href="/settings/api-key-authentication"
              icon={<KeyOutlined />}
            >
              {i18nText('settingsMcpManagement', 'auto.generate_api_key')}
            </Button>
          }
        />
        <Form layout="vertical">
          <Form.Item label={i18nText('settingsMcpManagement', 'auto.api_key')}>
            <Input.Password
              aria-label={i18nText('settingsMcpManagement', 'auto.api_key')}
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              autoComplete="off"
            />
          </Form.Item>
        </Form>
        <Tabs
          items={[
            {
              key: 'common',
              label: i18nText('settingsMcpManagement', 'auto.common'),
              children: (
                <JsonPreviewBlock
                  collapsible={false}
                  copyAriaLabel={i18nText(
                    'settingsMcpManagement',
                    'auto.copy_complete_json_configuration'
                  )}
                  fullscreenAriaLabel={i18nText(
                    'settingsMcpManagement',
                    'auto.enlarge_complete_json_configuration'
                  )}
                  title={i18nText(
                    'settingsMcpManagement',
                    'auto.complete_json_configuration'
                  )}
                  value={configuration}
                />
              )
            },
            {
              key: 'codex',
              label: 'Codex',
              children: renderAgentCommands(commandTabs?.codex)
            },
            {
              key: 'claude-code',
              label: 'Claude Code',
              children: renderAgentCommands(commandTabs?.claudeCode)
            },
            {
              key: 'opencode',
              label: 'OpenCode',
              children: renderAgentCommands(commandTabs?.openCode)
            }
          ]}
        />
      </Space>
    </Modal>
  );
}
