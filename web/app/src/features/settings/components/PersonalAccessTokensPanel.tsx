import { useCallback, useEffect, useMemo, useState } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Flex,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Tag,
  Typography,
  App
} from 'antd';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';

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
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../shared/ui/data-table/DataTable';
import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../shared/ui/data-table/data-table-state';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import './personal-access-tokens-panel.css';

interface CreatePersonalAccessTokenFormValues {
  name: string;
  role_code: CreateSettingsPersonalAccessTokenInput['role_code'];
  expiration_policy: CreateSettingsPersonalAccessTokenInput['expiration_policy'];
}

const PAGE_SIZE = 20;

type PersonalAccessTokenStatusFilter = 'all' | 'active' | 'deleted';

function personalAccessTokenStatus(
  token: SettingsPersonalAccessToken
): Exclude<PersonalAccessTokenStatusFilter, 'all'> {
  return token.revoked || !token.enabled ? 'deleted' : 'active';
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
  const [statusFilterDraft, setStatusFilterDraft] =
    useState<PersonalAccessTokenStatusFilter>('active');
  const [statusFilter, setStatusFilter] =
    useState<PersonalAccessTokenStatusFilter>('active');
  const [page, setPage] = useState(1);

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
      <Typography.Text type="secondary">
        {i18nText('settings', 'auto.user_api_key_security_notice')}
      </Typography.Text>
    ),
    []
  );

  const columns = useMemo<Array<DataTableColumn<SettingsPersonalAccessToken>>>(
    () => [
      {
        key: 'name',
        title: i18nText('settings', 'auto.name'),
        dataIndex: 'name',
        width: 200,
        render: (_: unknown, token) => (
          <Typography.Text strong>{token.name}</Typography.Text>
        )
      },
      {
        key: 'token_prefix',
        title: i18nText('settings', 'auto.api_key_prefix'),
        dataIndex: 'token_prefix',
        width: 160,
        render: (_value: unknown, token) => (
          <Typography.Text code>{token.token_prefix}</Typography.Text>
        )
      },
      {
        key: 'status',
        title: i18nText('settings', 'auto.status'),
        width: 120,
        render: (_: unknown, token) =>
          token.revoked || !token.enabled ? (
            <Tag>{i18nText('settings', 'auto.revoked')}</Tag>
          ) : (
            <Tag color="green">{i18nText('settings', 'auto.enabled_alt')}</Tag>
          )
      },
      {
        key: 'expires_at',
        title: i18nText('settings', 'auto.expires'),
        dataIndex: 'expires_at',
        width: 180,
        render: (_value: unknown, token) => formatDateTime(token.expires_at)
      },
      {
        key: 'last_used_at',
        title: i18nText('settings', 'auto.last_used_at'),
        dataIndex: 'last_used_at',
        width: 180,
        render: (_value: unknown, token) => formatLastUsedAt(token.last_used_at)
      },
      {
        key: 'created_at',
        title: i18nText('settings', 'auto.created'),
        dataIndex: 'created_at',
        width: 180,
        render: (_value: unknown, token) => formatDateTime(token.created_at)
      },
      {
        key: 'action',
        title: i18nText('settings', 'auto.operation'),
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
    [revokeMutation.isPending, revokeMutation.mutate]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({
    columns,
    storageKey: 'settings.personal_access_tokens'
  });
  const filteredTokens = useMemo(() => {
    const tokens = tokensQuery.data ?? [];
    if (statusFilter === 'all') return tokens;

    return tokens.filter(
      (token) => personalAccessTokenStatus(token) === statusFilter
    );
  }, [statusFilter, tokensQuery.data]);
  const pagedTokens = useMemo(
    () => filteredTokens.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE),
    [filteredTokens, page]
  );

  const applyFilters = useCallback(() => {
    setStatusFilter(statusFilterDraft);
    setPage(1);
  }, [statusFilterDraft]);
  const resetFilters = useCallback(() => {
    setStatusFilterDraft('active');
    setStatusFilter('active');
    setPage(1);
  }, []);

  return (
    <SettingsSectionSurface heightMode="fill" status={sectionStatus}>
      <DataTableLayout
        filters={
          <DataTableFilterForm
            ariaLabel={i18nText('settings', 'auto.translation_catalog_filter')}
            resetLabel={i18nText('settings', 'auto.reset')}
            submitLabel={i18nText(
              'settings',
              'auto.translation_catalog_filter'
            )}
            onReset={resetFilters}
            onSubmit={applyFilters}
          >
            <DataTableFilterField label={i18nText('settings', 'auto.status')}>
              <Select<PersonalAccessTokenStatusFilter>
                aria-label={i18nText('settings', 'auto.status')}
                value={statusFilterDraft}
                options={[
                  {
                    value: 'all',
                    label: i18nText('settings', 'auto.api_key_status_all')
                  },
                  {
                    value: 'active',
                    label: i18nText('settings', 'auto.api_key_status_active')
                  },
                  {
                    value: 'deleted',
                    label: i18nText('settings', 'auto.revoked')
                  }
                ]}
                onChange={setStatusFilterDraft}
              />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
        <DataTable<SettingsPersonalAccessToken>
          columns={columns}
          configuration={tableConfiguration}
          dataSource={pagedTokens}
          emptyText={i18nText('settings', 'auto.no_user_api_keys')}
          loading={tokensQuery.isLoading || tokensQuery.isFetching}
          page={page}
          pageSize={PAGE_SIZE}
          rowKey="id"
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={() => setCreateModalOpen(true)}
              >
                {i18nText('settings', 'auto.create_api_key')}
              </Button>
              <Button onClick={() => tokensQuery.refetch()}>
                {i18nText('settings', 'auto.refresh')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
          total={filteredTokens.length}
          onPageChange={setPage}
        />
      </DataTableLayout>

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
