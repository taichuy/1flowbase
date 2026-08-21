import { useCallback, useEffect, useRef, useState } from 'react';

import {
  Button,
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
  EditOutlined,
  PlusOutlined
} from '@ant-design/icons';

import type {
  SettingsAuthenticateModelProviderInstanceResult,
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderAuthOperation,
  SettingsModelProviderInstance,
  SettingsModelProviderModelCatalog,
  SettingsModelProviderPricingTarget,
  SettingsModelProviderResetCreditCountResult,
  SettingsModelProviderUsageWindowsResult,
  PreviewSettingsModelProviderModelsResponse
} from '../../api/model-providers';
import { CollapseShell } from '../../../../shared/ui/collapse-shell/CollapseShell';
import { ResizableDrawer } from '../../../../shared/ui/resizable-drawer/ResizableDrawer';
import { CachedModelSelect } from './CachedModelSelect';
import {
  formatModelContextWindowValue,
  parseModelContextWindowInput
} from './model-context-window';
import { i18nText } from '../../../../shared/i18n/text';
import {
  ModelProviderConfiguredModelModal,
  type ConfiguredModelEditorValue
} from './ModelProviderConfiguredModelModal';
import { ModelProviderAccountOperationsCard } from './ModelProviderAccountOperationsCard';
import { ModelProviderAuthorizationCard } from './ModelProviderAuthorizationCard';

type DrawerMode = 'create' | 'edit';
type ModelProviderFormValue = string | boolean | number;
type ModelProviderConfigField =
  SettingsModelProviderCatalogEntry['form_schema'][number];
type PreviewModelDescriptor =
  SettingsModelProviderModelCatalog['models'][number];
type PreviewModelsResponse = PreviewSettingsModelProviderModelsResponse;
type AuthenticationResult = SettingsAuthenticateModelProviderInstanceResult;
type ConfiguredModelRow = {
  key: string;
  model_id: string;
  context_window_input: string;
  context_window_error: string | null;
  supports_multimodal: boolean;
  enabled: boolean;
  pricing_provider_code: string;
  pricing_model_id: string;
};

const CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS =
  'minmax(140px, 1fr) 100px minmax(120px, 0.8fr) 76px 48px 96px';
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
      nextConfig[field.key] = normalizeConfigFieldValue(
        field,
        field.default_value
      );
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
  return (
    key.includes('headers') || key.includes('json') || key.includes('schema')
  );
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
  pricingTargets: SettingsModelProviderPricingTarget[];
  defaultIncludedInMain: boolean;
  submitting: boolean;
  onClose: () => void | Promise<void>;
  onSubmit: (input: {
    display_name: string;
    included_in_main: boolean;
    config: Record<string, unknown>;
    configured_models: Array<{
      model_id: string;
      enabled: boolean;
      context_window_override_tokens: number | null;
      supports_multimodal: boolean;
      pricing_provider_code: string;
      pricing_model_id: string;
    }>;
    preview_token?: string;
  }) => Promise<void>;
  onPreviewModels: (
    config: Record<string, unknown>
  ) => Promise<PreviewModelsResponse>;
  onRevealSecret: (fieldKey: string) => Promise<string>;
  onAuthenticate: (
    operation: SettingsModelProviderAuthOperation
  ) => Promise<AuthenticationResult>;
  onCreateDraft?: (input: {
    display_name: string;
    included_in_main: boolean;
    config: Record<string, unknown>;
    operation: Extract<SettingsModelProviderAuthOperation, { type: 'begin' }>;
  }) => Promise<AuthenticationResult>;
  onDiscardDraft?: () => Promise<void>;
  onValidate?: () => Promise<void>;
  onRefreshUsage?: () => Promise<SettingsModelProviderUsageWindowsResult>;
  onCountResetCredits?: () => Promise<SettingsModelProviderResetCreditCountResult>;
  onConsumeResetCredit?: (input: {
    idempotency_key: string;
  }) => Promise<unknown>;
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
  pricingTargets,
  defaultIncludedInMain,
  submitting,
  onClose,
  onSubmit,
  onPreviewModels,
  onRevealSecret,
  onAuthenticate,
  onCreateDraft,
  onDiscardDraft,
  onValidate,
  onRefreshUsage,
  onCountResetCredits,
  onConsumeResetCredit
}: ModelProviderInstanceDrawerProps) {
  const [form] = Form.useForm<{
    display_name: string;
    included_in_main: boolean;
    config: Record<string, ModelProviderFormValue>;
  }>();
  const [secretDrafts, setSecretDrafts] = useState<Record<string, string>>({});
  const [revealedSecretKeys, setRevealedSecretKeys] = useState<
    Record<string, boolean>
  >({});
  const [revealingSecretKey, setRevealingSecretKey] = useState<string | null>(
    null
  );
  const [previewModels, setPreviewModels] = useState<PreviewModelDescriptor[]>(
    []
  );
  const configuredModelKeyRef = useRef(0);
  const [configuredModels, setConfiguredModels] = useState<
    ConfiguredModelRow[]
  >([]);
  const [selectedCachedModelId, setSelectedCachedModelId] = useState<
    string | undefined
  >();
  const [previewToken, setPreviewToken] = useState<string | undefined>();
  const [previewingModels, setPreviewingModels] = useState(false);
  const [usageSnapshot, setUsageSnapshot] =
    useState<SettingsModelProviderUsageWindowsResult | null>(null);
  const [configuredModelEditor, setConfiguredModelEditor] = useState<{
    rowKey: string | null;
    initialValue: ConfiguredModelEditorValue;
  } | null>(null);
  const [authenticationResult, setAuthenticationResult] =
    useState<AuthenticationResult | null>(null);
  const [authenticationError, setAuthenticationError] = useState<string | null>(
    null
  );
  const [authenticationRequestPending, setAuthenticationRequestPending] =
    useState(false);
  const [callbackValue, setCallbackValue] = useState('');
  const initializedDrawerRef = useRef<string | null>(null);
  const drawerIdentity = `${mode}:${catalogEntry?.installation_id ?? ''}:${instance?.id ?? ''}`;

  const nextConfiguredModelKey = useCallback(() => {
    const key = `configured-model-${configuredModelKeyRef.current}`;
    configuredModelKeyRef.current += 1;
    return key;
  }, []);

  const buildInitialConfiguredModels = useCallback(() => {
    const sourceModels =
      Array.isArray(instance?.configured_models) &&
      instance.configured_models.length > 0
        ? instance.configured_models
        : (instance?.enabled_model_ids ?? []).map((modelId) => ({
            model_id: modelId,
            enabled: true,
            context_window_override_tokens: null,
            supports_multimodal: null,
            pricing_provider_code: 'zero',
            pricing_model_id: 'any'
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
      enabled: model.enabled,
      pricing_provider_code: model.pricing_provider_code ?? 'zero',
      pricing_model_id: model.pricing_model_id ?? 'any'
    }));
  }, [instance, nextConfiguredModelKey]);

  useEffect(() => {
    if (!open) {
      initializedDrawerRef.current = null;
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
      setUsageSnapshot(null);
      setConfiguredModelEditor(null);
      setAuthenticationResult(null);
      setAuthenticationError(null);
      setAuthenticationRequestPending(false);
      setCallbackValue('');
      return;
    }

    if (initializedDrawerRef.current === drawerIdentity) {
      return;
    }
    initializedDrawerRef.current = drawerIdentity;

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
    setUsageSnapshot(null);
    setConfiguredModelEditor(null);
    setAuthenticationResult(null);
    setAuthenticationError(null);
    setAuthenticationRequestPending(false);
    setCallbackValue('');
  }, [
    buildInitialConfiguredModels,
    catalogEntry,
    defaultIncludedInMain,
    form,
    instance,
    mode,
    open,
    drawerIdentity
  ]);

  useEffect(() => {
    if (!open || !instance || !cachedModelCatalog || previewModels.length > 0) {
      return;
    }

    setPreviewModels(cachedModelCatalog.models);
  }, [cachedModelCatalog, instance, open, previewModels.length]);

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
      pricing_provider_code: string;
      pricing_model_id: string;
    }> = [];
    const seen = new Set<string>();
    let hasValidationError = false;

    setConfiguredModels((current) =>
      current.map((row) => {
        const parsedContextWindow = parseModelContextWindowInput(
          row.context_window_input
        );
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

      const parsedContextWindow = parseModelContextWindowInput(
        row.context_window_input
      );
      if (parsedContextWindow.error) {
        hasValidationError = true;
        continue;
      }

      seen.add(normalizedModelId);
      normalizedRows.push({
        model_id: normalizedModelId,
        enabled: row.enabled,
        context_window_override_tokens: parsedContextWindow.value,
        supports_multimodal: row.supports_multimodal,
        pricing_provider_code: row.pricing_provider_code,
        pricing_model_id: row.pricing_model_id
      });
    }

    return {
      hasValidationError,
      rows: normalizedRows
    };
  }

  function openConfiguredModelEditor(row?: ConfiguredModelRow) {
    const modelId = row?.model_id ?? selectedCachedModelId ?? '';
    const previewModel = previewModels.find(
      (model) => model.model_id === modelId
    );
    setConfiguredModelEditor({
      rowKey: row?.key ?? null,
      initialValue: {
        model_id: modelId,
        context_window_input: row?.context_window_input ?? '',
        supports_multimodal:
          row?.supports_multimodal ??
          previewModel?.supports_multimodal ??
          false,
        enabled: row?.enabled ?? true,
        pricing_provider_code: row?.pricing_provider_code ?? 'zero',
        pricing_model_id: row?.pricing_model_id ?? 'any'
      }
    });
  }

  function saveConfiguredModel(value: ConfiguredModelEditorValue) {
    setConfiguredModels((current) => {
      if (configuredModelEditor?.rowKey) {
        return current.map((row) =>
          row.key === configuredModelEditor.rowKey
            ? {
                ...row,
                ...value,
                context_window_error: null
              }
            : row
        );
      }
      return [
        ...current,
        {
          key: nextConfiguredModelKey(),
          ...value,
          context_window_error: null
        }
      ];
    });
    setConfiguredModelEditor(null);
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
      setRevealingSecretKey((current) =>
        current === fieldKey ? null : current
      );
    }
  }

  const title =
    mode === 'create'
      ? i18nText('settings', 'auto.api_key_authorization_configuration')
      : i18nText('settings', 'auto.edit_api_key_configuration');
  const formSchema = (catalogEntry?.form_schema ?? []).filter(
    (field) => !isPreviewOnlyField(field)
  );
  const editableConfigFields = formSchema.filter(
    (field) => !(mode === 'edit' && field.field_type === 'secret')
  );
  const configFieldNames = editableConfigFields.map(
    (field) => ['config', field.key] as const
  );
  const primaryConfigFields = formSchema.filter((field) => !field.advanced);
  const advancedConfigFields = formSchema.filter((field) => field.advanced);
  function buildDraftConfig(
    valuesConfig: Record<string, ModelProviderFormValue>
  ) {
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

  const runAuthenticationOperation = useCallback(
    async (operation: SettingsModelProviderAuthOperation) => {
      setAuthenticationRequestPending(true);
      setAuthenticationError(null);

      try {
        const result =
          operation.type === 'begin' && !instance && onCreateDraft
            ? await (async () => {
                const values = await form.validateFields([
                  ['display_name'],
                  ['included_in_main'],
                  ...configFieldNames
                ]);
                return onCreateDraft({
                  display_name: values.display_name,
                  included_in_main: values.included_in_main,
                  config: buildDraftConfig(
                    (values.config ?? {}) as Record<
                      string,
                      ModelProviderFormValue
                    >
                  ),
                  operation
                });
              })()
            : await onAuthenticate(operation);
        setAuthenticationResult(result);
        if (result.status !== 'pending') {
          setCallbackValue('');
        }
        return result;
      } catch (error) {
        setAuthenticationError(
          error instanceof Error
            ? error.message
            : i18nText('settings', 'auto.provider_authentication_failed')
        );
        throw error;
      } finally {
        setAuthenticationRequestPending(false);
      }
    },
    [
      buildDraftConfig,
      configFieldNames,
      form,
      instance,
      onAuthenticate,
      onCreateDraft
    ]
  );

  useEffect(() => {
    const userAction = authenticationResult?.user_action;
    if (authenticationResult?.status !== 'pending') {
      return;
    }

    const pollIntervalSeconds = Math.max(
      1,
      userAction?.poll_interval_seconds ?? 5
    );
    const timer = window.setTimeout(() => {
      void runAuthenticationOperation({ type: 'poll' }).catch(() => undefined);
    }, pollIntervalSeconds * 1000);

    return () => window.clearTimeout(timer);
  }, [authenticationResult, runAuthenticationOperation]);

  useEffect(() => {
    const expiresAt = authenticationResult?.user_action?.expires_at;
    if (
      mode !== 'create' ||
      !instance ||
      !onDiscardDraft ||
      authenticationResult?.status !== 'pending' ||
      !expiresAt
    ) {
      return;
    }

    const expiresAtMilliseconds = Date.parse(expiresAt);
    if (!Number.isFinite(expiresAtMilliseconds)) {
      return;
    }

    const timer = window.setTimeout(
      () => {
        void onDiscardDraft().catch(() => undefined);
      },
      Math.max(0, expiresAtMilliseconds - Date.now())
    );
    return () => window.clearTimeout(timer);
  }, [authenticationResult, instance, mode, onDiscardDraft]);

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
    setConfiguredModels((current) =>
      current.filter((row) => row.key !== rowKey)
    );
  }

  async function refreshUsageSnapshot() {
    if (!onRefreshUsage) {
      return null;
    }

    const snapshot = await onRefreshUsage();
    setUsageSnapshot(snapshot);
    return snapshot;
  }

  async function handlePreviewModels() {
    setPreviewingModels(true);

    try {
      if (instance && onValidate) {
        await onValidate();
        await refreshUsageSnapshot();
        return;
      }

      const values = await form.validateFields(configFieldNames);
      const preview = await onPreviewModels(
        buildDraftConfig(
          (values.config ?? {}) as Record<string, ModelProviderFormValue>
        )
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
    const normalizedConfiguredModels =
      normalizeConfiguredModels(configuredModels);
    if (normalizedConfiguredModels.hasValidationError) {
      return;
    }

    await onSubmit({
      display_name: values.display_name,
      included_in_main: values.included_in_main,
      config: buildDraftConfig(
        (values.config ?? {}) as Record<string, ModelProviderFormValue>
      ),
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
      ? i18nText(
          'settings',
          'auto.sensitive_fields_used_encrypted_storage_echoed_lists_interfaces'
        )
      : (field.description ??
        (field.key === 'base_url'
          ? i18nText(
              'settings',
              'auto.supports_input_standard_openai_compatible_addresses_filled_plug_value_used'
            )
          : undefined));

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
              placeholder={
                field.placeholder ?? i18nText('settings', 'auto.please_enter')
              }
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

      const previewSource = secretDrafts[field.key] ?? String(configuredSecret);
      const previewValue = previewSource
        ? previewSource.includes('****')
          ? previewSource
          : maskSecretPreview(previewSource)
        : i18nText('settings', 'auto.not_configured');

      return (
        <Form.Item
          key={field.key}
          label={label}
          extra={i18nText(
            'settings',
            'auto.leave_blank_retain_key_click_show_view_modify_value'
          )}
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
                {i18nText('settings', 'auto.hide')} {label}
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
                {i18nText('settings', 'auto.show')} {label}
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
            ? [
                {
                  required: true,
                  message: i18nText('settings', 'auto.please_fill_in', {
                    value1: label
                  })
                }
              ]
            : undefined
        }
        extra={fieldExtra}
      >
        {isSecret ? (
          <Input.Password
            autoComplete="off"
            placeholder={
              field.placeholder ?? i18nText('settings', 'auto.please_enter')
            }
          />
        ) : useSelect ? (
          <Select
            allowClear={!field.required}
            options={buildConfigSelectOptions(field)}
            placeholder={
              field.placeholder ?? i18nText('settings', 'auto.please_enter')
            }
          />
        ) : useTextArea ? (
          <Input.TextArea
            rows={4}
            placeholder={
              field.placeholder ??
              (field.key === 'base_url'
                ? (catalogEntry?.default_base_url ?? '')
                : undefined)
            }
          />
        ) : (
          <Input
            autoComplete={isSecret ? 'off' : undefined}
            placeholder={
              field.placeholder ??
              (field.key === 'base_url'
                ? (catalogEntry?.default_base_url ?? '')
                : undefined)
            }
          />
        )}
      </Form.Item>
    );
  }

  function renderAuthenticationCard() {
    if (!catalogEntry?.auth || (!instance && !onCreateDraft)) {
      return null;
    }
    return (
      <ModelProviderAuthorizationCard
        catalogEntry={catalogEntry}
        result={authenticationResult}
        errorMessage={authenticationError}
        pending={authenticationRequestPending}
        callbackValue={callbackValue}
        onCallbackValueChange={setCallbackValue}
        onBegin={(action) => {
          void runAuthenticationOperation({ type: 'begin', action }).catch(
            () => undefined
          );
        }}
        onSubmit={(value) => {
          void runAuthenticationOperation({ type: 'submit', value }).catch(
            () => undefined
          );
        }}
        onCancel={() => {
          if (mode === 'create' && instance && onDiscardDraft) {
            void onDiscardDraft().catch(() => undefined);
            return;
          }
          void runAuthenticationOperation({ type: 'cancel' }).catch(
            () => undefined
          );
        }}
      />
    );
  }

  return (
    <>
      <ResizableDrawer
        defaultWidth={560}
        maxWidth={1200}
        minWidth={480}
        open={open}
        zIndex={1100}
        title={title}
        onClose={onClose}
        destroyOnClose
        resizeLabel="调整供应商抽屉宽度"
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
                {i18nText('settings', 'auto.save')}
              </Button>
              <Button onClick={onClose}>
                {i18nText('settings', 'auto.cancel')}
              </Button>
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
                <div
                  className="model-provider-drawer__card-title"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    width: '100%',
                    flexWrap: 'wrap',
                    gap: 8
                  }}
                >
                  <div
                    style={{ display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    <ApiOutlined
                      style={{ color: 'var(--ant-color-primary)' }}
                    />
                    <span>{catalogEntry.display_name}</span>
                  </div>
                  <div
                    style={{
                      display: 'flex',
                      gap: 4,
                      flexWrap: 'wrap',
                      fontWeight: 'normal',
                      fontSize: '12px'
                    }}
                  >
                    <Tag color="blue" style={{ margin: 0 }}>
                      {catalogEntry.provider_code}
                    </Tag>
                    <Tag color="cyan" style={{ margin: 0 }}>
                      {catalogEntry.protocol}
                    </Tag>
                    <Tag color="purple" style={{ margin: 0 }}>
                      {i18nText('settings', 'auto.discovery_mode')}
                      {catalogEntry.model_discovery_mode}
                    </Tag>
                    <Tag color="gold" style={{ margin: 0 }}>
                      {i18nText('settings', 'auto.preset_models')}
                      {catalogEntry.predefined_models.length}
                    </Tag>
                  </div>
                </div>
                <div className="model-provider-drawer__card-body">
                  <Flex gap={16} align="flex-start">
                    <div style={{ flex: 1 }}>
                      <Form.Item
                        label={i18nText('settings', 'auto.name')}
                        name="display_name"
                        rules={[
                          {
                            required: true,
                            message: i18nText('settings', 'auto.fill_name')
                          }
                        ]}
                        style={{ marginBottom: 0 }}
                      >
                        <Input
                          placeholder={i18nText(
                            'settings',
                            'auto.example_openai_production'
                          )}
                        />
                      </Form.Item>
                    </div>
                    <div style={{ flex: 'none' }}>
                      <Form.Item
                        label={i18nText(
                          'settings',
                          'auto.inject_main_instance_alt'
                        )}
                        name="included_in_main"
                        valuePropName="checked"
                        style={{ marginBottom: 0 }}
                      >
                        <Switch
                          aria-label={i18nText(
                            'settings',
                            'auto.inject_main_instance_alt'
                          )}
                        />
                      </Form.Item>
                    </div>
                  </Flex>
                </div>
              </div>

              {renderAuthenticationCard()}

              <div className="model-provider-drawer__card">
                <div
                  className="model-provider-drawer__card-title"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    width: '100%'
                  }}
                >
                  <div
                    style={{ display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    <CheckCircleOutlined />
                    <span>
                      {i18nText('settings', 'auto.connection_configuration')}
                    </span>
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
                      {i18nText('settings', 'auto.detection')}
                    </Button>
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
                            header: i18nText(
                              'settings',
                              'auto.advanced_configuration_optional'
                            ),
                            children:
                              advancedConfigFields.map(renderConfigField)
                          }
                        ]}
                      />
                    </div>
                  ) : null}
                </div>
              </div>

              {instance ? (
                <ModelProviderAccountOperationsCard
                  catalogEntry={catalogEntry}
                  usageSnapshot={usageSnapshot}
                  onUsageSnapshot={setUsageSnapshot}
                  onRefreshUsage={
                    onRefreshUsage ? refreshUsageSnapshot : undefined
                  }
                  onCountResetCredits={onCountResetCredits}
                  onConsumeResetCredit={onConsumeResetCredit}
                />
              ) : null}

              <div className="model-provider-drawer__card">
                <div className="model-provider-drawer__card-title">
                  <PlusOutlined />
                  <span>
                    {i18nText('settings', 'auto.model_configuration')}
                  </span>
                </div>
                <div className="model-provider-drawer__card-body">
                  <Space
                    orientation="vertical"
                    size={16}
                    style={{ width: '100%' }}
                  >
                    <Flex align="center" gap={12} style={{ width: '100%' }}>
                      <div style={{ flex: 1 }}>
                        <CachedModelSelect
                          modelIds={previewModels.map(
                            (model) => model.model_id
                          )}
                          ariaLabel={i18nText('settings', 'auto.cache_model')}
                          placeholder={i18nText('settings', 'auto.cache_model')}
                          value={selectedCachedModelId}
                          emptyMode="select"
                          style={{ width: '100%' }}
                          onChange={applyCachedModelSelection}
                        />
                      </div>
                      <Button
                        type="dashed"
                        aria-label={i18nText('settings', 'auto.new')}
                        onClick={() => openConfiguredModelEditor()}
                      >
                        {i18nText('settings', 'auto.new')}
                      </Button>
                      {previewModels.length > 0 && (
                        <Button
                          type="primary"
                          onClick={() => {
                            setConfiguredModels((current) => {
                              const existingIds = new Set(
                                current.map((row) => row.model_id.trim())
                              );
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
                                    enabled: true,
                                    pricing_provider_code: 'zero',
                                    pricing_model_id: 'any'
                                  });
                                  existingIds.add(id);
                                }
                              }
                              return newRows;
                            });
                          }}
                        >
                          {i18nText('settings', 'auto.import_all')}
                        </Button>
                      )}
                    </Flex>

                    <div className="model-provider-drawer__model-table">
                      <div
                        className="model-provider-drawer__model-header"
                        style={{
                          gridTemplateColumns:
                            CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                          gap: CONFIGURED_MODEL_GRID_GAP,
                          alignItems: 'center'
                        }}
                      >
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.model_id_alt')}
                        </Typography.Text>
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.context_alt')}
                        </Typography.Text>
                        <Typography.Text strong style={{ color: 'inherit' }}>
                          {i18nText('settings', 'auto.billing_pricing_rules')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.multimodal')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.enabled')}
                        </Typography.Text>
                        <Typography.Text
                          strong
                          style={{ textAlign: 'center', color: 'inherit' }}
                        >
                          {i18nText('settings', 'auto.operation')}
                        </Typography.Text>
                      </div>

                      {configuredModels.length > 0 ? (
                        configuredModels.map((row, index) => (
                          <div
                            key={row.key}
                            className="model-provider-drawer__model-row"
                            style={{
                              gridTemplateColumns:
                                CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS,
                              gap: CONFIGURED_MODEL_GRID_GAP,
                              alignItems: 'start'
                            }}
                          >
                            <Typography.Text
                              ellipsis={{ tooltip: row.model_id }}
                            >
                              {row.model_id}
                            </Typography.Text>
                            <Typography.Text>
                              {row.context_window_input || '—'}
                            </Typography.Text>
                            <Typography.Text
                              ellipsis={{
                                tooltip: `${row.pricing_provider_code} / ${row.pricing_model_id}`
                              }}
                            >
                              {row.pricing_provider_code} /{' '}
                              {row.pricing_model_id}
                            </Typography.Text>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                paddingTop: 5
                              }}
                            >
                              <Switch
                                size="small"
                                aria-label={i18nText(
                                  'settings',
                                  'auto.enable_multimodal_model',
                                  { value1: index + 1 }
                                )}
                                checked={row.supports_multimodal}
                                onChange={(checked) => {
                                  updateConfiguredModelRow(row.key, {
                                    supports_multimodal: checked
                                  });
                                }}
                              />
                            </div>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                paddingTop: 5
                              }}
                            >
                              <Switch
                                size="small"
                                aria-label={i18nText(
                                  'settings',
                                  'auto.enable_model',
                                  { value1: index + 1 }
                                )}
                                checked={row.enabled}
                                onChange={(checked) => {
                                  updateConfiguredModelRow(row.key, {
                                    enabled: checked
                                  });
                                }}
                              />
                            </div>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'center',
                                gap: 2
                              }}
                            >
                              <Button
                                size="small"
                                type="text"
                                icon={<EditOutlined />}
                                aria-label={i18nText(
                                  'settings',
                                  'auto.edit_model',
                                  { value1: index + 1 }
                                )}
                                onClick={() => openConfiguredModelEditor(row)}
                              />
                              <Button
                                danger
                                size="small"
                                type="text"
                                icon={<DeleteOutlined />}
                                aria-label={i18nText(
                                  'settings',
                                  'auto.delete_model',
                                  { value1: index + 1 }
                                )}
                                className="model-provider-drawer__delete-btn"
                                style={{ height: 'auto', padding: '4px 8px' }}
                                onClick={() =>
                                  removeConfiguredModelRow(row.key)
                                }
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
                            description={i18nText(
                              'settings',
                              'auto.text_option'
                            )}
                          />
                        </div>
                      )}
                    </div>
                  </Space>
                </div>
              </div>
            </>
          ) : (
            <Typography.Text type="secondary">
              {i18nText(
                'settings',
                'auto.currently_provider_catalog_available'
              )}
            </Typography.Text>
          )}
        </Form>
      </ResizableDrawer>
      <ModelProviderConfiguredModelModal
        open={configuredModelEditor !== null}
        editing={Boolean(configuredModelEditor?.rowKey)}
        initialValue={configuredModelEditor?.initialValue ?? null}
        modelIds={previewModels.map((model) => model.model_id)}
        reservedModelIds={configuredModels.reduce<string[]>((reserved, row) => {
          if (row.key !== configuredModelEditor?.rowKey) {
            reserved.push(row.model_id);
          }
          return reserved;
        }, [])}
        pricingTargets={pricingTargets}
        onCancel={() => setConfiguredModelEditor(null)}
        onSave={saveConfiguredModel}
      />
    </>
  );
}
