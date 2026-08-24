import { useState } from 'react';
import {
  App,
  Alert,
  Button,
  Empty,
  Flex,
  Form,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tag
} from 'antd';
import { ArrowDownOutlined, ArrowUpOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createSettingsNetworkEgressRoute,
  deleteSettingsNetworkEgressRoute,
  fetchSettingsNetworkEgressPools,
  fetchSettingsNetworkEgressRoutes,
  settingsNetworkEgressPoolsQueryKey,
  settingsNetworkEgressRoutesQueryKey,
  updateSettingsNetworkEgressRoute,
  type SettingsNetworkEgressRoute
} from '../../api/network-center';
import {
  fetchSettingsModelProviderInstances,
  settingsModelProviderInstancesQueryKey
} from '../../api/model-providers';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { i18nText } from '../../../../shared/i18n/text';
import { useAuthStore } from '../../../../state/auth-store';

type Values = {
  target: string;
  instance_id?: string;
  pool_member_ids: string[];
  enabled: boolean;
};

export function NetworkEgressRoutesPanel() {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const queryClient = useQueryClient();
  const [route, setRoute] = useState<
    SettingsNetworkEgressRoute | null | undefined
  >();
  const [form] = Form.useForm<Values>();
  const target = Form.useWatch('target', form);
  const selectedPoolMemberIds = Form.useWatch('pool_member_ids', form) ?? [];
  const routes = useQuery({
    queryKey: settingsNetworkEgressRoutesQueryKey,
    queryFn: fetchSettingsNetworkEgressRoutes
  });
  const pools = useQuery({
    queryKey: settingsNetworkEgressPoolsQueryKey,
    queryFn: fetchSettingsNetworkEgressPools
  });
  const models = useQuery({
    queryKey: settingsModelProviderInstancesQueryKey,
    queryFn: fetchSettingsModelProviderInstances
  });
  const invalidate = () =>
    queryClient.invalidateQueries({
      queryKey: settingsNetworkEgressRoutesQueryKey
    });
  const fail = () =>
    message.error(
      i18nText('settings', 'auto.network_center_route_save_failed')
    );
  const create = useMutation({
    mutationFn: (input: {
      consumer_kind: string;
      consumer_reference: string | null;
      pool_member_ids: string[];
      enabled: boolean;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return createSettingsNetworkEgressRoute(input, csrfToken);
    },
    onSuccess: async () => {
      await invalidate();
      setRoute(undefined);
    },
    onError: fail
  });
  const update = useMutation({
    mutationFn: (input: {
      id: string;
      pool_member_ids: string[];
      enabled: boolean;
    }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return updateSettingsNetworkEgressRoute(
        input.id,
        { pool_member_ids: input.pool_member_ids, enabled: input.enabled },
        csrfToken
      );
    },
    onSuccess: async () => {
      await invalidate();
      setRoute(undefined);
    },
    onError: fail
  });
  const remove = useMutation({
    mutationFn: (id: string) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return deleteSettingsNetworkEgressRoute(id, csrfToken);
    },
    onSuccess: invalidate,
    onError: () =>
      message.error(
        i18nText('settings', 'auto.network_center_route_delete_failed')
      )
  });
  const open = (value: SettingsNetworkEgressRoute | null) => {
    setRoute(value);
    form.setFieldsValue({
      target:
        value?.consumer_kind === 'model_provider'
          ? value.consumer_reference
            ? 'model_instance'
            : 'model_default'
          : (value?.consumer_kind ?? 'github'),
      instance_id: value?.consumer_reference ?? undefined,
      pool_member_ids: value?.pool_member_ids ?? [],
      enabled: value?.enabled ?? true
    });
  };
  const submit = (values: Values) => {
    if (route)
      return update.mutate({
        id: route.id,
        pool_member_ids: values.pool_member_ids,
        enabled: values.enabled
      });
    const selector =
      values.target === 'model_instance'
        ? {
            consumer_kind: 'model_provider',
            consumer_reference: values.instance_id ?? null
          }
        : values.target === 'model_default'
          ? { consumer_kind: 'model_provider', consumer_reference: null }
          : { consumer_kind: values.target, consumer_reference: null };
    create.mutate({
      ...selector,
      pool_member_ids: values.pool_member_ids,
      enabled: values.enabled
    });
  };
  const poolMembers = (pools.data ?? []).flatMap((pool) => pool.members);
  const proxyName = new Map(
    poolMembers.map((member) => [member.id, member.display_name])
  );
  const modelName = new Map(
    (models.data ?? []).map((item) => [item.id, item.display_name])
  );
  const movePoolMember = (index: number, offset: -1 | 1) => {
    const nextIndex = index + offset;
    if (nextIndex < 0 || nextIndex >= selectedPoolMemberIds.length) return;
    const next = [...selectedPoolMemberIds];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    form.setFieldValue('pool_member_ids', next);
  };
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Flex justify="flex-end" data-testid="network-center-routes-shell">
          <Button type="primary" onClick={() => open(null)}>
            {i18nText('settings', 'auto.network_center_route_create')}
          </Button>
        </Flex>
        {routes.isError ? (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'settings',
              'auto.network_center_routes_load_failed'
            )}
          />
        ) : (
          <Table
            rowKey="id"
            loading={routes.isLoading || pools.isLoading || models.isLoading}
            dataSource={routes.data ?? []}
            pagination={false}
            locale={{
              emptyText: (
                <Empty
                  description={i18nText(
                    'settings',
                    'auto.network_center_no_routes'
                  )}
                />
              )
            }}
            columns={[
              {
                title: i18nText(
                  'settings',
                  'auto.network_center_route_consumer'
                ),
                render: (_, item: SettingsNetworkEgressRoute) =>
                  item.consumer_reference
                    ? (modelName.get(item.consumer_reference) ??
                      item.consumer_reference)
                    : item.consumer_kind
              },
              {
                title: i18nText(
                  'settings',
                  'auto.network_center_route_proxy_mapping'
                ),
                dataIndex: 'pool_member_ids',
                render: (memberIds: string[]) => (
                  <Space size={[4, 4]} wrap>
                    {memberIds.map((memberId) => (
                      <Tag key={memberId}>
                        {proxyName.get(memberId) ?? memberId}
                      </Tag>
                    ))}
                  </Space>
                )
              },
              {
                title: i18nText('settings', 'auto.status'),
                dataIndex: 'enabled',
                render: (enabled) => (
                  <Tag color={enabled ? 'green' : undefined}>
                    {i18nText(
                      'settings',
                      enabled ? 'auto.enabled' : 'auto.disabled'
                    )}
                  </Tag>
                )
              },
              {
                title: i18nText('settings', 'auto.operation'),
                render: (_, item: SettingsNetworkEgressRoute) => (
                  <Space>
                    <Button type="link" onClick={() => open(item)}>
                      {i18nText('settings', 'auto.edit')}
                    </Button>
                    <Popconfirm
                      title={i18nText(
                        'settings',
                        'auto.network_center_route_delete_confirm'
                      )}
                      onConfirm={() => remove.mutate(item.id)}
                    >
                      <Button type="link" danger>
                        {i18nText('settings', 'auto.delete')}
                      </Button>
                    </Popconfirm>
                  </Space>
                )
              }
            ]}
          />
        )}
      </Flex>
      <Modal
        open={route !== undefined}
        title={i18nText(
          'settings',
          route
            ? 'auto.network_center_route_edit'
            : 'auto.network_center_route_create'
        )}
        onCancel={() => setRoute(undefined)}
        onOk={() => form.submit()}
        confirmLoading={create.isPending || update.isPending}
      >
        <Form form={form} layout="vertical" onFinish={submit}>
          <Form.Item
            name="target"
            label={i18nText('settings', 'auto.network_center_route_consumer')}
            rules={[{ required: true }]}
          >
            <Select
              disabled={Boolean(route)}
              options={[
                {
                  value: 'github',
                  label: i18nText(
                    'settings',
                    'auto.network_center_route_github'
                  )
                },
                {
                  value: 'http_node',
                  label: i18nText(
                    'settings',
                    'auto.network_center_route_http_node'
                  )
                },
                {
                  value: 'model_default',
                  label: i18nText(
                    'settings',
                    'auto.network_center_route_model_default'
                  )
                },
                {
                  value: 'model_instance',
                  label: i18nText(
                    'settings',
                    'auto.network_center_route_model_instance'
                  )
                }
              ]}
            />
          </Form.Item>
          {target === 'model_instance' ? (
            <Form.Item
              name="instance_id"
              label={i18nText(
                'settings',
                'auto.network_center_route_model_instance'
              )}
              rules={[{ required: true }]}
            >
              <Select
                disabled={Boolean(route)}
                options={(models.data ?? []).map((item) => ({
                  value: item.id,
                  label: item.display_name
                }))}
              />
            </Form.Item>
          ) : null}
          <Form.Item
            name="pool_member_ids"
            label={i18nText(
              'settings',
              'auto.network_center_route_proxy_mapping'
            )}
            rules={[{ required: true, type: 'array', min: 1 }]}
          >
            <Select
              mode="multiple"
              optionFilterProp="label"
              options={poolMembers.map((member) => ({
                value: member.id,
                label: member.display_name
              }))}
            />
          </Form.Item>
          {selectedPoolMemberIds.length > 0 ? (
            <Flex vertical gap={8} style={{ marginBlock: '-12px 24px' }}>
              {selectedPoolMemberIds.map((memberId, index) => {
                const displayName = proxyName.get(memberId) ?? memberId;
                return (
                  <Flex key={memberId} align="center" justify="space-between">
                    <span>
                      {index + 1}. {displayName}
                    </span>
                    <Space size={4}>
                      <Button
                        type="text"
                        size="small"
                        icon={<ArrowUpOutlined />}
                        disabled={index === 0}
                        aria-label={`${i18nText('settings', 'auto.move_up')}：${displayName}`}
                        onClick={() => movePoolMember(index, -1)}
                      />
                      <Button
                        type="text"
                        size="small"
                        icon={<ArrowDownOutlined />}
                        disabled={index === selectedPoolMemberIds.length - 1}
                        aria-label={`${i18nText('settings', 'auto.move_down')}：${displayName}`}
                        onClick={() => movePoolMember(index, 1)}
                      />
                    </Space>
                  </Flex>
                );
              })}
            </Flex>
          ) : null}
          <Form.Item
            name="enabled"
            label={i18nText('settings', 'auto.network_center_route_enabled')}
            valuePropName="checked"
          >
            <Switch />
          </Form.Item>
        </Form>
      </Modal>
    </SettingsSectionSurface>
  );
}
