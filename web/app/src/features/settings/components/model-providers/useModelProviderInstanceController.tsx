import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { Button, Form, Input, Select, Space } from 'antd';

import type {
  SettingsModelProviderAuthOperation,
  SettingsModelProviderUsageWindowsResult
} from '../../api/model-providers';

import {
  formatModelContextWindowValue,
  parseModelContextWindowInput
} from './model-context-window';
import { i18nText } from '../../../../shared/i18n/text';
import { type ConfiguredModelEditorValue } from './ModelProviderConfiguredModelModal';

import { ModelProviderAuthorizationCard } from './ModelProviderAuthorizationCard';
export function useModelProviderInstanceController({
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
  const supportsConfigurationValidation =
    catalogEntry?.operational_capabilities.includes('validate_config') ?? false;
  const supportsModelListing =
    catalogEntry?.operational_capabilities.includes('list_models') ?? false;
  const canDetect = instance
    ? supportsConfigurationValidation && Boolean(onValidate)
    : supportsModelListing;
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
  const formSchema = useMemo(
    () =>
      (catalogEntry?.form_schema ?? []).filter(
        (field) => !isPreviewOnlyField(field)
      ),
    [catalogEntry?.form_schema]
  );
  const editableConfigFields = useMemo(
    () =>
      formSchema.filter(
        (field) => !(mode === 'edit' && field.field_type === 'secret')
      ),
    [formSchema, mode]
  );
  const configFieldNames = useMemo(
    () => editableConfigFields.map((field) => ['config', field.key] as const),
    [editableConfigFields]
  );
  const primaryConfigFields = formSchema.filter((field) => !field.advanced);
  const advancedConfigFields = formSchema.filter((field) => field.advanced);
  const buildDraftConfig = useCallback(
    (valuesConfig: Record<string, ModelProviderFormValue>) => {
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
    },
    [editableConfigFields, mode, catalogEntry, secretDrafts]
  );

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

  async function refreshUsageSnapshot(): Promise<SettingsModelProviderUsageWindowsResult> {
    if (!onRefreshUsage) {
      throw new Error('Usage refresh is unavailable.');
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
  return {
    form,
    previewModels,
    configuredModels,
    setConfiguredModels,
    selectedCachedModelId,
    previewingModels,
    usageSnapshot,
    setUsageSnapshot,
    configuredModelEditor,
    setConfiguredModelEditor,
    canDetect,
    nextConfiguredModelKey,
    clearPreviewState,
    openConfiguredModelEditor,
    saveConfiguredModel,
    applyCachedModelSelection,
    title,
    primaryConfigFields,
    advancedConfigFields,
    updateConfiguredModelRow,
    removeConfiguredModelRow,
    refreshUsageSnapshot,
    handlePreviewModels,
    handleSubmit,
    renderConfigField,
    renderAuthenticationCard
  };
}

import {
  type ModelProviderFormValue,
  type ModelProviderConfigField,
  type PreviewModelDescriptor,
  type AuthenticationResult,
  type ConfiguredModelRow,
  isSelectConfigField,
  buildFieldLabel,
  buildConfigSelectOptions,
  resolveDraftConfigValue,
  maskSecretPreview,
  buildInitialConfig,
  isTextAreaField,
  isPreviewOnlyField,
  shouldOmitDraftConfigValue,
  type ModelProviderInstanceDrawerProps
} from './model-provider-instance-config';
