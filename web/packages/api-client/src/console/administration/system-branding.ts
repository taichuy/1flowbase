import { apiFetch } from '../../transport';
import type { SystemBranding } from '../../public/system-metadata';

export function fetchConsoleSystemSettings() {
  return apiFetch<SystemBranding>({ path: '/api/console/settings/system' });
}

export function saveConsoleSystemSettings(
  body: SystemBranding,
  csrfToken: string
) {
  return apiFetch<SystemBranding>({
    path: '/api/console/settings/system',
    method: 'PUT',
    body,
    csrfToken
  });
}
