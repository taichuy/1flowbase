import { useEffect, useMemo, useState } from 'react';

import { App, Alert, Button, Empty, Flex, Form, Input, InputNumber, Popconfirm, Select, Space, Switch, Tag, Typography } from 'antd';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { createSettingsNetworkEgressProxy, deleteSettingsNetworkEgressPoolMember, fetchSettingsNetworkEgressPools, fetchSettingsNetworkEgressProviderTypes, settingsNetworkEgressPoolsQueryKey, testSettingsNetworkEgressPoolMember, updateSettingsNetworkEgressPoolMember, type CreateSettingsNetworkEgressProxyInput, type SettingsNetworkEgressPoolMember, type SettingsNetworkEgressProviderType } from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { FixedHeightModal } from '../../../../shared/ui/fixed-height-modal/FixedHeightModal';
import { DataTable, DataTableColumnSettings, type DataTableColumn } from '../../../../shared/ui/data-table/DataTable';
import { DataTableFilterField, DataTableFilterForm, DataTableLayout } from '../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import './network-egress-pools.css';

type ProxyFormValues = CreateSettingsNetworkEgressProxyInput;
type ProxyMemberEditValues = { enabled: boolean; sequence: number };
type ProxyPoolFilters = { search: string; health?: string; providerCode?: string };

const PAGE_SIZE = 20;

function healthTag(health: string) {
  const color = health === 'healthy' ? 'green' : health === 'unhealthy' ? 'red' : health === 'not_tested' ? 'default' : 'orange';
  return <Tag color={color}>{i18nText('settings', `auto.network_center_health_${health}`)}</Tag>;
}

function probeCapabilityTag(protocol: 'http' | 'https', status: string) {
  const label = protocol === 'http' ? (status === 'succeeded' ? i18nText('settings', 'auto.network_center_probe_http_available') : status === 'failed' ? i18nText('settings', 'auto.network_center_probe_http_unavailable') : i18nText('settings', 'auto.network_center_probe_http_not_tested')) : status === 'succeeded' ? i18nText('settings', 'auto.network_center_probe_https_available') : status === 'failed' ? i18nText('settings', 'auto.network_center_probe_https_unavailable') : i18nText('settings', 'auto.network_center_probe_https_not_tested');
  return <Tag color={status === 'succeeded' ? 'green' : status === 'failed' ? 'red' : undefined}>{label}</Tag>;
}

function probeErrorText(errorCode: string | null) {
  switch (errorCode) {
    case 'proxy_authentication_failed':
      return i18nText('settings', 'auto.network_center_probe_error_authentication');
    case 'proxy_timeout':
      return i18nText('settings', 'auto.network_center_probe_error_timeout');
    case 'proxy_request_rejected':
      return i18nText('settings', 'auto.network_center_probe_error_rejected');
    case 'https_connect_failed':
      return i18nText('settings', 'auto.network_center_probe_error_https_connect');
    case 'proxy_unavailable':
      return i18nText('settings', 'auto.network_center_probe_error_unavailable');
    case 'proxy_release_failed':
      return i18nText('settings', 'auto.network_center_probe_error_release');
    default:
      return i18nText('settings', 'auto.network_center_probe_error_http');
  }
}

function ProxyModal({ open, types, loading, submitting, onClose, onSubmit }: { open: boolean; types: SettingsNetworkEgressProviderType[]; loading: boolean; submitting: boolean; onClose: () => void; onSubmit: (values: ProxyFormValues) => void }) {
  const [form] = Form.useForm<ProxyFormValues>();
  const providerCode = Form.useWatch('provider_code', form);
  const proxyType = types.find((item) => item.provider_code === providerCode);
  return (
    <FixedHeightModal open={open} title={i18nText('settings', 'auto.network_center_member_create')} onCancel={onClose} onOk={() => form.submit()} confirmLoading={submitting} okText={i18nText('settings', 'auto.save')} destroyOnHidden width={640}>
      <Form form={form} layout="vertical" initialValues={{ description: '', config: {} }} onFinish={onSubmit}>
        <Form.Item name="provider_code" label={i18nText('settings', 'auto.network_center_providers')} rules={[{ required: true }]}>
          <Select
            loading={loading}
            options={types.map((item) => ({
              value: item.provider_code,
              label: item.display_name
            }))}
          />
        </Form.Item>
        <Form.Item name="display_name" label={i18nText('settings', 'auto.name')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="description" label={i18nText('settings', 'auto.description')}>
          <Input.TextArea rows={2} />
        </Form.Item>
        {proxyType?.form_schema.fields.map((field) => (
          <Form.Item key={field.key} name={['config', field.key]} label={field.label} extra={field.description} rules={[{ required: field.required }]}>
            {field.key.toLowerCase().includes('password') ? <Input.Password /> : <Input />}
          </Form.Item>
        ))}
      </Form>
    </FixedHeightModal>
  );
}

function ProxyMemberEditModal({ member, submitting, onClose, onSubmit }: { member: SettingsNetworkEgressPoolMember | null; submitting: boolean; onClose: () => void; onSubmit: (values: ProxyMemberEditValues) => void }) {
  const [form] = Form.useForm<ProxyMemberEditValues>();
  useEffect(() => {
    if (member)
      form.setFieldsValue({
        enabled: member.enabled,
        sequence: member.sequence
      });
  }, [form, member]);
  return (
    <FixedHeightModal open={member !== null} title={i18nText('settings', 'auto.network_center_member_edit')} onCancel={onClose} onOk={() => form.submit()} confirmLoading={submitting} destroyOnHidden width={520}>
      <Form form={form} layout="vertical" initialValues={{ enabled: member?.enabled, sequence: member?.sequence }} onFinish={onSubmit}>
        <Form.Item name="sequence" label={i18nText('settings', 'auto.network_center_member_sequence')} rules={[{ required: true }]}>
          <InputNumber min={0} precision={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="enabled" label={i18nText('settings', 'auto.network_center_member_enabled')} valuePropName="checked">
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
  const [filterDraft, setFilterDraft] = useState<ProxyPoolFilters>({ search: '' });
  const [filters, setFilters] = useState<ProxyPoolFilters>({ search: '' });
  const [page, setPage] = useState(1);
  const [editingMember, setEditingMember] = useState<SettingsNetworkEgressPoolMember | null>(null);
  const [testingMemberIds, setTestingMemberIds] = useState<ReadonlySet<string>>(() => new Set());
  const pools = useQuery({
    queryKey: settingsNetworkEgressPoolsQueryKey,
    queryFn: fetchSettingsNetworkEgressPools
  });
  const types = useQuery({
    queryKey: ['settings', 'network-center', 'provider-types'],
    queryFn: fetchSettingsNetworkEgressProviderTypes
  });
  const pool = pools.data?.[0];
  const invalidate = () =>
    queryClient.invalidateQueries({
      queryKey: settingsNetworkEgressPoolsQueryKey
    });
  const create = useMutation({
    mutationFn: (input: ProxyFormValues) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressProxy(input, csrfToken);
    },
    onSuccess: async () => {
      await Promise.all([
        invalidate(),
        queryClient.invalidateQueries({
          queryKey: ['settings', 'network-center', 'providers']
        })
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
  const runMemberTest = (memberId: string) => {
    if (!csrfToken || !pool) throw new Error('Missing proxy pool context');
    return testSettingsNetworkEgressPoolMember(pool.id, memberId, csrfToken);
  };
  const testMember = useMutation({
    mutationFn: runMemberTest,
    onMutate: (memberId) => {
      setTestingMemberIds((current) => new Set(current).add(memberId));
    },
    onSuccess: invalidate,
    onSettled: (_, __, memberId) => {
      setTestingMemberIds((current) => {
        const next = new Set(current);
        next.delete(memberId);
        return next;
      });
    },
    onError: () => {
      message.error(i18nText('settings', 'auto.network_center_member_test_failed'));
    }
  });
  const testCurrentPage = useMutation({
    mutationFn: async (memberIds: string[]) => {
      const results = await Promise.allSettled(memberIds.map(runMemberTest));
      if (results.some((result) => result.status === 'rejected')) {
        throw new Error('One or more proxy member tests failed');
      }
    },
    onMutate: (memberIds) => {
      setTestingMemberIds((current) => {
        const next = new Set(current);
        memberIds.forEach((memberId) => next.add(memberId));
        return next;
      });
    },
    onSettled: async (_, __, memberIds) => {
      setTestingMemberIds((current) => {
        const next = new Set(current);
        memberIds.forEach((memberId) => next.delete(memberId));
        return next;
      });
      await invalidate();
    },
    onError: () => {
      message.error(i18nText('settings', 'auto.network_center_member_test_failed'));
    }
  });
  const updateMemberAction = updateMember.mutate;
  const removeMemberAction = removeMember.mutate;
  const testMemberAction = testMember.mutate;
  const typeDisplayNameByCode = useMemo(() => new Map((types.data ?? []).map((type) => [type.provider_code, type.display_name])), [types.data]);
  const members = useMemo(
    () =>
      (pool?.members ?? []).filter((member) => {
        const terms = `${member.display_name} ${typeDisplayNameByCode.get(member.provider_code) ?? member.provider_code} ${member.address_summary ?? ''}`.toLowerCase();
        return (!filters.search || terms.includes(filters.search.trim().toLowerCase())) && (!filters.health || member.health === filters.health) && (!filters.providerCode || member.provider_code === filters.providerCode);
      }),
    [filters, pool?.members, typeDisplayNameByCode]
  );
  const pagedMembers = useMemo(() => members.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE), [members, page]);
  const applyFilters = () => {
    setFilters({ ...filterDraft, search: filterDraft.search.trim() });
    setPage(1);
  };
  const resetFilters = () => {
    const next = { search: '' };
    setFilterDraft(next);
    setFilters(next);
    setPage(1);
  };
  const columns = useMemo<Array<DataTableColumn<SettingsNetworkEgressPoolMember>>>(
    () => [
      { key: 'name', title: i18nText('settings', 'auto.name'), dataIndex: 'display_name', width: 160 },
      { key: 'provider', title: i18nText('settings', 'auto.network_center_providers'), width: 150, render: (_, member) => <Typography.Text type="secondary">{typeDisplayNameByCode.get(member.provider_code) ?? member.provider_code}</Typography.Text> },
      { key: 'address', title: i18nText('settings', 'auto.network_center_proxy_address'), width: 230, render: (_, member) => member.address_summary ? <Typography.Text copyable={{ text: member.address_summary }} style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace' }}>{member.address_summary}</Typography.Text> : <Typography.Text type="secondary">{member.provider_egress_key}</Typography.Text> },
      { key: 'region', title: i18nText('settings', 'auto.network_center_provider_egress_region'), dataIndex: 'region', width: 140, render: (_, member) => member.region ?? '-' },
      { key: 'health', title: i18nText('settings', 'auto.network_center_member_health'), dataIndex: 'health', width: 130, render: (_, member) => healthTag(member.health) },
      { key: 'latency', title: i18nText('settings', 'auto.network_center_probe_latency'), dataIndex: 'probe_latency_ms', width: 100, render: (latencyMs) => <Typography.Text>{latencyMs as number}ms</Typography.Text> },
      { key: 'probe', title: i18nText('settings', 'auto.network_center_member_test_result'), width: 280, sizing: 'fill', render: (_, member) => member.probe_status === 'not_tested' ? <Space size={4} wrap>{probeCapabilityTag('http', 'not_tested')}{probeCapabilityTag('https', 'not_tested')}</Space> : <Space orientation="vertical" size={0}><Space size={4} wrap>{probeCapabilityTag('http', member.probe_http_status)}{probeCapabilityTag('https', member.probe_https_status)}</Space>{member.probe_exit_ip ? <Typography.Text type="secondary">{member.probe_exit_ip}</Typography.Text> : null}{member.probe_error_code ? <Typography.Text type="danger">{probeErrorText(member.probe_error_code)}</Typography.Text> : null}{member.last_probed_at ? <Typography.Text type="secondary">{new Date(member.last_probed_at).toLocaleString()}</Typography.Text> : null}</Space> },
      { key: 'enabled', title: i18nText('settings', 'auto.status'), dataIndex: 'enabled', width: 100, align: 'center', render: (enabled, member) => <Switch checked={enabled as boolean} loading={updateMember.isPending} onChange={(next) => updateMemberAction({ memberId: member.id, input: { enabled: next, sequence: member.sequence } })} /> },
      { key: 'actions', title: i18nText('settings', 'auto.operation'), width: 180, minWidth: 180, align: 'center', render: (_, member) => <Space><Button type="link" loading={testingMemberIds.has(member.id)} onClick={() => testMemberAction(member.id)}>{i18nText('settings', 'auto.network_center_member_test')}</Button><Button type="link" onClick={() => setEditingMember(member)}>{i18nText('settings', 'auto.edit')}</Button><Popconfirm title={i18nText('settings', 'auto.network_center_member_delete_confirm')} onConfirm={() => removeMemberAction(member.id)}><Button type="link" danger loading={removeMember.isPending}>{i18nText('settings', 'auto.delete')}</Button></Popconfirm></Space> }
    ],
    [removeMember.isPending, removeMemberAction, testMemberAction, testingMemberIds, typeDisplayNameByCode, updateMember.isPending, updateMemberAction]
  );
  const tableConfiguration = usePersistedDataTableConfiguration({ columns, storageKey: 'settings.network_egress_pools' });

  return (
    <SettingsSectionSurface heightMode="fill">
      <div className="network-center-pools-shell" data-testid="network-center-pools-shell">
        <DataTableLayout
          filters={
            <DataTableFilterForm
              ariaLabel={i18nText('settings', 'auto.network_center_proxy_search')}
              resetLabel={i18nText('settings', 'auto.reset')}
              submitLabel={i18nText('settings', 'auto.network_center_proxy_search')}
              onReset={resetFilters}
              onSubmit={applyFilters}
            >
              <DataTableFilterField label={i18nText('settings', 'auto.network_center_proxy_search')}>
                <Input allowClear type="search" value={filterDraft.search} onChange={(event) => setFilterDraft((current) => ({ ...current, search: event.target.value }))} />
              </DataTableFilterField>
              <DataTableFilterField label={i18nText('settings', 'auto.network_center_member_health')}>
                <Select allowClear value={filterDraft.health} onChange={(health) => setFilterDraft((current) => ({ ...current, health }))} options={['healthy', 'not_tested', 'unhealthy', 'invalid'].map((value) => ({ value, label: i18nText('settings', `auto.network_center_health_${value}`) }))} />
              </DataTableFilterField>
              <DataTableFilterField label={i18nText('settings', 'auto.network_center_providers')}>
                <Select allowClear value={filterDraft.providerCode} onChange={(providerCode) => setFilterDraft((current) => ({ ...current, providerCode }))} options={types.data?.map((type) => ({ value: type.provider_code, label: type.display_name }))} />
              </DataTableFilterField>
            </DataTableFilterForm>
          }
        >
          {pools.isError ? <Alert type="error" showIcon title={i18nText('settings', 'auto.network_center_pools_load_failed')} /> : <DataTable<SettingsNetworkEgressPoolMember> columns={columns} configuration={tableConfiguration} dataSource={pagedMembers} emptyText={<Empty description={i18nText('settings', 'auto.network_center_no_pools')} />} loading={pools.isLoading || pools.isFetching} page={page} pageSize={PAGE_SIZE} rowKey="id" total={members.length} onPageChange={setPage} toolbar={<Flex justify="flex-end" gap={8} wrap><Button disabled={pagedMembers.length === 0 || testCurrentPage.isPending} loading={testCurrentPage.isPending} onClick={() => testCurrentPage.mutate(pagedMembers.map((member) => member.id))}>{i18nText('settings', 'auto.network_center_member_test')}</Button><Button type="primary" onClick={() => setCreateOpen(true)}>{i18nText('settings', 'auto.network_center_member_create')}</Button><Button onClick={() => pools.refetch()}>{i18nText('settings', 'auto.refresh')}</Button><DataTableColumnSettings columns={columns} configuration={tableConfiguration} /></Flex>} />}
        </DataTableLayout>
      </div>
      <ProxyModal open={createOpen} types={types.data ?? []} loading={types.isLoading} submitting={create.isPending} onClose={() => setCreateOpen(false)} onSubmit={(values) => create.mutate(values)} />
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
