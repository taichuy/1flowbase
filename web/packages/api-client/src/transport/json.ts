import { ApiClientError } from '../errors';

export interface ApiBaseUrlLocation {
  protocol?: string;
  hostname?: string;
  port?: string;
  origin?: string;
}

export interface ApiSuccessEnvelope<T> {
  data: T;
  meta: unknown | null;
}

export interface ApiRequestOptions {
  path: string;
  method?: string;
  body?: unknown;
  rawBody?: BodyInit;
  contentType?: string | null;
  headers?: Record<string, string>;
  credentials?: RequestCredentials;
  csrfToken?: string | null;
  baseUrl?: string;
  expectJson?: boolean;
  unwrapSuccess?: boolean;
  signal?: AbortSignal;
}

export function getDefaultApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  if (!locationLike) {
    return '';
  }

  if (locationLike.origin) {
    return locationLike.origin;
  }

  const protocol = locationLike?.protocol === 'https:' ? 'https:' : 'http:';
  const hostname = locationLike?.hostname || '127.0.0.1';
  const port = locationLike?.port;

  return port ? `${protocol}//${hostname}:${port}` : `${protocol}//${hostname}`;
}

export function unwrapApiSuccess<T>(payload: ApiSuccessEnvelope<T>): T {
  return payload.data;
}

export async function apiFetch<T>({
  path,
  method = 'GET',
  body,
  rawBody,
  contentType,
  headers: extraHeaders,
  credentials = 'include',
  csrfToken,
  baseUrl = getDefaultApiBaseUrl(),
  expectJson = true,
  unwrapSuccess = true,
  signal
}: ApiRequestOptions): Promise<T> {
  if (body !== undefined && rawBody !== undefined) {
    throw new Error(
      'apiFetch does not support body and rawBody at the same time'
    );
  }

  const headers: Record<string, string> = {};

  if (body !== undefined) {
    headers['content-type'] = 'application/json';
  }

  if (contentType !== undefined && contentType !== null) {
    headers['content-type'] = contentType;
  }

  Object.assign(headers, extraHeaders);

  if (csrfToken) {
    headers['x-csrf-token'] = csrfToken;
  }

  const response = await fetch(`${baseUrl}${path}`, {
    method,
    credentials,
    headers,
    body:
      body !== undefined
        ? JSON.stringify(body)
        : rawBody !== undefined
          ? rawBody
          : undefined,
    signal
  });

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  if (!expectJson || response.status === 204) {
    return undefined as T;
  }

  if (unwrapSuccess === false) {
    return (await response.json()) as T;
  }

  return unwrapApiSuccess<T>((await response.json()) as ApiSuccessEnvelope<T>);
}

export async function apiFetchVoid(
  options: Omit<ApiRequestOptions, 'expectJson'>
): Promise<void> {
  await apiFetch<void>({
    ...options,
    expectJson: false
  });
}
