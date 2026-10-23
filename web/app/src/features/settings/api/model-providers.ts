import {
  listConsoleModelProviderCatalog,
  listConsoleModelProviderInstances,
  listConsoleSettingsModelProviderOptions,
  type ConsoleModelProviderCatalogEntry,
  type ConsoleModelProviderCatalogResponse,
  type ConsoleModelProviderAuthOperation,
  type ConsoleAuthenticateModelProviderInstanceResult,
  type ConsoleModelProviderInstance,
  type ConsoleModelProviderMainInstance,
  type ConsoleModelProviderModelCatalog,
  type ConsoleModelProviderOptions,
  type ConsoleModelProviderPricingTarget,
  type ConsoleModelProviderResetCreditCountResult,
  type ConsoleModelProviderRequestLogsFilter,
  type ConsoleModelProviderUsageWindowsResult,
  type ConsumeConsoleModelProviderResetCreditInput,
  type ConsumeConsoleModelProviderResetCreditResult,
  type ConsoleValidateModelProviderResult,
  type CreateConsoleModelProviderInput,
  type PreviewConsoleModelProviderModelsInput,
  type PreviewConsoleModelProviderModelsResponse,
  type RevealConsoleModelProviderSecretResult,
  type UpdateConsoleModelProviderInput,
  type UpdateConsoleModelProviderMainInstanceInput
} from '@1flowbase/api-client';
import { lookupCatalogText } from './catalog/localized-text';

export {
  getConsoleModelProviderMainInstance as fetchSettingsModelProviderMainInstance,
  getConsoleModelProviderModels as fetchSettingsModelProviderModels,
  previewConsoleModelProviderModels as previewSettingsModelProviderModels,
  createConsoleModelProviderInstance as createSettingsModelProviderInstance,
  updateConsoleModelProviderInstance as updateSettingsModelProviderInstance,
  updateConsoleModelProviderMainInstance as updateSettingsModelProviderMainInstance,
  validateConsoleModelProviderInstance as validateSettingsModelProviderInstance,
  getConsoleModelProviderUsageWindows as getSettingsModelProviderUsageWindows,
  getConsoleModelProviderResetCreditCount as getSettingsModelProviderResetCreditCount,
  consumeConsoleModelProviderResetCredit as consumeSettingsModelProviderResetCredit,
  authenticateConsoleModelProviderInstance as authenticateSettingsModelProviderInstance,
  refreshConsoleModelProviderModels as refreshSettingsModelProviderModels,
  revealConsoleModelProviderSecret as revealSettingsModelProviderSecret,
  deleteConsoleModelProviderInstance as deleteSettingsModelProviderInstance,
  listConsoleModelProviderRequestLogs as fetchSettingsModelProviderRequestLogs,
  deleteConsoleModelProviderRequestLogs as deleteSettingsModelProviderRequestLogs,
  clearConsoleModelProviderRequestLogsBatch as clearSettingsModelProviderRequestLogsBatch
} from '@1flowbase/api-client';

export type SettingsModelProviderCatalogEntry =
  ConsoleModelProviderCatalogEntry;
export type SettingsModelProviderAuthOperation =
  ConsoleModelProviderAuthOperation;
export type SettingsAuthenticateModelProviderInstanceResult =
  ConsoleAuthenticateModelProviderInstanceResult;
export type SettingsModelProviderInstance = ConsoleModelProviderInstance;
export type SettingsModelProviderOptions = ConsoleModelProviderOptions;
export type SettingsModelProviderPricingTarget =
  ConsoleModelProviderPricingTarget;
export type SettingsModelProviderModelCatalog =
  ConsoleModelProviderModelCatalog;
export type SettingsRevealModelProviderSecretResult =
  RevealConsoleModelProviderSecretResult;
export type SettingsValidateModelProviderResult =
  ConsoleValidateModelProviderResult;
export type SettingsModelProviderMainInstance =
  ConsoleModelProviderMainInstance;
export type SettingsModelProviderUsageWindowsResult =
  ConsoleModelProviderUsageWindowsResult;
export type SettingsModelProviderResetCreditCountResult =
  ConsoleModelProviderResetCreditCountResult;
export type ConsumeSettingsModelProviderResetCreditInput =
  ConsumeConsoleModelProviderResetCreditInput;
export type ConsumeSettingsModelProviderResetCreditResult =
  ConsumeConsoleModelProviderResetCreditResult;
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

function resolveCatalogLocalizedValue<Value extends string | null | undefined>(
  response: Pick<
    ConsoleModelProviderCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >,
  namespace: string,
  value: Value
): Value | string {
  return value
    ? (lookupCatalogText(response, namespace, [value]) ?? value)
    : value;
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
    ...(entry.auth
      ? {
          auth: {
            ...entry.auth,
            actions: entry.auth.actions.map((action) => ({
              ...action,
              label: resolveCatalogLocalizedValue(
                response,
                entry.namespace,
                action.label
              )
            }))
          }
        }
      : {}),
    form_schema: entry.form_schema.map((field) => {
      const label = resolveCatalogLocalizedValue(
        response,
        entry.namespace,
        field.label
      );
      const description = resolveCatalogLocalizedValue(
        response,
        entry.namespace,
        field.description
      );
      const placeholder = resolveCatalogLocalizedValue(
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
                const optionDescription = resolveCatalogLocalizedValue(
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
  return listConsoleSettingsModelProviderOptions();
}

export function settingsModelProviderRequestLogsQueryKey(
  filter: ConsoleModelProviderRequestLogsFilter
) {
  return ['settings', 'model-providers', 'request-logs', filter] as const;
}
