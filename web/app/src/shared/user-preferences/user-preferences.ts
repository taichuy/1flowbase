import {
  getDefaultApiBaseUrl,
  patchConsoleMeMeta,
  type ApiBaseUrlLocation,
  type ConsoleMe,
  type ConsoleUserMeta
} from '@1flowbase/api-client';

import { ensureConsoleOperationAccess, hasCachedConsoleOperationAccess } from '../../state/console-operation-access';

/** Check the current backend projection when a non-React preference writer runs. */
export function hasUserPreferenceWriteAccess(): boolean {
  return hasCachedConsoleOperationAccess('patch_me_meta');
}

/** Keep the first user edit when the permission cache is cold. */
export function withUserPreferenceWriteAccess(write: () => void): void {
  if (hasUserPreferenceWriteAccess()) {
    write();
    return;
  }
  void ensureConsoleOperationAccess(['patch_me_meta']).then(({ patch_me_meta }) => {
    if (patch_me_meta && hasUserPreferenceWriteAccess()) write();
  });
}

export type UserPreferencePatch = ConsoleUserMeta;

export function getUserPreferencesApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined =
    typeof window !== 'undefined' ? window.location : undefined
): string {
  return import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike);
}

export function patchUserPreferences(
  patch: UserPreferencePatch,
  csrfToken: string,
  baseUrl = getUserPreferencesApiBaseUrl()
): Promise<ConsoleMe> {
  return patchConsoleMeMeta(patch, csrfToken, baseUrl);
}
