import {
  getDefaultApiBaseUrl,
  patchConsoleMeMeta,
  type ApiBaseUrlLocation,
  type ConsoleMe,
  type ConsoleUserMeta
} from '@1flowbase/api-client';

import { useAuthStore } from '../../state/auth-store';
import { hasConsoleOperationAccess } from '../../state/console-operation-access';

/** Check the current backend projection when a non-React preference writer runs. */
export function hasUserPreferenceWriteAccess(): boolean {
  const { actor, me } = useAuthStore.getState();
  return hasConsoleOperationAccess(
    actor?.effective_display_role,
    me?.permissions,
    'patch_me_meta'
  );
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
