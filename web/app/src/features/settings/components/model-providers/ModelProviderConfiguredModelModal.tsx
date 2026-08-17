import { useEffect, useMemo, useState } from 'react';

import {
  AutoComplete,
  Empty,
  Flex,
  Form,
  Input,
  Modal,
  Table,
  Typography
} from 'antd';

import type { SettingsModelProviderPricingTarget } from '../../api/model-providers';
import { i18nText } from '../../../../shared/i18n/text';
import { formatPricingRate } from '../../lib/pricing-format';
import {
  MODEL_CONTEXT_WINDOW_PRESET_OPTIONS,
  parseModelContextWindowInput
} from './model-context-window';

export type ConfiguredModelEditorValue = {
  model_id: string;
  context_window_input: string;
  pricing_provider_code: string;
  pricing_model_id: string;
};

function pricingTargetKey(providerCode: string, modelId: string) {
  return JSON.stringify([providerCode, modelId]);
}

function formatEffectiveWindow(target: SettingsModelProviderPricingTarget) {
  const effectiveFrom = target.effective_from.replace('T', ' ').slice(0, 16);
  const effectiveTo = target.effective_to
    ? target.effective_to.replace('T', ' ').slice(0, 16)
    : '∞';
  const localWindow =
    target.local_time_start && target.local_time_end
      ? ` · ${target.local_time_start.slice(0, 5)}–${target.local_time_end.slice(0, 5)}`
      : '';
  return `${effectiveFrom} → ${effectiveTo} · ${target.timezone}${localWindow}`;
}

export function ModelProviderConfiguredModelModal({
  open,
  editing,
  initialValue,
  modelIds,
  reservedModelIds,
  pricingTargets,
  onCancel,
  onSave
}: {
  open: boolean;
  editing: boolean;
  initialValue: ConfiguredModelEditorValue | null;
  modelIds: string[];
  reservedModelIds: string[];
  pricingTargets: SettingsModelProviderPricingTarget[];
  onCancel: () => void;
  onSave: (value: ConfiguredModelEditorValue) => void;
}) {
  const [form] = Form.useForm<{
    model_id: string;
    context_window_input: string;
  }>();
  const [providerFilter, setProviderFilter] = useState('');
  const [modelFilter, setModelFilter] = useState('');
  const [selectedTargetKey, setSelectedTargetKey] = useState<string>();
  const [pricingError, setPricingError] = useState(false);

  useEffect(() => {
    if (!open) return;
    form.setFieldsValue({
      model_id: initialValue?.model_id ?? '',
      context_window_input: initialValue?.context_window_input ?? ''
    });
    setProviderFilter('');
    setModelFilter('');
    setSelectedTargetKey(
      pricingTargetKey(
        initialValue?.pricing_provider_code ?? 'zero',
        initialValue?.pricing_model_id ?? 'any'
      )
    );
    setPricingError(false);
  }, [form, initialValue, open]);

  const filteredTargets = useMemo(() => {
    const normalizedProvider = providerFilter.trim().toLowerCase();
    const normalizedModel = modelFilter.trim().toLowerCase();
    return pricingTargets.filter(
      (target) =>
        target.provider_code.toLowerCase().includes(normalizedProvider) &&
        target.upstream_model_id.toLowerCase().includes(normalizedModel)
    );
  }, [modelFilter, pricingTargets, providerFilter]);

  const selectedTarget = pricingTargets.find(
    (target) =>
      pricingTargetKey(target.provider_code, target.upstream_model_id) ===
      selectedTargetKey
  );

  async function handleSave() {
    const values = await form.validateFields();
    if (!selectedTarget) {
      setPricingError(true);
      return;
    }
    onSave({
      model_id: values.model_id.trim(),
      context_window_input: values.context_window_input?.trim() ?? '',
      pricing_provider_code: selectedTarget.provider_code,
      pricing_model_id: selectedTarget.upstream_model_id
    });
  }

  return (
    <Modal
      open={open}
      width={760}
      zIndex={1200}
      destroyOnHidden
      title={`${editing ? i18nText('settings', 'auto.edit') : i18nText('settings', 'auto.new')} ${i18nText('settings', 'auto.model_configuration')}`}
      okText={i18nText('settings', 'auto.confirm')}
      cancelText={i18nText('settings', 'auto.cancel')}
      onCancel={onCancel}
      onOk={() => void handleSave().catch(() => undefined)}
    >
      <Form form={form} layout="vertical">
        <Flex gap={12} align="start">
          <Form.Item
            name="model_id"
            label={i18nText('settings', 'auto.model_id_alt')}
            style={{ flex: 1 }}
            rules={[
              { required: true, whitespace: true },
              {
                validator: (_, value: string) =>
                  reservedModelIds.includes(value?.trim())
                    ? Promise.reject(
                        new Error(
                          i18nText(
                            'settings',
                            'auto.model_configuration_duplicate_id'
                          )
                        )
                      )
                    : Promise.resolve()
              }
            ]}
          >
            <AutoComplete
              options={modelIds.map((modelId) => ({
                label: modelId,
                value: modelId
              }))}
              filterOption={(inputValue, option) =>
                String(option?.value ?? '')
                  .toLowerCase()
                  .includes(inputValue.toLowerCase())
              }
              placeholder={i18nText('settings', 'auto.enter_model_id')}
            />
          </Form.Item>
          <Form.Item
            name="context_window_input"
            label={i18nText('settings', 'auto.context_alt')}
            style={{ width: 180 }}
            rules={[
              {
                validator: (_, value: string) => {
                  const parsed = parseModelContextWindowInput(value ?? '');
                  return parsed.error
                    ? Promise.reject(new Error(parsed.error))
                    : Promise.resolve();
                }
              }
            ]}
          >
            <AutoComplete
              options={MODEL_CONTEXT_WINDOW_PRESET_OPTIONS.map((option) => ({
                label: option.label,
                value: option.value
              }))}
              filterOption={(inputValue, option) =>
                String(option?.value ?? '')
                  .toLowerCase()
                  .includes(inputValue.toLowerCase())
              }
              placeholder={i18nText('settings', 'auto.example_one_two_eight_k')}
            />
          </Form.Item>
        </Flex>

        <Typography.Title level={5} style={{ marginTop: 0 }}>
          {i18nText('settings', 'auto.billing_pricing_rules')}
        </Typography.Title>
        <Flex gap={12} style={{ marginBottom: 12 }}>
          <Input
            allowClear
            value={providerFilter}
            onChange={(event) => setProviderFilter(event.target.value)}
            placeholder={i18nText('settings', 'auto.billing_provider_code')}
          />
          <Input
            allowClear
            value={modelFilter}
            onChange={(event) => setModelFilter(event.target.value)}
            placeholder={i18nText('settings', 'auto.billing_model_id')}
          />
        </Flex>
        <Table<SettingsModelProviderPricingTarget>
          size="small"
          pagination={false}
          scroll={{ y: 200 }}
          rowKey={(target) =>
            pricingTargetKey(target.provider_code, target.upstream_model_id)
          }
          dataSource={filteredTargets}
          locale={{ emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} /> }}
          rowSelection={{
            type: 'radio',
            selectedRowKeys: selectedTargetKey ? [selectedTargetKey] : [],
            onChange: (keys) => {
              setSelectedTargetKey(String(keys[0] ?? ''));
              setPricingError(false);
            }
          }}
          onRow={(target) => ({
            onClick: () => {
              setSelectedTargetKey(
                pricingTargetKey(target.provider_code, target.upstream_model_id)
              );
              setPricingError(false);
            }
          })}
          columns={[
            {
              key: 'provider_code',
              dataIndex: 'provider_code',
              title: i18nText('settings', 'auto.billing_provider_code'),
              width: 150
            },
            {
              key: 'upstream_model_id',
              dataIndex: 'upstream_model_id',
              title: i18nText('settings', 'auto.billing_model_id'),
              width: 190
            },
            {
              key: 'effective_time',
              title: i18nText('settings', 'auto.billing_effective_from'),
              render: (_, target) => formatEffectiveWindow(target)
            }
          ]}
        />
        {pricingError ? (
          <Typography.Text type="danger">
            {i18nText('settings', 'auto.billing_select_rule_required')}
          </Typography.Text>
        ) : null}

        <Table<SettingsModelProviderPricingTarget>
          size="small"
          pagination={false}
          style={{ marginTop: 16 }}
          rowKey={(target) =>
            pricingTargetKey(target.provider_code, target.upstream_model_id)
          }
          dataSource={selectedTarget ? [selectedTarget] : []}
          locale={{ emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} /> }}
          columns={[
            {
              key: 'input',
              title: i18nText('settings', 'auto.billing_input_price'),
              render: (_, target) =>
                formatPricingRate(
                  target.input_token_unit_price,
                  target.input_token_unit_size
                )
            },
            {
              key: 'output',
              title: i18nText('settings', 'auto.billing_output_price'),
              render: (_, target) =>
                formatPricingRate(
                  target.output_token_unit_price,
                  target.output_token_unit_size
                )
            },
            {
              key: 'cache_hit',
              title: i18nText('settings', 'auto.billing_cache_price'),
              render: (_, target) =>
                formatPricingRate(
                  target.cache_hit_token_unit_price,
                  target.cache_hit_token_unit_size
                )
            }
          ]}
        />
      </Form>
    </Modal>
  );
}
