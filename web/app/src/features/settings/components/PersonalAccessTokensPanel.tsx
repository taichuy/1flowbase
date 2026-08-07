import { useCallback, useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Table,
  Tag,
  Typography,
  App,
  type TableProps
} from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';

import { useAuthStore } from '../../../state/auth-store';
import { formatDateTime as formatLocalizedDateTime } from '../../../shared/i18n/format';
import { copyTextToClipboard } from '../../../shared/ui/clipboard/copy-text';
import {
  createSettingsPersonalAccessToken,
  fetchSettingsPersonalAccessTokenRoleOptions,
  fetchSettingsPersonalAccessTokens,
  revokeSettingsPersonalAccessToken,
  settingsPersonalAccessTokenRoleOptionsQueryKey,
  settingsPersonalAccessTokensQueryKey,
  type CreateSettingsPersonalAccessTokenInput,
  type SettingsPersonalAccessToken
} from '../api/personal-access-tokens';
import { i18nText } from '../../../shared/i18n/text';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import './personal-access-tokens-panel.css';

interface CreatePersonalAccessTokenFormValues {
  name: string;
  role_code: CreateSettingsPersonalAccessTokenInput['role_code'];
  expiration_policy: CreateSettingsPersonalAccessTokenInput['expiration_policy'];
}

function formatDateTime(value: string | null) {
  if (!value) {
    return i18nText('settings', 'auto.never_expires');
  }

  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return formatLocalizedDateTime(date, {
    dateStyle: 'medium',
    timeStyle: 'short'
  });
}

function formatLastUsedAt(value: string | null) {
  return value
    ? formatDateTime(value)
    : i18nText('settings', 'auto.not_used_yet');
}

export function PersonalAccessTokensPanel() {
  const { message } = App.useApp();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const actor = useAuthStore((state) => state.actor);
  const [createForm] = Form.useForm<CreatePersonalAccessTokenFormValues>();
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [createdToken, setCreatedToken] =
    useState<SettingsPersonalAccessToken | null>(null);

  const tokensQuery = useQuery({
    queryKey: settingsPersonalAccessTokensQueryKey,
    queryFn: fetchSettingsPersonalAccessTokens
  });
  const roleOptionsQuery = useQuery({
    queryKey: settingsPersonalAccessTokenRoleOptionsQueryKey,
    queryFn: fetchSettingsPersonalAccessTokenRoleOptions
  });

  const createMutation = useMutation({
    mutationFn: async (values: CreatePersonalAccessTokenFormValues) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return createSettingsPersonalAccessToken(
        {
          name: values.name,
          role_code: values.role_code,
          expiration_policy: values.expiration_policy
        },
        csrfToken
      );
    },
    onSuccess: async (token) => {
      setCreatedToken(token);
      setCreateModalOpen(false);
      createForm.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsPersonalAccessTokensQueryKey
      });
    }
  });

  const revokeMutation = useMutation({
    mutationFn: async (apiKeyId: string) => {
      if (!csrfToken) {
        throw new Error('missing csrf token');
      }

      return revokeSettingsPersonalAccessToken(apiKeyId, csrfToken);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: settingsPersonalAccessTokensQueryKey
      });
    }
  });

  const expirationOptions = useMemo(
    () => [
      {
        value: '30d',
        label: i18nText('settings', 'auto.expiration_thirty_days')
      },
      {
        value: '1y',
        label: i18nText('settings', 'auto.expiration_one_year')
      },
      {
        value: '3y',
        label: i18nText('settings', 'auto.expiration_three_years')
      },
      {
        value: 'never',
        label: i18nText('settings', 'auto.never_expires')
      }
    ],
    []
  );
  const roleOptions = useMemo(() => {
    const roles = roleOptionsQuery.data ?? [];
    if (roles.length) {
      return roles.map((role) => ({
        value: role.code,
        label:
          role.name === role.code ? role.name : `${role.name} (${role.code})`
      }));
    }

    return actor?.effective_display_role
      ? [
          {
            value: actor.effective_display_role,
            label: actor.effective_display_role
          }
        ]
      : [];
  }, [actor?.effective_display_role, roleOptionsQuery.data]);

  useEffect(() => {
    if (!createModalOpen || !roleOptions.length) {
      return;
    }

    const currentRoleCode = createForm.getFieldValue('role_code') as
      | string
      | undefined;
    if (roleOptions.some((option) => option.value === currentRoleCode)) {
      return;
    }

    const preferredRole =
      roleOptions.find(
        (option) => option.value === actor?.effective_display_role
      ) ?? roleOptions[0];
    createForm.setFieldValue('role_code', preferredRole.value);
  }, [actor?.effective_display_role, createForm, createModalOpen, roleOptions]);

  const handleCreateSubmit = useCallback(
    (values: CreatePersonalAccessTokenFormValues) => {
      createMutation.mutate(values);
    },
    [createMutation]
  );

  const handleCopyCreatedToken = useCallback(async () => {
    if (!createdToken?.token) {
      return;
    }

    try {
      await copyTextToClipboard(createdToken.token);
      message.success(i18nText('settings', 'auto.copied'));
    } catch {
      message.error(i18nText('settings', 'auto.copy_failed_manual'));
    }
  }, [createdToken?.token, message]);

  const sectionStatus = useMemo(
    () => (
      <div className="personal-access-tokens-panel__action-row">
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.user_api_key_security_notice')}
        </Typography.Text>
        <Button
          type="primary"
          icon={<PlusOutlined />}
          onClick={() => setCreateModalOpen(true)}
        >
          {i18nText('settings', 'auto.create_api_key')}
        </Button>
      </div>
    ),
    []
  );

  const columns = useMemo<TableProps<SettingsPersonalAccessToken>['columns']>(
    () => [
      {
        title: i18nText('settings', 'auto.name'),
        dataIndex: 'name',
        key: 'name',
        render: (_: unknown, token) => (
          <Typography.Text strong>{token.name}</Typography.Text>
        )
      },
      {
        title: i18nText('settings', 'auto.api_key_prefix'),
        dataIndex: 'token_prefix',
        key: 'token_prefix',
        width: 160,
        render: (token_prefix: string) => (
          <Typography.Text code>{token_prefix}</Typography.Text>
        )
      },
      {
        title: i18nText('settings', 'auto.status'),
        key: 'status',
        width: 120,
        render: (_: unknown, token) =>
          token.revoked || !token.enabled ? (
            <Tag>{i18nText('settings', 'auto.revoked')}</Tag>
          ) : (
            <Tag color="green">{i18nText('settings', 'auto.enabled_alt')}</Tag>
          )
      },
      {
        title: i18nText('settings', 'auto.expires'),
        dataIndex: 'expires_at',
        key: 'expires_at',
        render: (expires_at: string | null) => formatDateTime(expires_at)
      },
      {
        title: i18nText('settings', 'auto.last_used_at'),
        dataIndex: 'last_used_at',
        key: 'last_used_at',
        render: (last_used_at: string | null) => formatLastUsedAt(last_used_at)
      },
      {
        title: i18nText('settings', 'auto.created'),
        dataIndex: 'created_at',
        key: 'created_at',
        render: (created_at: string) => formatDateTime(created_at)
      },
      {
        title: i18nText('settings', 'auto.operation'),
        key: 'action',
        width: 120,
        render: (_: unknown, token) =>
          token.revoked || !token.enabled ? null : (
            <Popconfirm
              title={i18nText('settings', 'auto.delete_api_key')}
              description={i18nText(
                'settings',
                'auto.delete_api_key_description',
                { value1: token.name }
              )}
              okText={i18nText('settings', 'auto.confirm_delete')}
              cancelText={i18nText('settings', 'auto.cancel')}
              okButtonProps={{ danger: true }}
              onConfirm={() => revokeMutation.mutate(token.id)}
            >
              <Button
                danger
                size="small"
                icon={<DeleteOutlined />}
                loading={revokeMutation.isPending}
              >
                {i18nText('settings', 'auto.delete')}
              </Button>
            </Popconfirm>
          )
      }
    ],
    [revokeMutation]
  );

  return (
    <SettingsSectionSurface heightMode="fill" status={sectionStatus}>
      <Table<SettingsPersonalAccessToken>
        rowKey="id"
        loading={tokensQuery.isLoading}
        dataSource={tokensQuery.data ?? []}
        columns={columns}
        pagination={false}
        size="middle"
        locale={{ emptyText: i18nText('settings', 'auto.no_user_api_keys') }}
      />

      <Modal
        title={i18nText('settings', 'auto.create_api_key')}
        open={createModalOpen}
        onCancel={() => {
          setCreateModalOpen(false);
          createForm.resetFields();
        }}
        onOk={() => createForm.submit()}
        confirmLoading={createMutation.isPending}
        okText={i18nText('settings', 'auto.create')}
        cancelText={i18nText('settings', 'auto.cancel')}
        destroyOnHidden
      >
        <Form
          form={createForm}
          layout="vertical"
          onFinish={handleCreateSubmit}
          style={{ marginTop: 16 }}
          initialValues={{ expiration_policy: '1y' }}
        >
          <Form.Item
            label={i18nText('settings', 'auto.name')}
            name="name"
            rules={[
              {
                required: true,
                message: i18nText('settings', 'auto.fill_name')
              }
            ]}
          >
            <Input autoFocus />
          </Form.Item>
          <Form.Item
            label={i18nText('settings', 'auto.role')}
            name="role_code"
            rules={[
              {
                required: true,
                message: i18nText('settings', 'auto.please_select_role')
              }
            ]}
          >
            <Select
              options={roleOptions}
              loading={roleOptionsQuery.isLoading}
              showSearch
              optionFilterProp="label"
            />
          </Form.Item>
          <Form.Item
            label={i18nText('settings', 'auto.expiration_policy')}
            name="expiration_policy"
            rules={[
              {
                required: true,
                message: i18nText('settings', 'auto.please_select_expiration')
              }
            ]}
          >
            <Select options={expirationOptions} />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        title={i18nText('settings', 'auto.api_key_created')}
        open={Boolean(createdToken?.token)}
        onCancel={() => setCreatedToken(null)}
        footer={[
          <Button key="close" type="text" onClick={() => setCreatedToken(null)}>
            {i18nText('settings', 'auto.off')}
          </Button>,
          <Button
            key="copy"
            aria-label={i18nText('settings', 'auto.copy')}
            className="personal-access-tokens-panel__created-token-copy"
            onClick={handleCopyCreatedToken}
          >
            {i18nText('settings', 'auto.copy')}
          </Button>
        ]}
        destroyOnHidden
      >
        <Space
          orientation="vertical"
          className="personal-access-tokens-panel__created-token-modal"
        >
          <Typography.Text>
            {i18nText('settings', 'auto.api_key_created_once_notice')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.api_key_created_hidden_after_close')}
          </Typography.Text>
          <Typography.Text className="personal-access-tokens-panel__created-token">
            {createdToken?.token}
          </Typography.Text>
        </Space>
      </Modal>
    </SettingsSectionSurface>
  );
}
