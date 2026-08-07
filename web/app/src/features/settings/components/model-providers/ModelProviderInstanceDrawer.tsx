import { useCallback, useEffect, useRef, useState } from 'react';

import {
  AutoComplete,
  Button,
  Drawer,
  Empty,
  Flex,
  Form,
  Input,
  Select,
  Space,
  Switch,
  Tag,
  Typography
} from 'antd';

import {
  ApiOutlined,
  CheckCircleOutlined,
  DeleteOutlined,
  PlusOutlined
} from '@ant-design/icons';

import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderInstance,
  SettingsModelProviderModelCatalog,
  PreviewSettingsModelProviderModelsResponse
} from '../../api/model-providers';
import { CollapseShell } from '../../../../shared/ui/collapse-shell/CollapseShell';
import { CachedModelSelect } from './CachedModelSelect';
import {
  MODEL_CONTEXT_WINDOW_PRESET_OPTIONS,
  formatModelContextWindowValue,
  parseModelContextWindowInput
} from './model-context-window';
import { i18nText } from '../../../../shared/i18n/text';

type DrawerMode = 'create' | 'edit';
type ModelProviderFormValue = string | boolean | number;
type ModelProviderConfigField = SettingsModelProviderCatalogEntry['form_schema'][number];
type PreviewModelDescriptor = SettingsModelProviderModelCatalog['models'][number];
type PreviewModelsResponse = PreviewSettingsModelProviderModelsResponse;
type ConfiguredModelRow = {
  key: string;
  model_id: string;
  context_window_input: string;
  context_window_error: string | null;
  supports_multimodal: boolean;
  enabled: boolean;
};

const CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS = 'minmax(0, 1fr) 112px 76px 48px 40px';
const CONFIGURED_MODEL_GRID_GAP = 8;

function isSelectConfigField(field: ModelProviderConfigField) {
  return field.field_type === 'enum' || field.control === 'select';
}

function toSelectOptionValue(value: unknown): string | number {
  if (typeof value === 'string' || typeof value === 'number') {
    return value;
  }

  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }

  if (value === null || value === undefined) {
    return '';
  }

  if (typeof value === 'object') {
    return JSON.stringify(value);
  }

  return String(value);
}

function normalizeConfigFieldValue(
  field: ModelProviderConfigField,
  value: unknown
): ModelProviderFormValue {
  if (isSelectConfigField(field)) {
    return toSelectOptionValue(value);
  }

  if (typeof value === 'boolean') {
    return value;
  }

  if (typeof value === 'number') {
    return String(value);
  }

  if (typeof value === 'string') {
    return value;
  }

  if (value === null || value === undefined) {
    return '';
  }

  if (typeof value === 'object') {
    return JSON.stringify(value, null, 2);
  }

  return String(value);
}

function buildFieldLabel(key: string) {
  if (key === 'base_url') {
    return 'API Endpoint';
  }

  if (key === 'api_key') {
    return 'API Key';
  }

  if (key === 'api_protocol') {
    return 'API 协议';
  }

  return key;
}

function buildConfigSelectOptions(field: ModelProviderConfigField) {
  return (field.options ?? []).map((option) => ({
    label: option.label || String(option.value ?? ''),
    value: toSelectOptionValue(option.value),
    disabled: option.disabled ?? false
  }));
}

function resolveDraftConfigValue(
  field: ModelProviderConfigField,
  value: ModelProviderFormValue
) {
  if (!isSelectConfigField(field)) {
    return value;
  }

  const matchedOption = (field.options ?? []).find(
    (option) => toSelectOptionValue(option.value) === value
  );

  return matchedOption ? matchedOption.value : value;
}

function maskSecretPreview(value: string) {
  if (value.length <= 8) {
    return '****';
  }

  return `${value.slice(0, 4)}****${value.slice(-4)}`;
}

function buildInitialConfig(
  mode: DrawerMode,
  entry: SettingsModelProviderCatalogEntry | null,
  instance: SettingsModelProviderInstance | null
) {
  const currentConfig = instance?.config_json ?? {};
  const nextConfig: Record<string, ModelProviderFormValue> = {};

  for (const field of entry?.form_schema ?? []) {
    if (mode === 'edit' && field.field_type === 'secret') {
      nextConfig[field.key] = '';
      continue;
    }

    const currentValue = currentConfig[field.key];

    if (currentValue !== undefined) {
      nextConfig[field.key] = normalizeConfigFieldValue(field, currentValue);
      continue;
    }

    if (field.default_value !== undefined && field.default_value !== null) {
      nextConfig[field.key] = normalizeConfigFieldValue(field, field.default_value);
      continue;
    }

    if (field.field_type === 'boolean') {
      nextConfig[field.key] = field.key === 'validate_model';
      continue;
    }

    if (field.key === 'base_url' && entry?.default_base_url) {
      nextConfig[field.key] = entry.default_base_url;
      continue;
    }

    nextConfig[field.key] = '';
  }

  return nextConfig;
}

function isTextAreaField(key: string) {
  return key.includes('headers') || key.includes('json') || key.includes('schema');
}

function isPreviewOnlyField(field: ModelProviderConfigField) {
  return field.key === 'validate_model';
}

function shouldOmitDraftConfigValue(value: ModelProviderFormValue | undefined) {
  return typeof value === 'string' && value.length === 0;
}

type ModelProviderInstanceDrawerProps = {
  open: boolean;
  mode: DrawerMode;
  catalogEntry: SettingsModelProviderCatalogEntry | null;
  instance: SettingsModelProviderInstance | null;
  cachedModelCatalog: SettingsModelProviderModelCatalog | null;
  defaultIncludedInMain: boolean;
  submitting: boolean;
  onClose: () => void;
  onSubmit: (input: {
    display_name: string;
    included_in_main: boolean;
    config: Record<string, unknown>;
    configured_models: Array<{
      model_id: string;
      enabled: boolean;
      context_window_override_tokens: number | null;
      supports_multimodal: boolean;
    }>;
    preview_token?: string;
  }) => Promise<void>;
  onPreviewModels: (config: Record<string, unknown>) => Promise<PreviewModelsResponse>;
  onRevealSecret: (fieldKey: string) => Promise<string>;
};

export function ModelProviderInstanceDrawer(
  props: ModelProviderInstanceDrawerProps
) {
  if (!props.open) {
    return null;
  }

  return <ModelProviderInstanceDrawerContent {...props} />;
}

function ModelProviderInstanceDrawerContent({
  open,
  mode,
  catalogEntry,
  instance,
  cachedModelCatalog,
  defaultIncludedInMain,
  submitting,
  onClose,
  onSubmit,
  onPreviewModels,
  onRevealSecret
}: ModelProviderInstanceDrawerProps) {
  const [form] = Form.useForm<{
    display_name: string;
    included_in_main: boolean;
    config: Record<string, ModelProviderFormValue>;
  }>();
  const [secretDrafts, setSecretDrafts] = useState<Record<string, string>>({});
  const [revealedSecretKeys, setRevealedSecretKeys] = useState<Record<string, boolean>>({});
  const [revealingSecretKey, setRevealingSecretKey] = useState<string | null>(null);
  const [previewModels, setPreviewModels] = useState<PreviewModelDescriptor[]>([]);
  const configuredModelKeyRef = useRef(0);
  const [configuredModels, setConfiguredModels] = useState<ConfiguredModelRow[]>([]);
  const [selectedCachedModelId, setSelectedCachedModelId] = useState<string | undefined>();
  const [previewToken, setPreviewToken] = useState<string | undefined>();
  const [previewingModels, setPreviewingModels] = useState(false);

  const nextConfiguredModelKey = useCallback(() => {
    const key = `configured-model-${configuredModelKeyRef.current}`;
    configuredModelKeyRef.current += 1;
    return key;
  }, []);

  const buildInitialConfiguredModels = useCallback(() => {
    const sourceModels =
      Array.isArray(instance?.configured_models) && instance.configured_models.length > 0
        ? instance.configured_models
        : (instance?.enabled_model_ids ?? []).map((modelId) => ({
            model_id: modelId,
            enabled: true,
            context_window_override_tokens: null,
            supports_multimodal: null
          }));

    configuredModelKeyRef.current = 0;
    return sourceModels.map((model) => ({
      key: nextConfiguredModelKey(),
      model_id: model.model_id,
      context_window_input: formatModelContextWindowValue(
        model.context_window_override_tokens
      ),
      context_window_error: null,
      supports_multimodal: model.supports_multimodal ?? false,
      enabled: model.enabled
    }));
  }, [instance, nextConfiguredModelKey]);

  useEffect(() => {
    if (!open) {
      form.resetFields();
      setSecretDrafts({});
      setRevealedSecretKeys({});
      setRevealingSecretKey(null);
      setPreviewModels([]);
      configuredModelKeyRef.current = 0;
      setConfiguredModels([]);
      setSelectedCachedModelId(undefined);
      setPreviewToken(undefined);
      setPreviewingModels(false);
      return;
    }

    form.setFieldsValue({
      display_name: instance?.display_name ?? catalogEntry?.display_name ?? '',
      included_in_main: instance?.included_in_main ?? defaultIncludedInMain,
      config: buildInitialConfig(mode, catalogEntry, instance)
    });
    setPreviewModels([]);
    setConfiguredModels(buildInitialConfiguredModels());
    setSelectedCachedModelId(undefined);
    setSecretDrafts({});
    setRevealedSecretKeys({});
    setRevealingSecretKey(null);
    setPreviewToken(undefined);
    setPreviewingModels(false);
  }, [
    buildInitialConfiguredModels,
    catalogEntry,
    defaultIncludedInMain,
    form,
    instance,
    mode,
    open
  ]);

  useEffect(() => {
    if (!open || mode !== 'edit' || !cachedModelCatalog || previewModels.length > 0) {
      return;
    }

    setPreviewModels(cachedModelCatalog.models);
  }, [cachedModelCatalog, mode, open, previewModels.length]);

  function clearPreviewState() {
    setPreviewModels([]);
    setPreviewToken(undefined);
    setSelectedCachedModelId(undefined);
  }

  function normalizeConfiguredModels(rows: ConfiguredModelRow[]) {
    const normalizedRows: Array<{
      model_id: string;
      enabled: boolean;
      context_window_override_tokens: number | null;
      supports_multimodal: boolean;
    }> = [];
    const seen = new Set<string>();
    let hasValidationError = false;

    setConfiguredModels((current) =>
      current.map((row) => {
        const parsedContextWindow = parseModelContextWindowInput(row.context_window_input);
        if (parsedContextWindow.error) {
          hasValidationError = true;
        }

        return {
          ...row,
          context_window_error: parsedContextWindow.error
        };
      })
    );

    for (const row of rows) {
      const normalizedModelId = row.model_id.trim();
      if (!normalizedModelId || seen.has(normalizedModelId)) {
        continue;
      }

      const parsedContextWindow = parseModelContextWindowInput(row.context_window_input);
      if (parsedContextWindow.error) {
        hasValidationError = true;
        continue;
      }

      seen.add(normalizedModelId);
      normalizedRows.push({
        model_id: normalizedModelId,
        enabled: row.enabled,
        context_window_override_tokens: parsedContextWindow.value,
        supports_multimodal: row.supports_multimodal
      });
    }

    return {
      hasValidationError,
      rows: normalizedRows
    };
  }

  function appendConfiguredModelRow(initial?: Partial<ConfiguredModelRow>) {
    setConfiguredModels((current) => [
      ...current,
      {
        key: nextConfiguredModelKey(),
        model_id: initial?.model_id ?? '',
        context_window_input: initial?.context_window_input ?? '',
        context_window_error: initial?.context_window_error ?? null,
        supports_multimodal: initial?.supports_multimodal ?? false,
        enabled: initial?.enabled ?? true
      }
    ]);
  }

  function applyCachedModelSelection(modelId: string | null) {
    setSelectedCachedModelId(modelId ?? undefined);
  }

  async function handleRevealSecret(fieldKey: string) {
    setRevealingSecretKey(fieldKey);

    try {
      const value = await onRevealSecret(fieldKey);
      setSecretDrafts((current) => ({
        ...current,
        [fieldKey]: value
      }));
      clearPreviewState();
      setRevealedSecretKeys((current) => ({
        ...current,
        [fieldKey]: true
      }));
    } finally {
      setRevealingSecretKey((current) => (current === fieldKey ? null : current));
    }
  }

  const title = mode === 'create' ? i18nText("settings", "auto.api_key_authorization_configuration") : i18nText("settings", "auto.edit_api_key_configuration");
  const formSchema = (catalogEntry?.form_schema ?? []).filter(
    (field) => !isPreviewOnlyField(field)
  );
  const editableConfigFields = formSchema.filter(
    (field) => !(mode === 'edit' && field.field_type === 'secret')
  );
  const configFieldNames = editableConfigFields.map((field) => ['config', field.key] as const);
  const primaryConfigFields = formSchema.filter((field) => !field.advanced);
  const advancedConfigFields = formSchema.filter((field) => field.advanced);
  const modelAutocompleteOptions = previewModels.map((model) => ({
    label: model.model_id,
    value: model.model_id
  }));
  const contextWindowOptions = MODEL_CONTEXT_WINDOW_PRESET_OPTIONS.map((option) => ({
    label: option.label,
    value: option.value
  }));

  function buildDraftConfig(valuesConfig: Record<string, ModelProviderFormValue>) {
    const config: Record<string, unknown> = {};

    for (const field of editableConfigFields) {
      const nextValue = valuesConfig?.[field.key];
      if (nextValue === undefined || shouldOmitDraftConfigValue(nextValue)) {
        continue;
      }

      config[field.key] = resolveDraftConfigValue(field, nextValue);
    }

    if (mode === 'edit' && catalogEntry) {
      for (const field of catalogEntry.form_schema) {
        if (field.field_type !== 'secret') {
          continue;
        }

        delete config[field.key];
        const nextSecret = secretDrafts[field.key];
        if (typeof nextSecret === 'string' && nextSecret.length > 0) {
          config[field.key] = nextSecret;
        }
      }
    }
    return config;
  }

  function updateConfiguredModelRow(
    rowKey: string,
    patch: Partial<
      Pick<
        ConfiguredModelRow,
        | 'model_id'
        | 'context_window_input'
        | 'context_window_error'
        | 'supports_multimodal'
        | 'enabled'
      >
    >
  ) {
    setConfiguredModels((current) =>
      current.map((row) => (row.key === rowKey ? { ...row, ...patch } : row))
    );
  }

  function removeConfiguredModelRow(rowKey: string) {
    setConfiguredModels((current) => current.filter((row) => row.key !== rowKey));
  }

  async function handlePreviewModels() {
    const values = await form.validateFields(configFieldNames);
    setPreviewingModels(true);

    try {
      const preview = await onPreviewModels(
        buildDraftConfig((values.config ?? {}) as Record<string, ModelProviderFormValue>)
      );
      setPreviewModels(preview.models);
      setSelectedCachedModelId(undefined);
      setPreviewToken(preview.preview_token);
    } finally {
      setPreviewingModels(false);
    }
  }

  async function handleSubmit() {
    const values = await form.validateFields([
      ['display_name'],
      ['included_in_main'],
      ...configFieldNames
    ]);
    const normalizedConfiguredModels = normalizeConfiguredModels(configuredModels);
    if (normalizedConfiguredModels.hasValidationError) {
      return;
    }

    await onSubmit({
      display_name: values.display_name,
      included_in_main: values.included_in_main,
      config: buildDraftConfig((values.config ?? {}) as Record<string, ModelProviderFormValue>),
      configured_models: normalizedConfiguredModels.rows,
      preview_token: previewToken
    });
  }

  function renderConfigField(field: ModelProviderConfigField) {
    const label = field.label || buildFieldLabel(field.key);

    const isSecret = field.field_type === 'secret';
    const useTextArea = isTextAreaField(field.key);
    const useSelect = isSelectConfigField(field);
    const fieldExtra = isSecret
      ? i18nText("settings", "auto.sensitive_fields_used_encrypted_storage_echoed_lists_interfaces")
      : field.description ??
        (field.key === 'base_url'
          ? i18nText("settings", "auto.supports_input_standard_openai_compatible_addresses_filled_plug_value_used")
          : undefined);

    if (isSecret && mode === 'edit') {
      const configuredSecret = instance?.config_json[field.key];
      const hasConfiguredSecret =
        typeof configuredSecret === 'string' && configuredSecret.length > 0;

      if (!hasConfiguredSecret) {
        return (
          <Form.Item key={field.key} label={label} extra={fieldExtra}>
            <Input.Password
              aria-label={label}
              autoComplete="off"
              placeholder={field.placeholder ?? i18nText("settings", "auto.please_enter")}
              value={secretDrafts[field.key] ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                setSecretDrafts((current) => ({
                  ...current,
                  [field.key]: value
                }));
                clearPreviewState();
              }}
            />
          </Form.Item>
        );
      }

      const previewSource =
        secretDrafts[field.key] ??
        String(configuredSecret);
      const previewValue = previewSource
        ? previewSource.includes('****')
          ? previewSource
          : maskSecretPreview(previewSource)
        : i18nText("settings", "auto.not_configured");

      return (
        <Form.Item
          key={field.key}
          label={label}
          extra={i18nText("settings", "auto.leave_blank_retain_key_click_show_view_modify_value")}
        >
          {revealedSecretKeys[field.key] ? (
            <Space.Compact block>
              <Input
                aria-label={label}
                autoComplete="off"
                value={secretDrafts[field.key] ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  setSecretDrafts((current) => ({
                    ...current,
                    [field.key]: value
                  }));
                  clearPreviewState();
                }}
              />
              <Button
                onClick={() => {
                  clearPreviewState();
                  setRevealedSecretKeys((current) => ({
                    ...current,
                    [field.key]: false
                  }));
                }}
              >
                {i18nText("settings", "auto.hide")} {label}
              </Button>
            </Space.Compact>
          ) : (
            <Space.Compact block>
              <Input aria-label={label} readOnly value={previewValue} />
              <Button
                loading={revealingSecretKey === field.key}
                onClick={() => {
                  void handleRevealSecret(field.key).catch(() => undefined);
                }}
              >
                {i18nText("settings", "auto.show")} {label}
              </Button>
            </Space.Compact>
          )}
        </Form.Item>
      );
    }

    return (
      <Form.Item
        key={field.key}
        label={label}
        name={['config', field.key]}
        rules={
          field.required && (!isSecret || mode === 'create')
            ? [{ required: true, message: i18nText("settings", "auto.please_fill_in", { value1: label }) }]
            : undefined
        }
        extra={fieldExtra}
      >
        {isSecret ? (
          <Input.Password
            autoComplete="off"
            placeholder={field.placeholder ?? i18nText("settings", "auto.please_enter")}
          />
        ) : useSelect ? (
          <Select
            allowClear={!field.required}
            options={buildConfigSelectOptions(field)}
            placeholder={field.placeholder ?? i18nText("settings", "auto.please_enter")}
          />
        ) : useTextArea ? (
          <Input.TextArea
            rows={4}
            placeholder={
              field.placeholder ??
              (field.key === 'base_url' ? catalogEntry?.default_base_url ?? '' : undefined)
            }
          />
        ) : (
          <Input
            autoComplete={isSecret ? 'off' : undefined}
            placeholder={
              field.placeholder ??
              (field.key === 'base_url' ? catalogEntry?.default_base_url ?? '' : undefined)
            }
          />
        )}
      </Form.Item>
    );
  }

  return (
    <Drawer
      open={open}
      size={560}
      zIndex={1100}
      title={title}
      onClose={onClose}
      destroyOnHidden
      footer={
        <div style={{ textAlign: 'right' }}>
          <Space>
            <Button
              type="primary"
              loading={submitting}
              onClick={() => {
                void handleSubmit().catch(() => undefined);
              }}
            >
              {i18nText("settings", "auto.save")}</Button>
            <Button onClick={onClose}>{i18nText("settings", "auto.cancel")}</Button>
          </Space>
        </div>
      }
    >
      <Form
        form={form}
        layout="vertical"
        onValuesChange={(changedValues) => {
          if ('config' in changedValues) {
            clearPreviewState();
          }
        }}
      >
        {catalogEntry ? (
          <>
            <div className="model-provider-drawer__card">
              <div className="model-provider-drawer__card-title" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%', flexWrap: 'wrap', gap: 8 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <ApiOutlined style={{ color: 'var(--ant-color-primary)' }} />
                  <span>{catalogEntry.display_name}</span>
                </div>
                <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', fontWeight: 'normal', fontSize: '12px' }}>
                  <Tag color="blue" style={{ margin: 0 }}>{catalogEntry.provider_code}</Tag>
                  <Tag color="cyan" style={{ margin: 0 }}>{catalogEntry.protocol}</Tag>
                  <Tag color="purple" style={{ margin: 0 }}>{i18nText("settings", "auto.discovery_mode")}{catalogEntry.model_discovery_mode}</Tag>
                  <Tag color="gold" style={{ margin: 0 }}>{i18nText("settings", "auto.preset_models")}{catalogEntry.predefined_models.length}</Tag>
                </div>
              </div>
              <div className="model-provider-drawer__card-body">
                <Flex gap={16} align="flex-start">
                  <div style={{ flex: 1 }}>
                    <Form.Item
                      label={i18nText("settings", "auto.name")}
                      name="display_name"
                      rules={[{ required: true, message: i18nText("settings", "auto.fill_name") }]}
                      style={{ marginBottom: 0 }}
                    >
                      <Input placeholder={i18nText("settings", "auto.example_openai_production")} />
                    </Form.Item>
                  </div>
                  <div style={{ flex: 'none' }}>
                    <Form.Item
                      label={i18nText("settings", "auto.inject_main_instance_alt")}
                      name="included_in_main"
                      valuePropName="checked"
                      style={{ marginBottom: 0 }}
                    >
                      <Switch aria-label={i18nText("settings", "auto.inject_main_instance_alt")} />
                    </Form.Item>
                  </div>
                </Flex>
              </div>
            </div>

            <div className="model-provider-drawer__card">
              <div className="model-provider-drawer__card-title" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', width: '100%' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <CheckCircleOutlined />
                  <span>{i18nText("settings", "auto.connection_configuration")}</span>
                </div>
                <div>
                  <Button
                    size="small"
                    loading={previewingModels}
                    onClick={(e) => {
                      e.stopPropagation();
                      void handlePreviewModels().catch(() => undefined);
                    }}
                  >
                    {i18nText("settings", "auto.detection")}</Button>
                </div>
              </div>
              <div className="model-provider-drawer__card-body">
                {primaryConfigFields.map(renderConfigField)}
                {advancedConfigFields.length > 0 ? (
                  <div style={{ marginTop: 12 }}>
                    <CollapseShell
                      variant="compact"
                      items={[
                        {
                          key: 'advanced-config',
                          header: i18nText("settings", "auto.advanced_configuration_optional"),
                          children: advancedConfigFields.map(renderConfigField)
                        }
                      ]}
                    />
                  </div>
                ) : null}
              </div>
            </div>

            <div className="model-provider-drawer__card">
              <div className="model-provider-drawer__card-title">
                <PlusOutlined />
                <span>{i18nText("settings", "auto.model_configuration")}</span>
              </div>
              <div className="model-provider-drawer__card-body">
                <Space orientation="vertical" size={16} style={{ width: '100%' }}>
                  <Flex align="center" gap={12} style={{ width: '100%' }}>
                    <div style={{ flex: 1 }}>
                      <CachedModelSelect
                        modelIds={previewModels.map((model) => model.model_id)}
                        ariaLabel={i18nText("settings", "auto.cache_model")}
                        placeholder={i18nText("settings", "auto.cache_model")}
                        value={selectedCachedModelId}
                        emptyMode="select"
                        style={{ width: '100%' }}
                        onChange={applyCachedModelSelection}
                      />
                    </div>
                    <Button type="dashed" aria-label={i18nText("settings", "auto.new")} onClick={() => appendConfiguredModelRow()}>
                      {i18nText("settings", "auto.new")}</Button>
                    {previewModels.length > 0 && (
                      <Button
                        type="primary"
                        onClick={() => {
                          setConfiguredModels((current) => {
                            const existingIds = new Set(current.map((row) => row.model_id.trim()));
                            const newRows = [...current];
                            for (const pm of previewModels) {
                              const id = pm.model_id.trim();
                              if (id && !existingIds.has(id)) {
                                newRows.push({
                                  key: nextConfiguredModelKey(),
                                  model_id: id,
                                  context_window_input: '',
                                  context_window_error: null,
                                  supports_multimodal: pm.supports_multimodal,
                                  enabled: true
                                });
                                existingIds.add(id);
                              }
                            }
                            return newRows;
                          });
                        }}
                      >
                        {i18nText("settings", "auto.import_all")}</Button>
                    )}
                  </Flex>

                  <div className="model-provider-drawer__model-table">
                    <div
                      className="model-provider-drawer__model-header"
                      style={{
                        gridTemplateColumns: CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                        gap: CONFIGURED_MODEL_GRID_GAP,
                        alignItems: 'center'
                      }}
                    >
                      <Typography.Text strong style={{ color: 'inherit' }}>{i18nText("settings", "auto.model_id_alt")}</Typography.Text>
                      <Typography.Text strong style={{ color: 'inherit' }}>{i18nText("settings", "auto.context_alt")}</Typography.Text>
                      <Typography.Text strong style={{ textAlign: 'center', color: 'inherit' }}>
                        {i18nText("settings", "auto.multimodal")}</Typography.Text>
                      <Typography.Text strong style={{ textAlign: 'center', color: 'inherit' }}>
                        {i18nText("settings", "auto.enabled")}</Typography.Text>
                      <Typography.Text strong style={{ textAlign: 'center', color: 'inherit' }}>
                        {i18nText("settings", "auto.operation")}</Typography.Text>
                    </div>

                    {configuredModels.length > 0 ? (
                      configuredModels.map((row, index) => (
                        <div
                          key={row.key}
                          className="model-provider-drawer__model-row"
                          style={{
                            gridTemplateColumns: CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                            gap: CONFIGURED_MODEL_GRID_GAP,
                            alignItems: 'start'
                          }}
                        >
                          <div>
                            <AutoComplete
                              value={row.model_id}
                              options={modelAutocompleteOptions}
                              onChange={(value) => {
                                updateConfiguredModelRow(row.key, {
                                  model_id: String(value)
                                });
                              }}
                              placeholder={
                                previewModels.length > 0
                                  ? i18nText("settings", "auto.enter_select_model_id_detection_cache")
                                  : i18nText("settings", "auto.enter_model_id")
                              }
                              filterOption={(inputValue, option) =>
                                String(option?.value ?? '')
                                  .toLowerCase()
                                  .includes(inputValue.toLowerCase())
                              }
                              style={{ width: '100%' }}
                            >
                              <Input aria-label={i18nText("settings", "auto.model_id", { value1: index + 1 })} />
                            </AutoComplete>
                          </div>
                          <div>
                            <AutoComplete
                              value={row.context_window_input}
                              options={contextWindowOptions}
                              onChange={(value) => {
                                const parsedContextWindow = parseModelContextWindowInput(
                                  String(value)
                                );
                                updateConfiguredModelRow(row.key, {
                                  context_window_input: String(value),
                                  context_window_error: parsedContextWindow.error
                                });
                              }}
                              placeholder={i18nText("settings", "auto.example_one_two_eight_k")}
                              filterOption={(inputValue, option) =>
                                String(option?.value ?? '')
                                  .toLowerCase()
                                  .includes(inputValue.toLowerCase())
                              }
                              style={{ width: '100%' }}
                            >
                              <Input aria-label={i18nText("settings", "auto.context", { value1: index + 1 })} />
                            </AutoComplete>
                            {row.context_window_error ? (
                              <Typography.Text
                                type="danger"
                                style={{ display: 'block', marginTop: 4, fontSize: 12 }}
                              >
                                {row.context_window_error}
                              </Typography.Text>
                            ) : null}
                          </div>
                          <div style={{ display: 'flex', justifyContent: 'center', paddingTop: 5 }}>
                            <Switch
                              size="small"
                              aria-label={i18nText("settings", "auto.enable_multimodal_model", { value1: index + 1 })}
                              checked={row.supports_multimodal}
                              onChange={(checked) => {
                                updateConfiguredModelRow(row.key, {
                                  supports_multimodal: checked
                                });
                              }}
                            />
                          </div>
                          <div style={{ display: 'flex', justifyContent: 'center', paddingTop: 5 }}>
                            <Switch
                              size="small"
                              aria-label={i18nText("settings", "auto.enable_model", { value1: index + 1 })}
                              checked={row.enabled}
                              onChange={(checked) => {
                                updateConfiguredModelRow(row.key, {
                                  enabled: checked
                                });
                              }}
                            />
                          </div>
                          <div style={{ display: 'flex', justifyContent: 'center' }}>
                            <Button
                              danger
                              size="small"
                              type="text"
                              icon={<DeleteOutlined />}
                              aria-label={i18nText("settings", "auto.delete_model", { value1: index + 1 })}
                              className="model-provider-drawer__delete-btn"
                              style={{ height: 'auto', padding: '4px 8px' }}
                              onClick={() => removeConfiguredModelRow(row.key)}
                            />
                          </div>
                        </div>
                      ))
                    ) : (
                      <div
                        style={{
                          padding: '32px 16px',
                          textAlign: 'center'
                        }}
                      >
                        <Empty
                          image={Empty.PRESENTED_IMAGE_SIMPLE}
                          description={i18nText("settings", "auto.text_option")}
                        />
                      </div>
                    )}
                  </div>
                </Space>
              </div>
            </div>
          </>
        ) : (
          <Typography.Text type="secondary">{i18nText("settings", "auto.currently_provider_catalog_available")}</Typography.Text>
        )}
      </Form>
    </Drawer>
  );
}
