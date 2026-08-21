import { useState } from 'react';

import {
  App,
  Alert,
  Button,
  Empty,
  Flex,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  createSettingsNetworkEgressPool,
  createSettingsNetworkEgressPoolMember,
  deleteSettingsNetworkEgressPool,
  deleteSettingsNetworkEgressPoolMember,
  fetchSettingsNetworkEgressPools,
  settingsNetworkEgressPoolsQueryKey,
  updateSettingsNetworkEgressPool,
  updateSettingsNetworkEgressPoolMember,
  type CreateSettingsNetworkEgressPoolInput,
  type CreateSettingsNetworkEgressPoolMemberInput,
  type SettingsNetworkEgressPool,
  type SettingsNetworkEgressPoolMember,
  type SettingsNetworkEgressProvider,
  type UpdateSettingsNetworkEgressPoolInput,
  type UpdateSettingsNetworkEgressPoolMemberInput
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';

type PoolFormValues = CreateSettingsNetworkEgressPoolInput;
type MemberFormValues = CreateSettingsNetworkEgressPoolMemberInput;
type MemberUpdateFormValues = UpdateSettingsNetworkEgressPoolMemberInput;

function healthTag(health: string) {
  const color =
    health === 'healthy'
      ? 'green'
      : health === 'unhealthy'
        ? 'red'
        : health === 'invalid'
          ? 'orange'
          : undefined;
  const label =
    health === 'healthy' || health === 'unhealthy' || health === 'invalid'
      ? i18nText('settings', `auto.network_center_health_${health}`)
      : health;

  return <Tag color={color}>{label}</Tag>;
}

function PoolNameModal({
  pool,
  open,
  submitting,
  onClose,
  onSubmit
}: {
  pool: SettingsNetworkEgressPool | null;
  open: boolean;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (values: PoolFormValues) => void;
}) {
  const [form] = Form.useForm<PoolFormValues>();

  return (
    <Modal
      title={i18nText(
        'settings',
        pool
          ? 'auto.network_center_pool_edit'
          : 'auto.network_center_pool_create'
      )}
      open={open}
      onCancel={onClose}
      destroyOnHidden
      okText={i18nText('settings', 'auto.save')}
      confirmLoading={submitting}
      onOk={() => form.submit()}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={pool ? { display_name: pool.display_name } : undefined}
        onFinish={onSubmit}
      >
        <Form.Item
          name="display_name"
          label={i18nText('settings', 'auto.network_center_pool_display_name')}
          rules={[{ required: true }]}
        >
          <Input autoFocus />
        </Form.Item>
      </Form>
    </Modal>
  );
}

function MemberModal({
  member,
  providers,
  open,
  submitting,
  onClose,
  onSubmit
}: {
  member: SettingsNetworkEgressPoolMember | null;
  providers: SettingsNetworkEgressProvider[];
  open: boolean;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (values: MemberFormValues | MemberUpdateFormValues) => void;
}) {
  const [form] = Form.useForm();
  const isEditing = member !== null;
  const referenceOptions = providers.flatMap((provider) =>
    provider.egresses.map((egress) => ({
      value: `${provider.id}:${egress.provider_egress_key}`,
      label: `${provider.display_name} · ${egress.display_name} (${egress.provider_egress_key})`,
      provider_id: provider.id,
      provider_egress_key: egress.provider_egress_key
    }))
  );

  return (
    <Modal
      title={i18nText(
        'settings',
        isEditing
          ? 'auto.network_center_member_edit'
          : 'auto.network_center_member_create'
      )}
      open={open}
      onCancel={onClose}
      destroyOnHidden
      okText={i18nText('settings', 'auto.save')}
      confirmLoading={submitting}
      onOk={() => form.submit()}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={
          member
            ? { enabled: member.enabled, sequence: member.sequence }
            : { enabled: true, sequence: 0 }
        }
        onFinish={(values) => {
          if (isEditing) {
            onSubmit(values as MemberUpdateFormValues);
            return;
          }
          const reference = values as MemberFormValues & {
            provider_reference?: string;
          };
          const selected = referenceOptions.find(
            (option) => option.value === reference.provider_reference
          );
          if (!selected) {
            return;
          }
          onSubmit({
            provider_id: selected.provider_id,
            provider_egress_key: selected.provider_egress_key,
            enabled: reference.enabled,
            sequence: reference.sequence
          });
        }}
      >
        {!isEditing ? (
          <Form.Item
            name="provider_reference"
            label={i18nText('settings', 'auto.network_center_member_reference')}
            rules={[{ required: true }]}
          >
            <Select options={referenceOptions} />
          </Form.Item>
        ) : (
          <Form.Item
            label={i18nText('settings', 'auto.network_center_member_reference')}
          >
            <Typography.Text>
              {member.provider_id} · {member.provider_egress_key}
            </Typography.Text>
          </Form.Item>
        )}
        <Form.Item
          name="sequence"
          label={i18nText('settings', 'auto.network_center_member_sequence')}
          rules={[{ required: true }]}
        >
          <InputNumber
            min={0}
            precision={0}
            className="network-center-pools__sequence"
          />
        </Form.Item>
        <Form.Item
          name="enabled"
          label={i18nText('settings', 'auto.network_center_member_enabled')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>
      </Form>
    </Modal>
  );
}

export function NetworkEgressPoolsPanel({
  providers
}: {
  providers: SettingsNetworkEgressProvider[];
}) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const poolsQuery = useQuery({
    queryKey: settingsNetworkEgressPoolsQueryKey,
    queryFn: fetchSettingsNetworkEgressPools
  });
  const [poolModal, setPoolModal] = useState<
    SettingsNetworkEgressPool | null | undefined
  >(undefined);
  const [memberModal, setMemberModal] = useState<{
    pool: SettingsNetworkEgressPool;
    member: SettingsNetworkEgressPoolMember | null;
  } | null>(null);

  const invalidatePools = () =>
    queryClient.invalidateQueries({
      queryKey: settingsNetworkEgressPoolsQueryKey
    });
  const createPoolMutation = useMutation({
    mutationFn: (input: PoolFormValues) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressPool(input, csrfToken);
    },
    onSuccess: async () => {
      await invalidatePools();
      setPoolModal(undefined);
    },
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_pool_save_failed')
      )
  });
  const updatePoolMutation = useMutation({
    mutationFn: ({
      poolId,
      input
    }: {
      poolId: string;
      input: PoolFormValues;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return updateSettingsNetworkEgressPool(poolId, input, csrfToken);
    },
    onSuccess: async () => {
      await invalidatePools();
      setPoolModal(undefined);
    },
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_pool_save_failed')
      )
  });
  const deletePoolMutation = useMutation({
    mutationFn: (poolId: string) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return deleteSettingsNetworkEgressPool(poolId, csrfToken);
    },
    onSuccess: invalidatePools,
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_pool_delete_failed')
      )
  });
  const createMemberMutation = useMutation({
    mutationFn: ({
      poolId,
      input
    }: {
      poolId: string;
      input: MemberFormValues;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressPoolMember(poolId, input, csrfToken);
    },
    onSuccess: async () => {
      await invalidatePools();
      setMemberModal(null);
    },
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_member_save_failed')
      )
  });
  const updateMemberMutation = useMutation({
    mutationFn: ({
      poolId,
      memberId,
      input
    }: {
      poolId: string;
      memberId: string;
      input: MemberUpdateFormValues;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return updateSettingsNetworkEgressPoolMember(
        poolId,
        memberId,
        input,
        csrfToken
      );
    },
    onSuccess: async () => {
      await invalidatePools();
      setMemberModal(null);
    },
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_member_save_failed')
      )
  });
  const deleteMemberMutation = useMutation({
    mutationFn: ({
      poolId,
      memberId
    }: {
      poolId: string;
      memberId: string;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return deleteSettingsNetworkEgressPoolMember(poolId, memberId, csrfToken);
    },
    onSuccess: invalidatePools,
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_member_delete_failed')
      )
  });

  const memberColumns: ColumnsType<SettingsNetworkEgressPoolMember> = [
    {
      title: i18nText('settings', 'auto.network_center_member_reference'),
      key: 'reference',
      render: (_, member) =>
        `${member.provider_id} · ${member.provider_egress_key}`
    },
    {
      title: i18nText('settings', 'auto.network_center_member_sequence'),
      dataIndex: 'sequence',
      key: 'sequence',
      width: 100
    },
    {
      title: i18nText('settings', 'auto.network_center_member_enabled'),
      dataIndex: 'enabled',
      key: 'enabled',
      width: 120,
      render: (enabled: boolean) =>
        enabled
          ? i18nText('settings', 'auto.enabled')
          : i18nText('settings', 'auto.disabled')
    },
    {
      title: i18nText('settings', 'auto.network_center_member_health'),
      dataIndex: 'health',
      key: 'health',
      width: 120,
      render: healthTag
    }
  ];

  const poolColumns: ColumnsType<SettingsNetworkEgressPool> = [
    {
      title: i18nText('settings', 'auto.network_center_pool_display_name'),
      dataIndex: 'display_name',
      key: 'display_name',
      render: (displayName: string, pool) => (
        <Space size={8}>
          {displayName}
          {pool.owner_provider_id ? (
            <Tag>
              {i18nText('settings', 'auto.network_center_pool_provider_owned')}
            </Tag>
          ) : null}
        </Space>
      )
    },
    {
      title: i18nText(
        'settings',
        'auto.network_center_pool_selection_strategy'
      ),
      dataIndex: 'selection_strategy',
      key: 'selection_strategy',
      render: (strategy: string) =>
        strategy === 'healthy_first'
          ? i18nText('settings', 'auto.network_center_pool_healthy_first')
          : strategy
    },
    {
      title: i18nText('settings', 'auto.network_center_pool_members'),
      key: 'members',
      render: (_, pool) => pool.members.length
    },
    {
      title: '',
      key: 'actions',
      width: 160,
      render: (_, pool) =>
        pool.owner_provider_id ? null : (
          <Space size={0}>
            <Button type="link" onClick={() => setPoolModal(pool)}>
              {i18nText('settings', 'auto.edit')}
            </Button>
            <Popconfirm
              title={i18nText(
                'settings',
                'auto.network_center_pool_delete_confirm'
              )}
              onConfirm={() => deletePoolMutation.mutate(pool.id)}
            >
              <Button type="link" danger loading={deletePoolMutation.isPending}>
                {i18nText('settings', 'auto.delete')}
              </Button>
            </Popconfirm>
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
          data-testid="network-center-pools-shell"
        >
          <Button type="primary" onClick={() => setPoolModal(null)}>
            {i18nText('settings', 'auto.network_center_pool_create')}
          </Button>
        </Flex>
        {poolsQuery.isError ? (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'settings',
              'auto.network_center_pools_load_failed'
            )}
          />
        ) : poolsQuery.data?.length === 0 && !poolsQuery.isLoading ? (
          <Empty
            description={i18nText('settings', 'auto.network_center_no_pools')}
          />
        ) : (
          <Table
            rowKey="id"
            columns={poolColumns}
            dataSource={poolsQuery.data ?? []}
            loading={poolsQuery.isLoading}
            pagination={false}
            expandable={{
              expandedRowRender: (pool) => (
                <Space
                  orientation="vertical"
                  size={12}
                  className="network-center-pools__members"
                >
                  <Flex justify="space-between" align="center">
                    <Typography.Text strong>
                      {i18nText('settings', 'auto.network_center_pool_members')}
                    </Typography.Text>
                    {pool.owner_provider_id ? null : (
                      <Button
                        size="small"
                        onClick={() => setMemberModal({ pool, member: null })}
                      >
                        {i18nText(
                          'settings',
                          'auto.network_center_member_create'
                        )}
                      </Button>
                    )}
                  </Flex>
                  <Table
                    rowKey="id"
                    size="small"
                    columns={
                      [
                        ...memberColumns,
                        pool.owner_provider_id
                          ? null
                          : {
                              title: '',
                              key: 'actions',
                              width: 140,
                              render: (
                                _: unknown,
                                member: SettingsNetworkEgressPoolMember
                              ) => (
                                <Space size={0}>
                                  <Button
                                    type="link"
                                    onClick={() =>
                                      setMemberModal({ pool, member })
                                    }
                                  >
                                    {i18nText('settings', 'auto.edit')}
                                  </Button>
                                  <Popconfirm
                                    title={i18nText(
                                      'settings',
                                      'auto.network_center_member_delete_confirm'
                                    )}
                                    onConfirm={() =>
                                      deleteMemberMutation.mutate({
                                        poolId: pool.id,
                                        memberId: member.id
                                      })
                                    }
                                  >
                                    <Button
                                      type="link"
                                      danger
                                      loading={deleteMemberMutation.isPending}
                                    >
                                      {i18nText('settings', 'auto.delete')}
                                    </Button>
                                  </Popconfirm>
                                </Space>
                              )
                            }
                      ].filter(
                        Boolean
                      ) as ColumnsType<SettingsNetworkEgressPoolMember>
                    }
                    dataSource={pool.members}
                    pagination={false}
                  />
                </Space>
              )
            }}
          />
        )}
      </Flex>
      <PoolNameModal
        pool={poolModal ?? null}
        open={poolModal !== undefined}
        submitting={
          createPoolMutation.isPending || updatePoolMutation.isPending
        }
        onClose={() => setPoolModal(undefined)}
        onSubmit={(values) => {
          if (poolModal) {
            updatePoolMutation.mutate({ poolId: poolModal.id, input: values });
            return;
          }
          createPoolMutation.mutate(values);
        }}
      />
      <MemberModal
        member={memberModal?.member ?? null}
        providers={providers}
        open={memberModal !== null}
        submitting={
          createMemberMutation.isPending || updateMemberMutation.isPending
        }
        onClose={() => setMemberModal(null)}
        onSubmit={(values) => {
          if (!memberModal) return;
          if (memberModal.member) {
            updateMemberMutation.mutate({
              poolId: memberModal.pool.id,
              memberId: memberModal.member.id,
              input: values as MemberUpdateFormValues
            });
            return;
          }
          createMemberMutation.mutate({
            poolId: memberModal.pool.id,
            input: values as MemberFormValues
          });
        }}
      />
    </SettingsSectionSurface>
  );
}
