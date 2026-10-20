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

type DrawerMode = 'create' | 'edit';

export type ModelProviderFormValue = string | boolean | number;

export type ModelProviderConfigField =
  SettingsModelProviderCatalogEntry['form_schema'][number];

export type PreviewModelDescriptor =
  SettingsModelProviderModelCatalog['models'][number];

type PreviewModelsResponse = PreviewSettingsModelProviderModelsResponse;

export type AuthenticationResult =
  SettingsAuthenticateModelProviderInstanceResult;

export type ConfiguredModelRow = {
  key: string;
  model_id: string;
  context_window_input: string;
  context_window_error: string | null;
  supports_multimodal: boolean;
  enabled: boolean;
  pricing_provider_code: string;
  pricing_model_id: string;
};

export const CONFIGURED_MODEL_GRID_TEMPLATE_COLUMNS =
  'minmax(140px, 1fr) 100px minmax(120px, 0.8fr) 76px 48px 96px';

export const CONFIGURED_MODEL_GRID_GAP = 8;

export function isSelectConfigField(field: ModelProviderConfigField) {
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

export function buildFieldLabel(key: string) {
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

export function buildConfigSelectOptions(field: ModelProviderConfigField) {
  return (field.options ?? []).map((option) => ({
    label: option.label || String(option.value ?? ''),
    value: toSelectOptionValue(option.value),
    disabled: option.disabled ?? false
  }));
}

export function resolveDraftConfigValue(
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

export function maskSecretPreview(value: string) {
  if (value.length <= 8) {
    return '****';
  }

  return `${value.slice(0, 4)}****${value.slice(-4)}`;
}

export function buildInitialConfig(
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

export function isTextAreaField(key: string) {
  return (
    key.includes('headers') || key.includes('json') || key.includes('schema')
  );
}

export function isPreviewOnlyField(field: ModelProviderConfigField) {
  return field.key === 'validate_model';
}

export function shouldOmitDraftConfigValue(
  value: ModelProviderFormValue | undefined
) {
  return typeof value === 'string' && value.length === 0;
}

export type ModelProviderInstanceDrawerProps = {
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
