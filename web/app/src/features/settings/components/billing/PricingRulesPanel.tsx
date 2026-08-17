import { useCallback, useMemo, useState } from 'react';
import { DeleteOutlined, EditOutlined, PlusOutlined } from '@ant-design/icons';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Flex,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Tag
} from 'antd';

import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../../../../shared/ui/data-table/DataTableLayout';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import {
  createSettingsPricingRule,
  deleteSettingsPricingRule,
  listSettingsPricingRules,
  settingsPricingRulesQueryKey,
  updateSettingsPricingRule,
  type SettingsPricingRule,
  type SettingsPricingRulesFilter
} from '../../api/billing';
import { formatPricingRate } from './pricing-rate-display';
import './pricing-rules-panel.css';

const DEFAULT_UNIT = 1_000_000;
const PAGE_SIZE = 20;
type PricingKind = 'input' | 'output' | 'cache_hit';

type PricingRuleFilters = Omit<
  SettingsPricingRulesFilter,
  'page' | 'page_size'
>;

const DEFAULT_FILTERS: PricingRuleFilters = {};

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
  const [page, setPage] = useState(1);
  const [providerCode, setProviderCode] = useState('');
  const [upstreamModelId, setUpstreamModelId] = useState('');
  const [enabled, setEnabled] = useState<boolean>();
  const [sourceKind, setSourceKind] = useState<'official' | 'manual'>();
  const [appliedFilters, setAppliedFilters] =
    useState<PricingRuleFilters>(DEFAULT_FILTERS);
  const filter = useMemo<SettingsPricingRulesFilter>(
    () => ({
      ...appliedFilters,
      page,
      page_size: PAGE_SIZE
    }),
    [appliedFilters, page]
  );
  const rulesQuery = useQuery({
    queryKey: [...settingsPricingRulesQueryKey, filter],
    queryFn: () => listSettingsPricingRules(filter)
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
  const deletePricingRule = deleteMutation.mutate;

  const openEditor = useCallback(
    (rule?: SettingsPricingRule) => {
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
    },
    [form]
  );
  const columns = useMemo<Array<DataTableColumn<SettingsPricingRule>>>(
    () => [
      {
        key: 'provider_code',
        title: i18nText('settings', 'auto.billing_provider_code'),
        dataIndex: 'provider_code',
        width: 180
      },
      {
        key: 'upstream_model_id',
        title: i18nText('settings', 'auto.billing_model_id'),
        dataIndex: 'upstream_model_id',
        width: 220
      },
      {
        key: 'input_price',
        title: i18nText('settings', 'auto.billing_input_price'),
        width: 200,
        render: (_: unknown, row: SettingsPricingRule) =>
          formatPricingRate(
            row.input_token_unit_price,
            row.input_token_unit_size
          )
      },
      {
        key: 'output_price',
        title: i18nText('settings', 'auto.billing_output_price'),
        width: 200,
        render: (_: unknown, row: SettingsPricingRule) =>
          formatPricingRate(
            row.output_token_unit_price,
            row.output_token_unit_size
          )
      },
      {
        key: 'cache_price',
        title: i18nText('settings', 'auto.billing_cache_price'),
        width: 200,
        render: (_: unknown, row: SettingsPricingRule) =>
          formatPricingRate(
            row.cache_hit_token_unit_price,
            row.cache_hit_token_unit_size
          )
      },
      {
        key: 'source_kind',
        title: i18nText('settings', 'auto.source'),
        dataIndex: 'source_kind',
        width: 120,
        render: (value: unknown) =>
          i18nText(
            'settings',
            value === 'official'
              ? 'auto.billing_source_official'
              : 'auto.billing_source_manual'
          )
      },
      {
        key: 'status',
        title: i18nText('settings', 'auto.billing_status'),
        width: 120,
        render: (_: unknown, row: SettingsPricingRule) => (
          <Tag color={row.enabled ? 'green' : 'default'}>
            {row.enabled
              ? i18nText('settings', 'auto.enabled')
              : i18nText('settings', 'auto.deactivate')}
          </Tag>
        )
      },
      {
        key: 'operation',
        title: i18nText('settings', 'auto.operation'),
        width: 220,
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
              onConfirm={() => deletePricingRule(row.id)}
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
    [canManage, deletePricingRule, openEditor]
  );
  const tableConfiguration = useUserPreferenceDataTableConfiguration({
    columns,
    preferenceKey: 'settings.model_provider_pricing_rules'
  });

  function applyFilters() {
    setPage(1);
    setAppliedFilters({
      provider_code: providerCode.trim() || undefined,
      upstream_model_id: upstreamModelId.trim() || undefined,
      enabled,
      source_kind: sourceKind
    });
  }

  function resetFilters() {
    setProviderCode('');
    setUpstreamModelId('');
    setEnabled(undefined);
    setSourceKind(undefined);
    setPage(1);
    setAppliedFilters(DEFAULT_FILTERS);
  }

  return (
    <section className="pricing-rules-panel">
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
            <DataTableFilterField
              label={i18nText('settings', 'auto.billing_provider_code')}
            >
              <Input
                aria-label={i18nText('settings', 'auto.billing_provider_code')}
                value={providerCode}
                onChange={(event) => setProviderCode(event.target.value)}
              />
            </DataTableFilterField>
            <DataTableFilterField
              label={i18nText('settings', 'auto.billing_model_id')}
            >
              <Input
                aria-label={i18nText('settings', 'auto.billing_model_id')}
                value={upstreamModelId}
                onChange={(event) => setUpstreamModelId(event.target.value)}
              />
            </DataTableFilterField>
            <DataTableFilterField
              label={i18nText('settings', 'auto.billing_status')}
            >
              <Select
                aria-label={i18nText('settings', 'auto.billing_status')}
                allowClear
                options={[
                  {
                    label: i18nText('settings', 'auto.enabled'),
                    value: true
                  },
                  {
                    label: i18nText('settings', 'auto.deactivate'),
                    value: false
                  }
                ]}
                value={enabled}
                onChange={setEnabled}
              />
            </DataTableFilterField>
            <DataTableFilterField label={i18nText('settings', 'auto.source')}>
              <Select
                aria-label={i18nText('settings', 'auto.source')}
                allowClear
                options={[
                  {
                    label: i18nText('settings', 'auto.billing_source_official'),
                    value: 'official'
                  },
                  {
                    label: i18nText('settings', 'auto.billing_source_manual'),
                    value: 'manual'
                  }
                ]}
                value={sourceKind}
                onChange={setSourceKind}
              />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
        <DataTable<SettingsPricingRule>
          columns={columns}
          configuration={tableConfiguration}
          dataSource={rulesQuery.data?.items ?? []}
          loading={rulesQuery.isLoading || rulesQuery.isFetching}
          page={page}
          pageSize={PAGE_SIZE}
          rowKey="id"
          toolbar={
            <Flex justify="flex-end" gap={8} wrap>
              <Button
                type="primary"
                icon={<PlusOutlined />}
                disabled={!canManage}
                onClick={() => openEditor()}
              >
                {i18nText('settings', 'auto.billing_add_rule')}
              </Button>
              <DataTableColumnSettings
                columns={columns}
                configuration={tableConfiguration}
              />
            </Flex>
          }
          total={rulesQuery.data?.total_count ?? 0}
          onPageChange={setPage}
        />
      </DataTableLayout>
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
    </section>
  );
}
