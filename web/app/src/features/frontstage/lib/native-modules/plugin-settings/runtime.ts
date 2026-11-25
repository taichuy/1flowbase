import {
  fetchRecordCapabilities,
  requestManagedService,
  openManagedServiceStream,
  type ManagedServiceStreamOptions,
  type ManagedServiceRequest
} from '@1flowbase/api-client';
import { notifySystemMetadataChanged } from '../../../../../shared/api/system-metadata-seed';
import { useAuthStore } from '../../../../../state/auth-store';

/** Read the current session at call time; never freeze credentials into compiled TSX. */
export async function request<T>(options: ManagedServiceRequest): Promise<T> {
  const result = await requestManagedService<T>(
    options,
    useAuthStore.getState().csrfToken
  );
  const method = (options.method ?? 'GET').toUpperCase();
  if (
    method !== 'GET' &&
    options.path.startsWith('/api/console/managed-services/commercial-license/')
  ) {
    notifySystemMetadataChanged();
  }
  return result;
}

export function openStream(options: ManagedServiceStreamOptions) {
  return openManagedServiceStream(options, useAuthStore.getState().csrfToken);
}

export { RecordPermissionEditor } from '../../../../settings/components/record-permissions/RecordPermissionEditor';
export type { RecordPermissionCapabilities } from '@1flowbase/api-client';
export type { RecordPermissionEditorProps } from '../../../../settings/components/record-permissions/RecordPermissionEditor';

/** Capabilities are always evaluated by the host for the current caller. */
export function getRecordCapabilities(
  pluginCode: string,
  resourceCode: string,
  recordIds: string[],
  signal?: AbortSignal
) {
  return fetchRecordCapabilities(
    {
      plugin_code: pluginCode,
      resource_code: resourceCode,
      record_ids: recordIds
    },
    useAuthStore.getState().csrfToken,
    undefined,
    signal
  );
}
