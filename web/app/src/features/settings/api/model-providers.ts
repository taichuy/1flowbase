import {
  createConsoleModelProviderInstance,
  deleteConsoleModelProviderInstance,
  getConsoleModelProviderMainInstance,
  getConsoleModelProviderModels,
  listConsoleModelProviderCatalog,
  listConsoleModelProviderInstances,
  listConsoleModelProviderOptions,
  listConsoleModelProviderRequestLogs,
  previewConsoleModelProviderModels,
  revealConsoleModelProviderSecret,
  refreshConsoleModelProviderModels,
  updateConsoleModelProviderInstance,
  updateConsoleModelProviderMainInstance,
  validateConsoleModelProviderInstance,
  type ConsoleModelProviderCatalogEntry,
  type ConsoleModelProviderCatalogResponse,
  type ConsoleModelProviderInstance,
  type ConsoleModelProviderMainInstance,
  type ConsoleModelProviderModelCatalog,
  type ConsoleModelProviderOptions,
  type ConsoleModelProviderRequestLogsFilter,
  type ConsoleValidateModelProviderResult,
  type CreateConsoleModelProviderInput,
  type PreviewConsoleModelProviderModelsInput,
  type PreviewConsoleModelProviderModelsResponse,
  type RevealConsoleModelProviderSecretResult,
  type UpdateConsoleModelProviderInput,
  type UpdateConsoleModelProviderMainInstanceInput
} from '@1flowbase/api-client';

export type SettingsModelProviderCatalogEntry =
  ConsoleModelProviderCatalogEntry;
export type SettingsModelProviderInstance = ConsoleModelProviderInstance;
export type SettingsModelProviderOptions = ConsoleModelProviderOptions;
export type SettingsModelProviderModelCatalog =
  ConsoleModelProviderModelCatalog;
export type SettingsRevealModelProviderSecretResult =
  RevealConsoleModelProviderSecretResult;
export type SettingsValidateModelProviderResult =
  ConsoleValidateModelProviderResult;
export type SettingsModelProviderMainInstance =
  ConsoleModelProviderMainInstance;
export type CreateSettingsModelProviderInput = CreateConsoleModelProviderInput;
export type PreviewSettingsModelProviderModelsInput =
  PreviewConsoleModelProviderModelsInput;
export type PreviewSettingsModelProviderModelsResponse =
  PreviewConsoleModelProviderModelsResponse;
export type UpdateSettingsModelProviderInput = UpdateConsoleModelProviderInput;
export type UpdateSettingsModelProviderMainInstanceInput =
  UpdateConsoleModelProviderMainInstanceInput;

export const settingsModelProviderCatalogQueryKey = [
  'settings',
  'model-providers',
  'catalog'
] as const;
export const settingsModelProviderInstancesQueryKey = [
  'settings',
  'model-providers',
  'instances'
] as const;
export const settingsModelProviderOptionsQueryKey = [
  'settings',
  'model-providers',
  'options'
] as const;

export function settingsModelProviderModelsQueryKey(instanceId: string) {
  return ['settings', 'model-providers', 'models', instanceId] as const;
}

function pickCatalogLocales(localeMeta: Record<string, unknown>) {
  const candidates = [
    localeMeta.resolved_locale,
    localeMeta.fallback_locale,
    'zh_Hans',
    'en_US'
  ];

  return candidates.filter(
    (value, index): value is string =>
      typeof value === 'string' && candidates.indexOf(value) === index
  );
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null;
}

function readCatalogLocalizedValue(
  bundle: Record<string, unknown>,
  dottedKey: string
): string | null {
  let current: unknown = bundle;

  for (const segment of dottedKey.split('.')) {
    current = asRecord(current)?.[segment];
    if (current === undefined) {
      return null;
    }
  }

  return typeof current === 'string' ? current : null;
}

function resolveCatalogLocalizedValue(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  namespace: string,
  value: string
): string;
function resolveCatalogLocalizedValue(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  namespace: string,
  value: null | undefined
): null | undefined;
function resolveCatalogLocalizedValue(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  namespace: string,
  value: string | null | undefined
) {
  if (!value) {
    return value;
  }

  const namespaceCatalog = asRecord(response.i18n_catalog)?.[namespace];
  const localeCatalog = asRecord(namespaceCatalog);
  if (!localeCatalog) {
    return value;
  }

  for (const locale of pickCatalogLocales(response.locale_meta)) {
    const localizedBundle = asRecord(localeCatalog[locale]);
    if (!localizedBundle) {
      continue;
    }

    const localizedValue = readCatalogLocalizedValue(localizedBundle, value);
    if (localizedValue) {
      return localizedValue;
    }
  }

  for (const localizedBundle of Object.values(localeCatalog)) {
    const normalizedBundle = asRecord(localizedBundle);
    if (!normalizedBundle) {
      continue;
    }

    const localizedValue = readCatalogLocalizedValue(normalizedBundle, value);
    if (localizedValue) {
      return localizedValue;
    }
  }

  return value;
}

function resolveOptionalCatalogLocalizedValue(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  namespace: string,
  value: string | null | undefined
) {
  return value == null
    ? value
    : resolveCatalogLocalizedValue(response, namespace, value);
}

function localizeCatalogEntryConfigSchema(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  entry: ConsoleModelProviderCatalogEntry
) {
  return {
    ...entry,
    form_schema: entry.form_schema.map((field) => {
      const label = resolveOptionalCatalogLocalizedValue(
        response,
        entry.namespace,
        field.label
      );
      const description = resolveOptionalCatalogLocalizedValue(
        response,
        entry.namespace,
        field.description
      );
      const placeholder = resolveOptionalCatalogLocalizedValue(
        response,
        entry.namespace,
        field.placeholder
      );

      return {
        ...field,
        ...(label === undefined ? {} : { label }),
        ...(description === undefined ? {} : { description }),
        ...(placeholder === undefined ? {} : { placeholder }),
        ...(field.options
          ? {
              options: field.options.map((option) => {
                const optionDescription = resolveOptionalCatalogLocalizedValue(
                  response,
                  entry.namespace,
                  option.description
                );

                return {
                  ...option,
                  label: resolveCatalogLocalizedValue(
                    response,
                    entry.namespace,
                    option.label
                  ),
                  ...(optionDescription === undefined ||
                  optionDescription === null
                    ? {}
                    : { description: optionDescription })
                };
              })
            }
          : {})
      };
    })
  };
}

export function fetchSettingsModelProviderCatalog(locale?: string) {
  return listConsoleModelProviderCatalog({ locale }).then((response) =>
    response.entries.map((entry) =>
      localizeCatalogEntryConfigSchema(response, entry)
    )
  );
}

export function fetchSettingsModelProviderInstances() {
  return listConsoleModelProviderInstances();
}

export function fetchSettingsModelProviderOptions() {
  return listConsoleModelProviderOptions();
}

export function fetchSettingsModelProviderMainInstance(providerCode: string) {
  return getConsoleModelProviderMainInstance(providerCode);
}

export function fetchSettingsModelProviderModels(instanceId: string) {
  return getConsoleModelProviderModels(instanceId);
}

export function previewSettingsModelProviderModels(
  input: PreviewSettingsModelProviderModelsInput,
  csrfToken: string
) {
  return previewConsoleModelProviderModels(input, csrfToken);
}

export function createSettingsModelProviderInstance(
  input: CreateSettingsModelProviderInput,
  csrfToken: string
) {
  return createConsoleModelProviderInstance(input, csrfToken);
}

export function updateSettingsModelProviderInstance(
  instanceId: string,
  input: UpdateSettingsModelProviderInput,
  csrfToken: string
) {
  return updateConsoleModelProviderInstance(instanceId, input, csrfToken);
}

export function updateSettingsModelProviderMainInstance(
  providerCode: string,
  input: UpdateSettingsModelProviderMainInstanceInput,
  csrfToken: string
) {
  return updateConsoleModelProviderMainInstance(providerCode, input, csrfToken);
}

export function validateSettingsModelProviderInstance(
  instanceId: string,
  csrfToken: string
) {
  return validateConsoleModelProviderInstance(instanceId, csrfToken);
}

export function refreshSettingsModelProviderModels(
  instanceId: string,
  csrfToken: string
) {
  return refreshConsoleModelProviderModels(instanceId, csrfToken);
}

export function revealSettingsModelProviderSecret(
  instanceId: string,
  key: string,
  csrfToken: string
) {
  return revealConsoleModelProviderSecret(instanceId, key, csrfToken);
}

export function deleteSettingsModelProviderInstance(
  instanceId: string,
  csrfToken: string
) {
  return deleteConsoleModelProviderInstance(instanceId, csrfToken);
}

export function settingsModelProviderRequestLogsQueryKey(
  filter: ConsoleModelProviderRequestLogsFilter
) {
  return ['settings', 'model-providers', 'request-logs', filter] as const;
}

export function fetchSettingsModelProviderRequestLogs(
  filter: ConsoleModelProviderRequestLogsFilter
) {
  return listConsoleModelProviderRequestLogs(filter);
}
