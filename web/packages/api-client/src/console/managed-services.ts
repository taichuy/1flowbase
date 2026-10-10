import { apiFetch } from '../transport';

export interface ManagedServiceRequest {
  path: string;
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  body?: unknown;
  signal?: AbortSignal;
}

/** Transport only: the compiled backend operation remains the authorization owner. */
export function requestManagedService<T>(
  request: ManagedServiceRequest,
  csrfToken: string | null
): Promise<T> {
  const base = 'https://managed-service.invalid';
  const url = new URL(request.path, base);
  if (
    !request.path.startsWith('/api/console/managed-services/') ||
    url.origin !== base ||
    !url.pathname.startsWith('/api/console/managed-services/') ||
    url.hash
  ) {
    return Promise.reject(new Error('Invalid managed service route'));
  }
  return apiFetch<T>({ ...request, csrfToken });
}
