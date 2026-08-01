import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderInstance
} from '../../../api/model-providers';

export type ModelProviderDrawerState =
  | { mode: 'create'; providerCode: string }
  | { mode: 'edit'; instanceId: string }
  | null;

export type ModelProviderInstanceModalState = {
  providerCode: string;
  displayName: string;
} | null;

export const EMPTY_MODEL_PROVIDER_INSTANCES: SettingsModelProviderInstance[] =
  [];
export const EMPTY_MODEL_PROVIDER_CATALOG: SettingsModelProviderCatalogEntry[] =
  [];
export const IDLE_MODEL_PROVIDER_MODELS_QUERY_KEY = [
  'settings',
  'model-providers',
  'models',
  'idle'
] as const;
export const MODEL_PROVIDER_MODELS_QUERY_KEY_PREFIX = [
  'settings',
  'model-providers',
  'models'
] as const;
export const MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX = [
  'settings',
  'model-providers',
  'main-instance'
] as const;

export function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : null;
}
