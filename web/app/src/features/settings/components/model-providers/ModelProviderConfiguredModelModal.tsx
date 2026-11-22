import { useEffect, useMemo, useRef, useState, type UIEvent } from 'react';

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

import {
  listSettingsPricingTargets,
  type SettingsPricingTarget,
  type SettingsPricingTargetProvider
} from '../../api/billing';
import { i18nText } from '../../../../shared/i18n/text';
import { formatPricingRate } from '../../lib/pricing-format';
import {
  MODEL_CONTEXT_WINDOW_PRESET_OPTIONS,
  parseModelContextWindowInput
} from './model-context-window';

const PRICING_TARGETS_PAGE_SIZE = 50;

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
  onCancel,
  onSave
}: {
  open: boolean;
  editing: boolean;
  initialValue: ConfiguredModelEditorValue | null;
  modelIds: string[];
  reservedModelIds: string[];
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

  const [providerSearch, setProviderSearch] = useState('');
  const [providerPage, setProviderPage] = useState(1);
  const [providers, setProviders] = useState<SettingsPricingTargetProvider[]>([]);
  const [providersTotalCount, setProvidersTotalCount] = useState(0);
  const [providersLoading, setProvidersLoading] = useState(false);

  const [modelSearch, setModelSearch] = useState('');
  const [modelPage, setModelPage] = useState(1);
  const [models, setModels] = useState<SettingsPricingTarget[]>([]);
  const [modelsTotalCount, setModelsTotalCount] = useState(0);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [resolvedSelectedTarget, setResolvedSelectedTarget] =
    useState<SettingsPricingTarget | null>(null);

  const providerRequestSeq = useRef(0);
  const modelRequestSeq = useRef(0);

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
    setProviderSearch('');
    setProviderPage(1);
    setProviders([]);
    setProvidersTotalCount(0);
    setModelSearch('');
    setModelPage(1);
    setModels([]);
    setModelsTotalCount(0);
    setResolvedSelectedTarget(null);
  }, [form, initialValue, open]);

  useEffect(() => {
    if (!open) return;
    const seq = ++providerRequestSeq.current;
    let active = true;
    setProvidersLoading(true);
    void listSettingsPricingTargets({
      provider_query: providerSearch.trim() || undefined,
      page: providerPage,
      page_size: PRICING_TARGETS_PAGE_SIZE
    })
      .then((response) => {
        if (!active || seq !== providerRequestSeq.current) return;
        setProvidersTotalCount(response.providers_total_count);
        setProviders((current) => {
          if (providerPage === 1) return response.providers;
          const seen = new Set(current.map((item) => item.provider_code));
          return [
            ...current,
            ...response.providers.filter((item) => !seen.has(item.provider_code))
          ];
        });
      })
      .catch(() => {
        if (!active || seq !== providerRequestSeq.current) return;
        if (providerPage === 1) {
          setProviders([]);
          setProvidersTotalCount(0);
        }
      })
      .finally(() => {
        if (active && seq === providerRequestSeq.current) {
          setProvidersLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [open, providerPage, providerSearch]);

  useEffect(() => {
    if (!open || !selectedProviderCode) return;
    const seq = ++modelRequestSeq.current;
    let active = true;
    setModelsLoading(true);
    void listSettingsPricingTargets({
      provider_code: selectedProviderCode,
      upstream_model_id: modelSearch.trim() || undefined,
      selected_model_id:
        modelPage === 1 && selectedModelId ? selectedModelId : undefined,
      page: modelPage,
      page_size: PRICING_TARGETS_PAGE_SIZE
    })
      .then((response) => {
        if (!active || seq !== modelRequestSeq.current) return;
        setModelsTotalCount(response.total_count);
        if (modelPage === 1) {
          setResolvedSelectedTarget(response.selected_target ?? null);
        }
        setModels((current) => {
          if (modelPage === 1) return response.items;
          const seen = new Set(current.map((item) => item.upstream_model_id));
          return [
            ...current,
            ...response.items.filter(
              (item) => !seen.has(item.upstream_model_id)
            )
          ];
        });
      })
      .catch(() => {
        if (!active || seq !== modelRequestSeq.current) return;
        if (modelPage === 1) {
          setModels([]);
          setModelsTotalCount(0);
          setResolvedSelectedTarget(null);
        }
      })
      .finally(() => {
        if (active && seq === modelRequestSeq.current) {
          setModelsLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [open, selectedProviderCode, modelPage, modelSearch]);

  const providerOptions = useMemo(() => {
    const codes = providers.map((item) => item.provider_code);
    if (
      !providerSearch.trim() &&
      selectedProviderCode &&
      !codes.includes(selectedProviderCode)
    ) {
      codes.unshift(selectedProviderCode);
    }
    return codes.map((providerCode) => ({
      label: providerCode,
      value: providerCode
    }));
  }, [providerSearch, providers, selectedProviderCode]);

  const modelOptions = useMemo(() => {
    const ids = models.map((item) => item.upstream_model_id);
    if (
      !modelSearch.trim() &&
      selectedModelId &&
      resolvedSelectedTarget?.provider_code === selectedProviderCode &&
      resolvedSelectedTarget?.upstream_model_id === selectedModelId &&
      !ids.includes(selectedModelId)
    ) {
      ids.unshift(selectedModelId);
    }
    return ids.map((modelId) => ({
      label: modelId,
      value: modelId
    }));
  }, [
    modelSearch,
    models,
    resolvedSelectedTarget,
    selectedModelId,
    selectedProviderCode
  ]);

  const selectedTarget =
    models.find(
      (target) =>
        target.provider_code === selectedProviderCode &&
        target.upstream_model_id === selectedModelId
    ) ??
    (resolvedSelectedTarget &&
    resolvedSelectedTarget.provider_code === selectedProviderCode &&
    resolvedSelectedTarget.upstream_model_id === selectedModelId
      ? resolvedSelectedTarget
      : undefined);

  function handleProviderPopupScroll(event: UIEvent<HTMLDivElement>) {
    const target = event.currentTarget;
    if (
      !providersLoading &&
      providers.length < providersTotalCount &&
      target.scrollTop + target.offsetHeight >= target.scrollHeight - 24
    ) {
      setProviderPage((page) => page + 1);
    }
  }

  function handleModelPopupScroll(event: UIEvent<HTMLDivElement>) {
    const target = event.currentTarget;
    if (
      !modelsLoading &&
      models.length < modelsTotalCount &&
      target.scrollTop + target.offsetHeight >= target.scrollHeight - 24
    ) {
      setModelPage((page) => page + 1);
    }
  }

  async function handleSave() {
    const values = await form.validateFields();
    if (!selectedProviderCode || !selectedModelId) {
      setPricingError(true);
      return;
    }
    let confirmedTarget = selectedTarget;
    if (!confirmedTarget) {
      try {
        const lookup = await listSettingsPricingTargets({
          provider_code: selectedProviderCode,
          selected_model_id: selectedModelId,
          page: 1,
          page_size: 1
        });
        confirmedTarget = lookup.selected_target ?? undefined;
        if (confirmedTarget) {
          setResolvedSelectedTarget(confirmedTarget);
        }
      } catch {
        confirmedTarget = undefined;
      }
    }
    if (!confirmedTarget) {
      setPricingError(true);
      return;
    }
    onSave({
      model_id: values.model_id.trim(),
      context_window_input: values.context_window_input?.trim() ?? '',
      supports_multimodal: values.supports_multimodal,
      enabled: values.enabled,
      pricing_provider_code: confirmedTarget.provider_code,
      pricing_model_id: confirmedTarget.upstream_model_id
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
              filterOption={false}
              loading={providersLoading}
              aria-label={i18nText('settings', 'auto.billing_provider_code')}
              value={selectedProviderCode || undefined}
              options={providerOptions}
              placeholder={i18nText('settings', 'auto.billing_provider_code')}
              onSearch={(value) => {
                setProviderSearch(value);
                setProviderPage(1);
              }}
              onPopupScroll={handleProviderPopupScroll}
              onChange={(providerCode) => {
                setSelectedProviderCode(providerCode);
                setSelectedModelId('');
                setModelSearch('');
                setModelPage(1);
                setModels([]);
                setModelsTotalCount(0);
                setResolvedSelectedTarget(null);
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
              filterOption={false}
              disabled={!selectedProviderCode}
              loading={modelsLoading}
              aria-label={i18nText('settings', 'auto.billing_model_id')}
              value={selectedModelId || undefined}
              options={modelOptions}
              placeholder={i18nText('settings', 'auto.billing_model_id')}
              onSearch={(value) => {
                setModelSearch(value);
                setModelPage(1);
              }}
              onPopupScroll={handleModelPopupScroll}
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

        <Table<SettingsPricingTarget>
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
            },
            {
              key: 'cache_write',
              title: i18nText('settings', 'auto.billing_cache_write_price'),
              render: (_, target) =>
                formatPricingRate(
                  target.cache_write_token_unit_price,
                  target.cache_write_token_unit_size
                )
            }
          ]}
        />
      </Form>
    </Modal>
  );
}
