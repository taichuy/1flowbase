import { DeleteOutlined, KeyOutlined, SaveOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Space,
  Switch,
  Tag,
  Typography
} from 'antd';
import { useEffect, useMemo, useState } from 'react';

import type { ConsoleMcpInstance } from '@1flowbase/api-client';
import { i18nText } from '../../../../shared/i18n/text';
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
  const [saveEnabled, setSaveEnabled] = useState(false);
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
      setSaveEnabled(false);
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
      setSaveEnabled(false);
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
      setSaveEnabled(false);
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
  const closeModal = () => {
    setApiKey('');
    setSaveEnabled(false);
    onClose();
  };

  return (
    <Modal
      title={i18nText(
        'settingsMcpManagement',
        'auto.client_configuration_title'
      )}
      open={Boolean(instance)}
      onCancel={closeModal}
      footer={
        <Button onClick={closeModal}>{i18nText('settings', 'auto.off')}</Button>
      }
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
        <Flex align="center" justify="space-between" gap="middle" wrap>
          <Space>
            <Switch
              aria-label={i18nText(
                'settingsMcpManagement',
                'auto.save_this_api_key'
              )}
              checked={saveEnabled}
              onChange={setSaveEnabled}
            />
            <Typography.Text>
              {i18nText('settingsMcpManagement', 'auto.save_this_api_key')}
            </Typography.Text>
            {saved ? (
              <Tag color="green">
                {i18nText('settingsMcpManagement', 'auto.encrypted_saved')}
              </Tag>
            ) : null}
          </Space>
          <Space>
            <Button
              icon={<SaveOutlined />}
              disabled={!saveEnabled || !apiKey.trim()}
              loading={saveMutation.isPending}
              onClick={() => saveMutation.mutate()}
            >
              {i18nText('settingsMcpManagement', 'auto.save_api_key')}
            </Button>
            {saved ? (
              <Button
                danger
                icon={<DeleteOutlined />}
                loading={deleteMutation.isPending}
                onClick={() => deleteMutation.mutate()}
              >
                {i18nText('settingsMcpManagement', 'auto.delete_saved_api_key')}
              </Button>
            ) : null}
          </Space>
        </Flex>
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
          height="260px"
          title={i18nText(
            'settingsMcpManagement',
            'auto.complete_json_configuration'
          )}
          value={configuration}
        />
      </Space>
    </Modal>
  );
}
