import { useEffect, useMemo, useState } from 'react';

import {
  App,
  Alert,
  Button,
  Empty,
  Flex,
  Form,
  Input,
  InputNumber,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography
} from 'antd';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  createSettingsNetworkEgressProxy,
  deleteSettingsNetworkEgressPoolMember,
  fetchSettingsNetworkEgressPools,
  fetchSettingsNetworkEgressProviderTypes,
  settingsNetworkEgressPoolsQueryKey,
  testSettingsNetworkEgressPoolMember,
  updateSettingsNetworkEgressPoolMember,
  type CreateSettingsNetworkEgressProxyInput,
  type SettingsNetworkEgressPoolMember,
  type SettingsNetworkEgressProviderType
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';

type ProxyFormValues = CreateSettingsNetworkEgressProxyInput;
type ProxyMemberEditValues = { enabled: boolean; sequence: number };

function healthTag(health: string) {
  const color = health === 'healthy' ? 'green' : health === 'unhealthy' ? 'red' : 'orange';
  return <Tag color={color}>{i18nText('settings', `auto.network_center_health_${health}`)}</Tag>;
}

function probeCapabilityTag(protocol: 'http' | 'https', status: string) {
  const label = protocol === 'http'
    ? status === 'succeeded'
      ? i18nText('settings', 'auto.network_center_probe_http_available')
      : status === 'failed'
        ? i18nText('settings', 'auto.network_center_probe_http_unavailable')
        : i18nText('settings', 'auto.network_center_probe_http_not_tested')
    : status === 'succeeded'
      ? i18nText('settings', 'auto.network_center_probe_https_available')
      : status === 'failed'
        ? i18nText('settings', 'auto.network_center_probe_https_unavailable')
        : i18nText('settings', 'auto.network_center_probe_https_not_tested');
  return <Tag color={status === 'succeeded' ? 'green' : status === 'failed' ? 'red' : undefined}>{label}</Tag>;
}

function probeErrorText(errorCode: string | null) {
  switch (errorCode) {
    case 'proxy_authentication_failed': return i18nText('settings', 'auto.network_center_probe_error_authentication');
    case 'proxy_timeout': return i18nText('settings', 'auto.network_center_probe_error_timeout');
    case 'proxy_request_rejected': return i18nText('settings', 'auto.network_center_probe_error_rejected');
    case 'https_connect_failed': return i18nText('settings', 'auto.network_center_probe_error_https_connect');
    case 'proxy_unavailable': return i18nText('settings', 'auto.network_center_probe_error_unavailable');
    case 'proxy_release_failed': return i18nText('settings', 'auto.network_center_probe_error_release');
    default: return i18nText('settings', 'auto.network_center_probe_error_http');
  }
}

function ProxyModal({
  open,
  types,
  loading,
  submitting,
  onClose,
  onSubmit
}: {
  open: boolean;
  types: SettingsNetworkEgressProviderType[];
  loading: boolean;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (values: ProxyFormValues) => void;
}) {
  const [form] = Form.useForm<ProxyFormValues>();
  const providerCode = Form.useWatch('provider_code', form);
  const proxyType = types.find((item) => item.provider_code === providerCode);
  return (
    <FixedHeightModal
      open={open}
      title={i18nText('settings', 'auto.network_center_member_create')}
      onCancel={onClose}
      onOk={() => form.submit()}
      confirmLoading={submitting}
      destroyOnHidden
      width={640}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{ description: '', config: {} }}
        onFinish={onSubmit}
      >
        <Form.Item
          name="provider_code"
          label={i18nText('settings', 'auto.network_center_providers')}
          rules={[{ required: true }]}
        >
          <Select
            loading={loading}
            options={types.map((item) => ({ value: item.provider_code, label: item.display_name }))}
          />
        </Form.Item>
        <Form.Item
          name="display_name"
          label={i18nText('settings', 'auto.name')}
          rules={[{ required: true }]}
        >
          <Input />
        </Form.Item>
        <Form.Item name="description" label={i18nText('settings', 'auto.description')}>
          <Input.TextArea rows={2} />
        </Form.Item>
        {proxyType?.form_schema.fields.map((field) => (
          <Form.Item
            key={field.key}
            name={['config', field.key]}
            label={field.label}
            extra={field.description}
            rules={[{ required: field.required }]}
          >
            {field.key.toLowerCase().includes('password') ? <Input.Password /> : <Input />}
          </Form.Item>
        ))}
      </Form>
    </FixedHeightModal>
  );
}

function ProxyMemberEditModal({
  member,
  submitting,
  onClose,
  onSubmit
}: {
  member: SettingsNetworkEgressPoolMember | null;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (values: ProxyMemberEditValues) => void;
}) {
  const [form] = Form.useForm<ProxyMemberEditValues>();
  useEffect(() => {
    if (member) form.setFieldsValue({ enabled: member.enabled, sequence: member.sequence });
  }, [form, member]);
  return (
    <FixedHeightModal
      open={member !== null}
      title={i18nText('settings', 'auto.network_center_member_edit')}
      onCancel={onClose}
      onOk={() => form.submit()}
      confirmLoading={submitting}
      destroyOnHidden
      width={520}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{ enabled: member?.enabled, sequence: member?.sequence }}
        onFinish={onSubmit}
      >
        <Form.Item
          name="sequence"
          label={i18nText('settings', 'auto.network_center_member_sequence')}
          rules={[{ required: true }]}
        >
          <InputNumber min={0} precision={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item
          name="enabled"
          label={i18nText('settings', 'auto.network_center_member_enabled')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
      </Form>
    </FixedHeightModal>
  );
}

export function NetworkEgressPoolsPanel() {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [health, setHealth] = useState<string>();
  const [providerCode, setProviderCode] = useState<string>();
  const [editingMember, setEditingMember] = useState<SettingsNetworkEgressPoolMember | null>(null);
  const pools = useQuery({ queryKey: settingsNetworkEgressPoolsQueryKey, queryFn: fetchSettingsNetworkEgressPools });
  const types = useQuery({ queryKey: ['settings', 'network-center', 'provider-types'], queryFn: fetchSettingsNetworkEgressProviderTypes });
  const pool = pools.data?.[0];
  const invalidate = () => queryClient.invalidateQueries({ queryKey: settingsNetworkEgressPoolsQueryKey });
  const create = useMutation({
    mutationFn: (input: ProxyFormValues) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressProxy(input, csrfToken);
    },
    onSuccess: async () => {
      await Promise.all([
        invalidate(),
        queryClient.invalidateQueries({ queryKey: ['settings', 'network-center', 'providers'] })
      ]);
      setCreateOpen(false);
    },
    onError: () => message.error(i18nText('settings', 'auto.network_center_proxy_create_failed'))
  });
  const updateMember = useMutation({
    mutationFn: ({ memberId, input }: { memberId: string; input: ProxyMemberEditValues }) => {
      if (!csrfToken || !pool) throw new Error('Missing proxy pool context');
      return updateSettingsNetworkEgressPoolMember(pool.id, memberId, input, csrfToken);
    },
    onSuccess: async () => {
      await invalidate();
      setEditingMember(null);
    }
  });
  const removeMember = useMutation({
    mutationFn: (memberId: string) => {
      if (!csrfToken || !pool) throw new Error('Missing proxy pool context');
      return deleteSettingsNetworkEgressPoolMember(pool.id, memberId, csrfToken);
    },
    onSuccess: invalidate
  });
  const testMember = useMutation({
    mutationFn: (memberId: string) => {
      if (!csrfToken || !pool) throw new Error('Missing proxy pool context');
      return testSettingsNetworkEgressPoolMember(pool.id, memberId, csrfToken);
    },
    onSuccess: invalidate,
    onError: () => message.error(i18nText('settings', 'auto.network_center_member_test_failed'))
  });
  const typeDisplayNameByCode = useMemo(
    () => new Map((types.data ?? []).map((type) => [type.provider_code, type.display_name])),
    [types.data]
  );
  const members = useMemo(() => (pool?.members ?? []).filter((member) => {
    const terms = `${member.display_name} ${typeDisplayNameByCode.get(member.provider_code) ?? member.provider_code} ${member.address_summary ?? ''}`.toLowerCase();
    return (!search || terms.includes(search.trim().toLowerCase()))
      && (!health || member.health === health)
      && (!providerCode || member.provider_code === providerCode);
  }), [health, pool?.members, providerCode, search, typeDisplayNameByCode]);

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16} data-testid="network-center-pools-shell">
        <Flex justify="space-between" gap={12} wrap>
          <Space wrap>
            <Input.Search
              allowClear
              placeholder={i18nText('settings', 'auto.network_center_proxy_search')}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              style={{ width: 240 }}
            />
            <Select
              allowClear
              placeholder={i18nText('settings', 'auto.network_center_member_health')}
              value={health}
              onChange={setHealth}
              style={{ width: 160 }}
              options={['healthy', 'unhealthy', 'invalid'].map((value) => ({ value, label: i18nText('settings', `auto.network_center_health_${value}`) }))}
            />
            <Select
              allowClear
              placeholder={i18nText('settings', 'auto.network_center_providers')}
              value={providerCode}
              onChange={setProviderCode}
              style={{ width: 180 }}
              options={types.data?.map((type) => ({ value: type.provider_code, label: type.display_name }))}
            />
          </Space>
          <Button type="primary" onClick={() => setCreateOpen(true)}>
            {i18nText('settings', 'auto.network_center_member_create')}
          </Button>
        </Flex>
        {pools.isError ? (
          <Alert type="error" showIcon title={i18nText('settings', 'auto.network_center_pools_load_failed')} />
        ) : (
          <Table
            rowKey="id"
            loading={pools.isLoading}
            dataSource={members}
            pagination={false}
            scroll={{ x: 1160 }}
            locale={{ emptyText: <Empty description={i18nText('settings', 'auto.network_center_no_pools')} /> }}
            columns={[
              {
                title: i18nText('settings', 'auto.name'),
                key: 'name',
                dataIndex: 'display_name'
              },
              {
                title: i18nText('settings', 'auto.network_center_providers'),
                dataIndex: 'provider_code',
                render: (providerCode) => <Typography.Text type="secondary">{typeDisplayNameByCode.get(providerCode) ?? providerCode}</Typography.Text>
              },
              {
                title: i18nText('settings', 'auto.network_center_proxy_address'),
                key: 'address',
                render: (_, member) => member.address_summary
                  ? <Typography.Text copyable={{ text: member.address_summary }} style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace' }}>{member.address_summary}</Typography.Text>
                  : <Typography.Text type="secondary">{member.provider_egress_key}</Typography.Text>
              },
              {
                title: i18nText('settings', 'auto.network_center_provider_egress_region'),
                dataIndex: 'region',
                render: (region) => region ?? '-'
              },
              {
                title: i18nText('settings', 'auto.network_center_member_health'),
                dataIndex: 'health',
                render: healthTag
              },
              {
                title: i18nText('settings', 'auto.network_center_probe_latency'),
                dataIndex: 'probe_latency_ms',
                render: (latencyMs: number) => <Typography.Text>{latencyMs}ms</Typography.Text>
              },
              {
                title: i18nText('settings', 'auto.network_center_member_test_result'),
                key: 'probe',
                render: (_, member) => member.probe_status === 'not_tested'
                  ? <Typography.Text type="secondary">-</Typography.Text>
                  : <Space orientation="vertical" size={0}>
                    <Space size={4} wrap>
                      {probeCapabilityTag('http', member.probe_http_status)}
                      {probeCapabilityTag('https', member.probe_https_status)}
                    </Space>
                    {member.probe_exit_ip ? <Typography.Text type="secondary">{member.probe_exit_ip}</Typography.Text> : null}
                    {member.probe_error_code ? <Typography.Text type="danger">{probeErrorText(member.probe_error_code)}</Typography.Text> : null}
                    {member.last_probed_at ? <Typography.Text type="secondary">{new Date(member.last_probed_at).toLocaleString()}</Typography.Text> : null}
                  </Space>
              },
              {
                title: i18nText('settings', 'auto.status'),
                dataIndex: 'enabled',
                render: (enabled, member) => <Switch checked={enabled} loading={updateMember.isPending} onChange={(next) => updateMember.mutate({ memberId: member.id, input: { enabled: next, sequence: member.sequence } })} />
              },
              {
                title: i18nText('settings', 'auto.operation'),
                render: (_, member) => <Space>
                  <Button type="link" loading={testMember.isPending} onClick={() => testMember.mutate(member.id)}>{i18nText('settings', 'auto.network_center_member_test')}</Button>
                  <Button type="link" onClick={() => setEditingMember(member)}>{i18nText('settings', 'auto.edit')}</Button>
                  <Popconfirm title={i18nText('settings', 'auto.network_center_member_delete_confirm')} onConfirm={() => removeMember.mutate(member.id)}>
                    <Button type="link" danger loading={removeMember.isPending}>{i18nText('settings', 'auto.delete')}</Button>
                  </Popconfirm>
                </Space>
              }
            ]}
          />
        )}
      </Flex>
      <ProxyModal
        open={createOpen}
        types={types.data ?? []}
        loading={types.isLoading}
        submitting={create.isPending}
        onClose={() => setCreateOpen(false)}
        onSubmit={(values) => create.mutate(values)}
      />
      <ProxyMemberEditModal
        member={editingMember}
        submitting={updateMember.isPending}
        onClose={() => setEditingMember(null)}
        onSubmit={(input) => {
          if (editingMember) updateMember.mutate({ memberId: editingMember.id, input });
        }}
      />
    </SettingsSectionSurface>
  );
}
