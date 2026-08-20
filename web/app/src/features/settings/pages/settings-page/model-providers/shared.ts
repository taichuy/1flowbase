import type { UploadFile } from 'antd/es/upload/interface';
import { ApiClientError } from '@1flowbase/api-client';

import type {
  SettingsModelProviderCatalogEntry,
  SettingsModelProviderInstance
} from '../../../api/model-providers';
import type { SettingsPluginFamilyEntry } from '../../../api/plugins';
import { i18nText } from '../../../../../shared/i18n/text';

export type ModelProviderDrawerState =
  | { mode: 'create'; providerCode: string; draftInstanceId?: string }
  | { mode: 'edit'; instanceId: string }
  | null;

export type ModelProviderInstanceModalState = {
  providerCode: string;
  displayName: string;
} | null;

export type OfficialInstallState = {
  pluginId: string | null;
  taskId: string | null;
  status: 'idle' | 'installing' | 'success' | 'failed';
};

export type UploadResultSummary = {
  displayName: string;
  version: string;
  trustLabel: string;
  availabilityLabel: string;
} | null;

export type RecentVersionSwitchNotice = {
  providerCode: string;
  targetVersion: string | null;
  migratedInstanceCount: number | null;
} | null;

export const EMPTY_MODEL_PROVIDER_INSTANCES: SettingsModelProviderInstance[] =
  [];
export const EMPTY_MODEL_PROVIDER_CATALOG: SettingsModelProviderCatalogEntry[] =
  [];
export const EMPTY_PLUGIN_FAMILIES: SettingsPluginFamilyEntry[] = [];
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

export function getPluginUploadErrorMessage(error: unknown) {
  if (error instanceof ApiClientError) {
    switch (error.code) {
      case 'plugin_runtime_target_mismatch':
        return i18nText('settings', 'auto.plugin_upload_platform_incompatible');
      case 'provider_package':
        return i18nText('settings', 'auto.plugin_upload_package_invalid');
      default:
        if (error.status === 413) {
          return i18nText('settings', 'auto.plugin_upload_too_large');
        }
        if (error.status >= 500) {
          return i18nText('settings', 'auto.plugin_upload_server_unavailable');
        }
        return i18nText('settings', 'auto.plugin_upload_failed');
    }
  }

  if (
    error instanceof Error &&
    /failed to fetch|networkerror|load failed/iu.test(error.message)
  ) {
    return i18nText('settings', 'auto.plugin_upload_network_unavailable');
  }

  return i18nText('settings', 'auto.plugin_upload_failed');
}

export function isTaskTerminal(status: string | null | undefined) {
  return (
    status === 'success' ||
    status === 'succeeded' ||
    status === 'failed' ||
    status === 'canceled' ||
    status === 'timed_out'
  );
}

export function isTaskSucceeded(status: string | null | undefined) {
  return status === 'success' || status === 'succeeded';
}

export function parseTaskDetailString(
  detail: Record<string, unknown>,
  key: string
) {
  const value = detail[key];
  return typeof value === 'string' ? value : null;
}

export function parseTaskDetailNumber(
  detail: Record<string, unknown>,
  key: string
) {
  const value = detail[key];
  return typeof value === 'number' ? value : null;
}

export function formatTrustLabel(trustLevel: string) {
  switch (trustLevel) {
    case 'verified_official':
      return i18nText('settings', 'auto.officially_issued');
    case 'checksum_only':
      return i18nText('settings', 'auto.checksum_only');
    default:
      return i18nText('settings', 'auto.not_verified');
  }
}

export function resetUploadState(
  setUploadFileList: (value: UploadFile[]) => void,
  setUploadValidationMessage: (value: string | null) => void,
  setUploadResultSummary: (value: UploadResultSummary) => void
) {
  setUploadFileList([]);
  setUploadValidationMessage(null);
  setUploadResultSummary(null);
}
