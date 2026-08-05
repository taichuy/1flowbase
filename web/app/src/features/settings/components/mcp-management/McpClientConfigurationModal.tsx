import { DeleteOutlined, KeyOutlined, SaveOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, App, Button, Form, Input, Space, Tabs, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';

import type { ConsoleMcpInstance } from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
import { McpCommandMarkdownPreview } from './McpCommandMarkdownPreview';
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

function quoteTomlString(value: string) {
  return JSON.stringify(value);
}

function buildCodexTomlHeaderLines(instanceName: string, apiKey: string) {
  return [
    `[mcp_servers.${quoteTomlString(instanceName)}.http_headers]`,
    `Authorization = ${quoteTomlString(`Bearer ${apiKey}`)}`
  ];
}

function buildCodexCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  const [headerSection, authorizationHeader] = buildCodexTomlHeaderLines(
    instanceName,
    apiKey
  );
  const cmdHeaderSection = `${quotePowerShell('[mcp_servers.')} + [char]34 + ${quotePowerShell(instanceName)} + [char]34 + ${quotePowerShell('.http_headers]')}`;
  const cmdAuthorizationHeader = `${quotePowerShell('Authorization = ')} + [char]34 + ${quotePowerShell(`Bearer ${apiKey}`)} + [char]34`;
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: `codex mcp remove ${quotePosix(instanceName)} >/dev/null 2>&1 || true; codex mcp add ${quotePosix(instanceName)} --url ${quotePosix(endpoint)} && printf '%s\\n' '' ${quotePosix(headerSection)} ${quotePosix(authorizationHeader)} >> ~/.codex/config.toml`
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: `codex mcp remove ${quotePowerShell(instanceName)} 2>$null; codex mcp add ${quotePowerShell(instanceName)} --url ${quotePowerShell(endpoint)}; Add-Content -Path (Join-Path $HOME '.codex\\config.toml') -Value '', ${quotePowerShell(headerSection)}, ${quotePowerShell(authorizationHeader)}`
    },
    {
      title: 'Windows CMD',
      language: 'bat',
      command: `codex mcp remove ${quoteWindowsCommand(instanceName)} >nul 2>&1 & codex mcp add ${quoteWindowsCommand(instanceName)} --url ${quoteWindowsCommand(endpoint)} && powershell -NoProfile -Command "Add-Content -Path (Join-Path $HOME '.codex\\config.toml') -Value '', (${cmdHeaderSection}), (${cmdAuthorizationHeader})"`
    }
  ];
}

function buildCodexRemoveCommands(instanceName: string) {
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: `codex mcp remove ${quotePosix(instanceName)}`
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: `codex mcp remove ${quotePowerShell(instanceName)}`
    },
    {
      title: 'Windows CMD',
      language: 'bat',
      command: `codex mcp remove ${quoteWindowsCommand(instanceName)}`
    }
  ];
}

function buildClaudeCodeCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  const addCommand = `claude mcp add --scope user --transport http ${quoteWindowsCommand(instanceName)} ${quoteWindowsCommand(endpoint)} --header ${quoteWindowsCommand(`Authorization: Bearer ${apiKey}`)}`;
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: `claude mcp remove --scope user ${quotePosix(instanceName)} >/dev/null 2>&1 || true; ${addCommand}`
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: `claude mcp remove --scope user ${quotePowerShell(instanceName)} 2>$null; ${addCommand}`
    }
  ];
}

function buildClaudeCodeRemoveCommands(instanceName: string) {
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: `claude mcp remove --scope user ${quotePosix(instanceName)}`
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: `claude mcp remove --scope user ${quotePowerShell(instanceName)}`
    }
  ];
}

function buildOpenCodeCommands(
  instanceName: string,
  endpoint: string,
  apiKey: string
) {
  const addCommand = `opencode mcp add ${quoteWindowsCommand(instanceName)} --url ${quoteWindowsCommand(endpoint)} --header ${quoteWindowsCommand(`Authorization=Bearer ${apiKey}`)}`;
  return [
    {
      title: 'macOS / Linux Shell',
      language: 'bash',
      command: addCommand
    },
    {
      title: 'Windows PowerShell',
      language: 'powershell',
      command: addCommand
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
    if (!instance) {
      return null;
    }

    const installEnabled = Boolean(apiKey.trim());
    return {
      codex: {
        install: installEnabled
          ? buildCommandMarkdown(
              buildCodexCommands(instance.instance_id, endpoint, apiKey)
            )
          : null,
        remove: buildCommandMarkdown(
          buildCodexRemoveCommands(instance.instance_id)
        )
      },
      claudeCode: {
        install: installEnabled
          ? buildCommandMarkdown(
              buildClaudeCodeCommands(instance.instance_id, endpoint, apiKey)
            )
          : null,
        remove: buildCommandMarkdown(
          buildClaudeCodeRemoveCommands(instance.instance_id)
        )
      },
      openCode: {
        install: installEnabled
          ? buildCommandMarkdown(
              buildOpenCodeCommands(instance.instance_id, endpoint, apiKey)
            )
          : null,
        remove: null
      }
    };
  }, [apiKey, endpoint, instance]);
  const closeModal = () => {
    setApiKey('');
    onClose();
  };
  const renderAgentCommands = (
    commands:
      | {
          install: string | null;
          remove: string | null;
        }
      | undefined
  ) => (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Space orientation="vertical" size="small" style={{ width: '100%' }}>
        <Typography.Title level={5} style={{ margin: 0 }}>
          {i18nText(
            'settingsMcpManagement',
            'auto.install_update_commands'
          )}
        </Typography.Title>
        {commands?.install ? (
          <McpCommandMarkdownPreview
            content={commands.install}
            ariaLabel={i18nText(
              'settingsMcpManagement',
              'auto.install_update_command_preview'
            )}
          />
        ) : (
          <Typography.Text type="secondary">
            {i18nText(
              'settingsMcpManagement',
              'auto.enter_api_key_to_generate_commands'
            )}
          </Typography.Text>
        )}
      </Space>
      <Space orientation="vertical" size="small" style={{ width: '100%' }}>
        <Typography.Title level={5} style={{ margin: 0 }}>
          {i18nText('settingsMcpManagement', 'auto.remove_commands')}
        </Typography.Title>
        {commands?.remove ? (
          <McpCommandMarkdownPreview
            content={commands.remove}
            ariaLabel={i18nText(
              'settingsMcpManagement',
              'auto.remove_command_preview'
            )}
          />
        ) : (
          <Alert
            type="warning"
            showIcon
            title={i18nText(
              'settingsMcpManagement',
              'auto.open_code_remove_unsupported'
            )}
            description={i18nText(
              'settingsMcpManagement',
              'auto.open_code_remove_configuration_path'
            )}
          />
        )}
      </Space>
    </Space>
  );

  return (
    <FixedHeightModal
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
      className="mcp-management__client-configuration-modal"
      scrollBodyClassName="mcp-management__client-configuration-scroll-body"
      width={720}
      destroyOnHidden
    >
      <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
        <Alert
          type="info"
          showIcon
          title={i18nText(
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
        <Alert
          type="warning"
          showIcon
          title={i18nText(
            'settingsMcpManagement',
            'auto.client_config_plaintext_notice'
          )}
        />
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
    </FixedHeightModal>
  );
}
