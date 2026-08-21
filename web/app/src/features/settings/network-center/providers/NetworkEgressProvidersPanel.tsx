import { useEffect, useState } from 'react';

import {
  App,
  Alert,
  Button,
  Empty,
  Flex,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  createSettingsNetworkEgressProvider,
  fetchSettingsNetworkEgressProviderTypes,
  fetchSettingsNetworkEgressProviders,
  settingsNetworkEgressProvidersQueryKey,
  syncSettingsNetworkEgressProvider,
  updateSettingsNetworkEgressProviderLifecycle,
  type CreateSettingsNetworkEgressProviderInput,
  type SettingsNetworkEgressProvider,
  type SettingsNetworkEgressProviderType
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';

function providerStatusTag(status: string) {
  const color =
    status === 'active'
      ? 'green'
      : status === 'disabled'
        ? 'default'
        : status === 'draft'
          ? 'orange'
          : undefined;

  const label =
    status === 'active'
      ? i18nText('settings', 'auto.network_center_provider_lifecycle_active')
      : status === 'disabled'
        ? i18nText(
            'settings',
            'auto.network_center_provider_lifecycle_disabled'
          )
        : status === 'draft'
          ? i18nText('settings', 'auto.network_center_provider_lifecycle_draft')
          : status;

  return <Tag color={color}>{label}</Tag>;
}

function healthStatusTag(status: string) {
  const color =
    status === 'healthy'
      ? 'green'
      : status === 'unhealthy'
        ? 'red'
        : status === 'invalid'
          ? 'orange'
          : undefined;

  const label =
    status === 'healthy'
      ? i18nText('settings', 'auto.network_center_health_healthy')
      : status === 'unhealthy'
        ? i18nText('settings', 'auto.network_center_health_unhealthy')
        : status === 'invalid'
          ? i18nText('settings', 'auto.network_center_health_invalid')
          : status;

  return <Tag color={color}>{label}</Tag>;
}

function ProviderRegistrationModal({
  open,
  submitting,
  providerTypes,
  providerTypesLoading,
  onClose,
  onSubmit
}: {
  open: boolean;
  submitting: boolean;
  providerTypes: SettingsNetworkEgressProviderType[];
  providerTypesLoading: boolean;
  onClose: () => void;
  onSubmit: (values: CreateSettingsNetworkEgressProviderInput) => void;
}) {
  const [form] = Form.useForm<{
    installation_id: string;
    display_name: string;
    description: string;
    config: Record<string, string>;
  }>();
  const installationId = Form.useWatch('installation_id', form);
  const selectedType = providerTypes.find(
    (providerType) => providerType.installation_id === installationId
  );

  useEffect(() => {
    if (open && providerTypes.length === 1 && !installationId) {
      form.setFieldValue('installation_id', providerTypes[0].installation_id);
    }
  }, [form, installationId, open, providerTypes]);

  return (
    <Modal
      title={i18nText('settings', 'auto.network_center_provider_register')}
      open={open}
      onCancel={onClose}
      destroyOnHidden
      okText={i18nText('settings', 'auto.create')}
      confirmLoading={submitting}
      onOk={() => form.submit()}
    >
      <Form
        form={form}
        layout="vertical"
        onFinish={(values) =>
          onSubmit({
            ...values,
            description: values.description ?? '',
            config: values.config ?? {}
          })
        }
        preserve={false}
      >
        <Form.Item
          name="installation_id"
          label={i18nText(
            'settings',
            'auto.network_center_provider_installation'
          )}
          rules={[{ required: true }]}
        >
          <Select
            autoFocus
            loading={providerTypesLoading}
            options={providerTypes.map((providerType) => ({
              label: providerType.display_name,
              value: providerType.installation_id
            }))}
            notFoundContent={i18nText(
              'settings',
              'auto.network_center_provider_no_types'
            )}
          />
        </Form.Item>
        <Form.Item
          name="display_name"
          label={i18nText('settings', 'auto.network_center_provider_name')}
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item
          name="description"
          label={i18nText('settings', 'auto.description')}
        >
          <Input.TextArea autoSize={{ minRows: 2, maxRows: 4 }} />
        </Form.Item>
        {selectedType?.form_schema.fields.map((field) => (
          <Form.Item
            key={field.key}
            name={['config', field.key]}
            label={field.label}
            help={field.description}
            rules={[{ required: field.required }]}
          >
            <Input
              placeholder={field.placeholder}
              type={field.control === 'url' ? 'url' : 'text'}
            />
          </Form.Item>
        ))}
        {selectedType ? null : (
          <Form.Item
            label={i18nText('settings', 'auto.provider_configuration')}
            help={i18nText(
              'settings',
              'auto.network_center_provider_secret_reference_help'
            )}
          >
            <Typography.Text type="secondary">
              {i18nText('settings', 'auto.network_center_provider_no_types')}
            </Typography.Text>
          </Form.Item>
        )}
      </Form>
    </Modal>
  );
}

function EgressList({ provider }: { provider: SettingsNetworkEgressProvider }) {
  if (provider.egresses.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText(
          'settings',
          'auto.network_center_provider_no_egresses'
        )}
      />
    );
  }

  return (
    <Table
      rowKey="provider_egress_key"
      size="small"
      pagination={false}
      dataSource={provider.egresses}
      columns={[
        {
          title: i18nText(
            'settings',
            'auto.network_center_provider_egress_name'
          ),
          dataIndex: 'display_name',
          key: 'display_name'
        },
        {
          title: i18nText(
            'settings',
            'auto.network_center_provider_egress_key'
          ),
          dataIndex: 'provider_egress_key',
          key: 'provider_egress_key'
        },
        {
          title: i18nText(
            'settings',
            'auto.network_center_provider_egress_region'
          ),
          dataIndex: 'region',
          key: 'region',
          render: (region: string | null) => region ?? '—'
        },
        {
          title: i18nText(
            'settings',
            'auto.network_center_provider_egress_availability'
          ),
          dataIndex: 'availability',
          key: 'availability'
        }
      ]}
    />
  );
}

export function NetworkEgressProvidersPanel() {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const providersQuery = useQuery({
    queryKey: settingsNetworkEgressProvidersQueryKey,
    queryFn: fetchSettingsNetworkEgressProviders
  });
  const providerTypesQuery = useQuery({
    queryKey: ['settings', 'network-center', 'provider-types'],
    queryFn: fetchSettingsNetworkEgressProviderTypes
  });
  const [registrationOpen, setRegistrationOpen] = useState(false);
  const invalidateProviders = () =>
    queryClient.invalidateQueries({
      queryKey: settingsNetworkEgressProvidersQueryKey
    });
  const createMutation = useMutation({
    mutationFn: (input: CreateSettingsNetworkEgressProviderInput) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressProvider(input, csrfToken);
    },
    onSuccess: async () => {
      await invalidateProviders();
      setRegistrationOpen(false);
    },
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_provider_register_failed')
      )
  });
  const lifecycleMutation = useMutation({
    mutationFn: ({
      providerId,
      lifecycle
    }: {
      providerId: string;
      lifecycle: string;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return updateSettingsNetworkEgressProviderLifecycle(
        providerId,
        { lifecycle },
        csrfToken
      );
    },
    onSuccess: invalidateProviders,
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_provider_lifecycle_failed')
      )
  });
  const syncMutation = useMutation({
    mutationFn: (providerId: string) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return syncSettingsNetworkEgressProvider(providerId, csrfToken);
    },
    onSuccess: invalidateProviders,
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_provider_sync_failed')
      )
  });

  const columns: ColumnsType<SettingsNetworkEgressProvider> = [
    {
      title: i18nText('settings', 'auto.network_center_provider_name'),
      dataIndex: 'display_name',
      key: 'display_name',
      render: (displayName: string, provider) => (
        <Space orientation="vertical" size={0}>
          <Typography.Text>{displayName}</Typography.Text>
          <Typography.Text type="secondary">
            {provider.provider_code}
          </Typography.Text>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.network_center_provider_lifecycle'),
      dataIndex: 'lifecycle',
      key: 'lifecycle',
      render: providerStatusTag
    },
    {
      title: i18nText('settings', 'auto.network_center_provider_health'),
      dataIndex: 'health_status',
      key: 'health_status',
      render: healthStatusTag
    },
    {
      title: i18nText('settings', 'auto.network_center_provider_secret'),
      dataIndex: 'secret_configured',
      key: 'secret_configured',
      render: (configured: boolean) =>
        configured
          ? i18nText('settings', 'auto.yes')
          : i18nText('settings', 'auto.not_configured')
    },
    {
      title: i18nText('settings', 'auto.network_center_provider_last_synced'),
      dataIndex: 'last_synced_at',
      key: 'last_synced_at',
      render: (lastSyncedAt: string | null) => lastSyncedAt ?? '—'
    },
    {
      title: i18nText('settings', 'auto.network_center_provider_sync_error'),
      dataIndex: 'last_sync_error',
      key: 'last_sync_error',
      render: (lastSyncError: string | null) => lastSyncError ?? '—'
    },
    {
      title: '',
      key: 'actions',
      width: 190,
      render: (_, provider) => (
        <Space size={0} wrap>
          {provider.lifecycle === 'active' ? (
            <Button
              type="link"
              loading={lifecycleMutation.isPending}
              onClick={() =>
                lifecycleMutation.mutate({
                  providerId: provider.id,
                  lifecycle: 'disabled'
                })
              }
            >
              {i18nText('settings', 'auto.network_center_provider_disable')}
            </Button>
          ) : (
            <Button
              type="link"
              loading={lifecycleMutation.isPending}
              onClick={() =>
                lifecycleMutation.mutate({
                  providerId: provider.id,
                  lifecycle: 'active'
                })
              }
            >
              {i18nText('settings', 'auto.network_center_provider_start')}
            </Button>
          )}
          <Button
            type="link"
            disabled={provider.lifecycle !== 'active'}
            loading={syncMutation.isPending}
            onClick={() => syncMutation.mutate(provider.id)}
          >
            {i18nText('settings', 'auto.network_center_provider_sync')}
          </Button>
        </Space>
      )
    }
  ];

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Flex
          justify="flex-end"
          align="center"
          gap={16}
          data-testid="network-center-providers-shell"
        >
          <Button type="primary" onClick={() => setRegistrationOpen(true)}>
            {i18nText('settings', 'auto.network_center_provider_register')}
          </Button>
        </Flex>
        {providersQuery.isError ? (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'settings',
              'auto.network_center_providers_load_failed'
            )}
          />
        ) : providersQuery.data?.length === 0 && !providersQuery.isLoading ? (
          <Empty
            description={i18nText(
              'settings',
              'auto.network_center_no_providers'
            )}
          />
        ) : (
          <Table
            rowKey="id"
            columns={columns}
            dataSource={providersQuery.data ?? []}
            loading={providersQuery.isLoading}
            pagination={false}
            scroll={{ x: 1000 }}
            expandable={{
              expandedRowRender: (provider) => (
                <EgressList provider={provider} />
              ),
              rowExpandable: (provider) => provider.egresses.length > 0
            }}
          />
        )}
      </Flex>
      <ProviderRegistrationModal
        open={registrationOpen}
        submitting={createMutation.isPending}
        providerTypes={providerTypesQuery.data ?? []}
        providerTypesLoading={providerTypesQuery.isLoading}
        onClose={() => setRegistrationOpen(false)}
        onSubmit={(values) => createMutation.mutate(values)}
      />
    </SettingsSectionSurface>
  );
}
