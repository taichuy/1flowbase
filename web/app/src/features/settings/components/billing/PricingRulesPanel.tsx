import { useMemo, useState } from 'react';
import { DeleteOutlined, EditOutlined, PlusOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Space,
  Switch,
  Table,
  Tag
} from 'antd';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  createSettingsPricingRule,
  deleteSettingsPricingRule,
  listSettingsPricingRules,
  settingsPricingRulesQueryKey,
  updateSettingsPricingRule,
  type SettingsPricingRule
} from '../../api/billing';
import { SettingsSectionSurface } from '../SettingsSectionSurface';

const DEFAULT_UNIT = 1_000_000;
type PricingKind = 'input' | 'output' | 'cache_hit';

function pricingUnitLabel(kind: PricingKind) {
  switch (kind) {
    case 'input':
      return i18nText('settings', 'auto.billing_input_unit');
    case 'output':
      return i18nText('settings', 'auto.billing_output_unit');
    case 'cache_hit':
      return i18nText('settings', 'auto.billing_cache_hit_unit');
  }
}

function pricingPriceLabel(kind: PricingKind) {
  switch (kind) {
    case 'input':
      return i18nText('settings', 'auto.billing_input_price');
    case 'output':
      return i18nText('settings', 'auto.billing_output_price');
    case 'cache_hit':
      return i18nText('settings', 'auto.billing_cache_hit_price');
  }
}

export function PricingRulesPanel({ canManage }: { canManage: boolean }) {
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [form] = Form.useForm();
  const [editing, setEditing] = useState<SettingsPricingRule | null>(null);
  const [open, setOpen] = useState(false);
  const rulesQuery = useQuery({
    queryKey: settingsPricingRulesQueryKey,
    queryFn: () => listSettingsPricingRules()
  });

  const saveMutation = useMutation({
    mutationFn: async (values: Record<string, unknown>) => {
      if (!csrfToken) throw new Error('missing csrf token');
      const payload = {
        provider_code: String(values.provider_code),
        upstream_model_id: String(values.upstream_model_id),
        input_token_unit_size: Number(values.input_token_unit_size),
        input_token_unit_price: String(values.input_token_unit_price),
        output_token_unit_size: Number(values.output_token_unit_size),
        output_token_unit_price: String(values.output_token_unit_price),
        cache_hit_token_unit_size: Number(values.cache_hit_token_unit_size),
        cache_hit_token_unit_price: String(values.cache_hit_token_unit_price),
        currency_code: 'USD' as const,
        effective_from: String(values.effective_from),
        effective_to: values.effective_to ? String(values.effective_to) : null,
        timezone: String(values.timezone),
        weekday_mask: Number(values.weekday_mask),
        local_time_start: values.local_time_start
          ? String(values.local_time_start)
          : null,
        local_time_end: values.local_time_end
          ? String(values.local_time_end)
          : null,
        priority: Number(values.priority),
        enabled: Boolean(values.enabled),
        source_kind: editing?.source_kind ?? ('manual' as const),
        source_catalog_id: editing?.source_catalog_id ?? null,
        source_version: editing?.source_version ?? null,
        source_checksum: editing?.source_checksum ?? null,
        extensions: editing?.extensions ?? {}
      };
      return editing
        ? updateSettingsPricingRule(
            editing.id,
            { ...payload, id: editing.id },
            csrfToken
          )
        : createSettingsPricingRule(payload, csrfToken);
    },
    onSuccess: async () => {
      setOpen(false);
      setEditing(null);
      form.resetFields();
      await queryClient.invalidateQueries({
        queryKey: settingsPricingRulesQueryKey
      });
    }
  });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => {
      if (!csrfToken) throw new Error('missing csrf token');
      return deleteSettingsPricingRule(id, csrfToken);
    },
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: settingsPricingRulesQueryKey })
  });

  const openEditor = (rule?: SettingsPricingRule) => {
    setEditing(rule ?? null);
    form.setFieldsValue(
      rule ?? {
        input_token_unit_size: DEFAULT_UNIT,
        output_token_unit_size: DEFAULT_UNIT,
        cache_hit_token_unit_size: DEFAULT_UNIT,
        input_token_unit_price: '0',
        output_token_unit_price: '0',
        cache_hit_token_unit_price: '0',
        effective_from: new Date().toISOString(),
        timezone: 'UTC',
        weekday_mask: 127,
        priority: 0,
        enabled: true
      }
    );
    setOpen(true);
  };
  const columns = useMemo(
    () => [
      {
        title: i18nText('settings', 'auto.billing_provider_code'),
        dataIndex: 'provider_code'
      },
      {
        title: i18nText('settings', 'auto.billing_model_id'),
        dataIndex: 'upstream_model_id'
      },
      {
        title: i18nText('settings', 'auto.billing_input_price'),
        render: (_: unknown, row: SettingsPricingRule) =>
          `$${row.input_token_unit_price} / ${row.input_token_unit_size}`
      },
      {
        title: i18nText('settings', 'auto.billing_output_price'),
        render: (_: unknown, row: SettingsPricingRule) =>
          `$${row.output_token_unit_price} / ${row.output_token_unit_size}`
      },
      {
        title: i18nText('settings', 'auto.billing_cache_price'),
        render: (_: unknown, row: SettingsPricingRule) =>
          `$${row.cache_hit_token_unit_price} / ${row.cache_hit_token_unit_size}`
      },
      {
        title: i18nText('settings', 'auto.billing_status'),
        render: (_: unknown, row: SettingsPricingRule) => (
          <Tag color={row.enabled ? 'green' : 'default'}>
            {row.enabled
              ? i18nText('settings', 'auto.enabled')
              : i18nText('settings', 'auto.deactivate')}
          </Tag>
        )
      },
      {
        title: i18nText('settings', 'auto.operation'),
        render: (_: unknown, row: SettingsPricingRule) => (
          <Space>
            <Button
              size="small"
              icon={<EditOutlined />}
              disabled={!canManage}
              onClick={() => openEditor(row)}
            >
              {i18nText('settings', 'auto.edit')}
            </Button>
            <Popconfirm
              title={i18nText('settings', 'auto.billing_delete_rule_confirm')}
              onConfirm={() => deleteMutation.mutate(row.id)}
            >
              <Button
                size="small"
                danger
                icon={<DeleteOutlined />}
                disabled={!canManage}
              >
                {i18nText('settings', 'auto.delete')}
              </Button>
            </Popconfirm>
          </Space>
        )
      }
    ],
    [canManage, deleteMutation]
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <Space
        style={{ width: '100%', justifyContent: 'flex-end', marginBottom: 16 }}
      >
        <Button
          type="primary"
          icon={<PlusOutlined />}
          disabled={!canManage}
          onClick={() => openEditor()}
        >
          {i18nText('settings', 'auto.billing_add_rule')}
        </Button>
      </Space>
      <Table
        rowKey="id"
        loading={rulesQuery.isLoading}
        dataSource={rulesQuery.data ?? []}
        columns={columns}
        pagination={{ pageSize: 20 }}
        scroll={{ x: 1100 }}
      />
      <Modal
        open={open}
        title={
          editing
            ? i18nText('settings', 'auto.billing_edit_rule')
            : i18nText('settings', 'auto.billing_add_rule')
        }
        onCancel={() => setOpen(false)}
        onOk={() => form.submit()}
        confirmLoading={saveMutation.isPending}
        width={760}
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={(values) => saveMutation.mutate(values)}
        >
          <Space align="start" wrap>
            <Form.Item
              name="provider_code"
              label={i18nText('settings', 'auto.billing_provider_code')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="upstream_model_id"
              label={i18nText('settings', 'auto.billing_model_id')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
          </Space>
          {(['input', 'output', 'cache_hit'] as const).map((kind) => (
            <Space align="start" key={kind}>
              <Form.Item
                name={`${kind}_token_unit_size`}
                label={pricingUnitLabel(kind)}
                rules={[{ required: true }]}
              >
                <InputNumber min={1} />
              </Form.Item>
              <Form.Item
                name={`${kind}_token_unit_price`}
                label={pricingPriceLabel(kind)}
                rules={[{ required: true }]}
              >
                <Input prefix="$" />
              </Form.Item>
            </Space>
          ))}
          <Space align="start" wrap>
            <Form.Item
              name="effective_from"
              label={i18nText('settings', 'auto.billing_effective_from')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="effective_to"
              label={i18nText('settings', 'auto.billing_effective_to')}
            >
              <Input />
            </Form.Item>
            <Form.Item
              name="timezone"
              label={i18nText('settings', 'auto.billing_timezone')}
              rules={[{ required: true }]}
            >
              <Input />
            </Form.Item>
          </Space>
          <Space align="start" wrap>
            <Form.Item
              name="weekday_mask"
              label={i18nText('settings', 'auto.billing_weekday_mask')}
            >
              <InputNumber min={1} max={127} />
            </Form.Item>
            <Form.Item
              name="local_time_start"
              label={i18nText('settings', 'auto.billing_local_start')}
            >
              <Input placeholder="09:00:00" />
            </Form.Item>
            <Form.Item
              name="local_time_end"
              label={i18nText('settings', 'auto.billing_local_end')}
            >
              <Input placeholder="18:00:00" />
            </Form.Item>
            <Form.Item
              name="priority"
              label={i18nText('settings', 'auto.billing_priority')}
            >
              <InputNumber min={0} />
            </Form.Item>
            <Form.Item
              name="enabled"
              label={i18nText('settings', 'auto.billing_enabled')}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
          </Space>
        </Form>
      </Modal>
    </SettingsSectionSurface>
  );
}
