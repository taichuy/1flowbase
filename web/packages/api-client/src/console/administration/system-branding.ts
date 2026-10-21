import { apiFetch } from '../../transport';
import type { SystemMetadata } from '../../public/system-metadata';

export function fetchConsoleSystemSettings() {
  return apiFetch<SystemMetadata>({ path: '/api/console/settings/system' });
}

export function saveConsoleSystemSettings(
  body: SystemMetadata,
  csrfToken: string
) {
  return apiFetch<SystemMetadata>({
    path: '/api/console/settings/system',
    method: 'PUT',
    body,
    csrfToken
  });
}
