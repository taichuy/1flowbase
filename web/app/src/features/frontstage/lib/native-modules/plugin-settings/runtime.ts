import {
  fetchRecordCapabilities,
  requestManagedService,
  openManagedServiceStream,
  type ManagedServiceStreamOptions,
  type ManagedServiceRequest
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../../state/auth-store';

/** Read the current session at call time; never freeze credentials into compiled TSX. */
export function request<T>(options: ManagedServiceRequest): Promise<T> {
  return requestManagedService<T>(options, useAuthStore.getState().csrfToken);
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
