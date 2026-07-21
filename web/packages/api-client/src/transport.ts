import { ApiClientError } from './errors';

export interface HealthResponse {
  service: string;
  status: 'ok';
  version: string;
}

export interface ApiBaseUrlLocation {
  protocol?: string;
  hostname?: string;
  port?: string;
  origin?: string;
}

interface ApiSuccessEnvelope<T> {
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
  csrfToken?: string | null;
  baseUrl?: string;
  expectJson?: boolean;
  unwrapSuccess?: boolean;
}

export interface ApiBlobResponse {
  blob: Blob;
  filename: string | null;
  contentType: string;
}

export interface ApiStreamResponse {
  body: ReadableStream<Uint8Array>;
  cancel(): void;
}

function normalizeDispositionFilename(value: string): string | null {
  const trimmed = value.trim().replace(/^"|"$/g, '');

  return trimmed.length > 0 ? trimmed : null;
}

function parseContentDispositionFilename(header: string | null): string | null {
  if (!header) {
    return null;
  }

  const filenameStarMatch = /(?:^|;)\s*filename\*=([^;]+)/iu.exec(header);
  if (filenameStarMatch?.[1]) {
    const rawValue = normalizeDispositionFilename(filenameStarMatch[1]);
    const encodedFilename = rawValue?.includes("''")
      ? rawValue.split("''").slice(1).join("''")
      : rawValue;

    if (encodedFilename) {
      try {
        return decodeURIComponent(encodedFilename);
      } catch {
        return encodedFilename;
      }
    }
  }

  const filenameMatch = /(?:^|;)\s*filename=([^;]+)/iu.exec(header);
  return filenameMatch?.[1]
    ? normalizeDispositionFilename(filenameMatch[1])
    : null;
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
  csrfToken,
  baseUrl = getDefaultApiBaseUrl(),
  expectJson = true,
  unwrapSuccess = true
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
    credentials: 'include',
    headers,
    body:
      body !== undefined
        ? JSON.stringify(body)
        : rawBody !== undefined
          ? rawBody
          : undefined
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

export async function apiFetchBlob({
  path,
  method = 'GET',
  body,
  rawBody,
  contentType,
  headers: extraHeaders,
  csrfToken,
  baseUrl = getDefaultApiBaseUrl()
}: Omit<
  ApiRequestOptions,
  'expectJson' | 'unwrapSuccess'
>): Promise<ApiBlobResponse> {
  if (body !== undefined && rawBody !== undefined) {
    throw new Error(
      'apiFetchBlob does not support body and rawBody at the same time'
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
    credentials: 'include',
    headers,
    body:
      body !== undefined
        ? JSON.stringify(body)
        : rawBody !== undefined
          ? rawBody
          : undefined
  });

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  return {
    blob: await response.blob(),
    filename: parseContentDispositionFilename(
      response.headers.get('content-disposition')
    ),
    contentType: response.headers.get('content-type') ?? ''
  };
}

export async function apiFetchStream({
  path,
  method = 'GET',
  body,
  rawBody,
  contentType,
  headers: extraHeaders,
  csrfToken,
  baseUrl = getDefaultApiBaseUrl()
}: Omit<
  ApiRequestOptions,
  'expectJson' | 'unwrapSuccess'
>): Promise<ApiStreamResponse> {
  if (body !== undefined && rawBody !== undefined) {
    throw new Error(
      'apiFetchStream does not support body and rawBody at the same time'
    );
  }
  const headers: Record<string, string> = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  if (contentType !== undefined && contentType !== null) {
    headers['content-type'] = contentType;
  }
  Object.assign(headers, extraHeaders);
  if (csrfToken) headers['x-csrf-token'] = csrfToken;

  const controller = new AbortController();
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    credentials: 'include',
    headers,
    signal: controller.signal,
    body:
      body !== undefined
        ? JSON.stringify(body)
        : rawBody !== undefined
          ? rawBody
          : undefined
  });
  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }
  if (!response.body) {
    throw new Error('Streaming response body is unavailable.');
  }
  return {
    body: response.body,
    cancel: () => controller.abort()
  };
}

export async function apiFetchVoid(
  options: Omit<ApiRequestOptions, 'expectJson'>
): Promise<void> {
  await apiFetch<void>({
    ...options,
    expectJson: false
  });
}

export async function fetchApiHealth(
  baseUrl = getDefaultApiBaseUrl()
): Promise<HealthResponse> {
  const response = await fetch(`${baseUrl}/health`, {
    credentials: 'include'
  });

  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }

  return (await response.json()) as HealthResponse;
}
