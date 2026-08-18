import { useEffect, useMemo, useState } from 'react';

import {
  AutoComplete,
  Empty,
  Flex,
  Form,
  Modal,
  Select,
  Switch,
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
  supports_multimodal: boolean;
  enabled: boolean;
  pricing_provider_code: string;
  pricing_model_id: string;
};

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
    supports_multimodal: boolean;
    enabled: boolean;
  }>();
  const [selectedProviderCode, setSelectedProviderCode] = useState('zero');
  const [selectedModelId, setSelectedModelId] = useState('any');
  const [pricingError, setPricingError] = useState(false);

  useEffect(() => {
    if (!open) return;
    form.setFieldsValue({
      model_id: initialValue?.model_id ?? '',
      context_window_input: initialValue?.context_window_input ?? '',
      supports_multimodal: initialValue?.supports_multimodal ?? false,
      enabled: initialValue?.enabled ?? true
    });
    setSelectedProviderCode(initialValue?.pricing_provider_code ?? 'zero');
    setSelectedModelId(initialValue?.pricing_model_id ?? 'any');
    setPricingError(false);
  }, [form, initialValue, open]);

  const providerOptions = useMemo(
    () =>
      Array.from(
        new Set(pricingTargets.map((target) => target.provider_code))
      ).map((providerCode) => ({ label: providerCode, value: providerCode })),
    [pricingTargets]
  );
  const modelOptions = useMemo(
    () =>
      pricingTargets
        .filter((target) => target.provider_code === selectedProviderCode)
        .map((target) => ({
          label: target.upstream_model_id,
          value: target.upstream_model_id
        })),
    [pricingTargets, selectedProviderCode]
  );

  const selectedTarget = pricingTargets.find(
    (target) =>
      target.provider_code === selectedProviderCode &&
      target.upstream_model_id === selectedModelId
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
      supports_multimodal: values.supports_multimodal,
      enabled: values.enabled,
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

        <Flex gap={24} align="center" style={{ marginBottom: 12 }}>
          <Flex gap={8} align="center">
            <Typography.Text>
              {i18nText('settings', 'auto.multimodal')}
            </Typography.Text>
            <Form.Item
              name="supports_multimodal"
              valuePropName="checked"
              noStyle
            >
              <Switch aria-label={i18nText('settings', 'auto.multimodal')} />
            </Form.Item>
          </Flex>
          <Flex gap={8} align="center">
            <Typography.Text>
              {i18nText('settings', 'auto.enabled')}
            </Typography.Text>
            <Form.Item name="enabled" valuePropName="checked" noStyle>
              <Switch aria-label={i18nText('settings', 'auto.enabled')} />
            </Form.Item>
          </Flex>
        </Flex>

        <Typography.Title level={5} style={{ marginTop: 0 }}>
          {i18nText('settings', 'auto.billing_pricing_rules')}
        </Typography.Title>
        <Flex gap={12} align="start">
          <Form.Item
            label={i18nText('settings', 'auto.billing_provider_code')}
            style={{ flex: 1, marginBottom: 0 }}
          >
            <Select
              showSearch
              aria-label={i18nText('settings', 'auto.billing_provider_code')}
              value={selectedProviderCode || undefined}
              options={providerOptions}
              optionFilterProp="label"
              placeholder={i18nText('settings', 'auto.billing_provider_code')}
              onChange={(providerCode) => {
                setSelectedProviderCode(providerCode);
                setSelectedModelId('');
                setPricingError(false);
              }}
            />
          </Form.Item>
          <Form.Item
            label={i18nText('settings', 'auto.billing_model_id')}
            style={{ flex: 1, marginBottom: 0 }}
          >
            <Select
              showSearch
              aria-label={i18nText('settings', 'auto.billing_model_id')}
              value={selectedModelId || undefined}
              options={modelOptions}
              optionFilterProp="label"
              placeholder={i18nText('settings', 'auto.billing_model_id')}
              onChange={(modelId) => {
                setSelectedModelId(modelId);
                setPricingError(false);
              }}
            />
          </Form.Item>
        </Flex>
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
            `${target.provider_code}:${target.upstream_model_id}`
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
