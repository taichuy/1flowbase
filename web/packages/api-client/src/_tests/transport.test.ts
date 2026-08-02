import { afterEach, describe, expect, test, vi } from 'vitest';

import { apiFetch, apiFetchBlob, getDefaultApiBaseUrl } from '../transport';

describe('apiFetch', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('apiFetch sends credentials and propagates x-csrf-token', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: { ok: true }, meta: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    const payload = await apiFetch<{ ok: boolean }>({
      path: '/api/console/session',
      method: 'GET',
      csrfToken: 'csrf-123',
      baseUrl: 'http://127.0.0.1:7800'
    });

    expect(payload).toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/session',
      expect.objectContaining({
        credentials: 'include',
        headers: expect.objectContaining({
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });

  test('apiFetch throws ApiClientError for non-2xx responses', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          code: 'not_authenticated',
          message: 'not authenticated'
        }),
        {
          status: 401,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      apiFetch({
        path: '/api/console/session',
        baseUrl: 'http://127.0.0.1:7800'
      })
    ).rejects.toEqual(
      expect.objectContaining({
        name: 'ApiClientError',
        status: 401,
        code: 'not_authenticated',
        message: 'not authenticated',
        body: {
          code: 'not_authenticated',
          message: 'not authenticated'
        }
      })
    );
  });

  test('AC-003 does not expose a non-JSON gateway response as the user-facing error message', async () => {
    const gatewayBody =
      '<html><head><title>504 Gateway Time-out</title></head><body>nginx</body></html>';
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(gatewayBody, {
        status: 504,
        headers: { 'content-type': 'text/html' }
      })
    );

    await expect(
      apiFetch({
        path: '/api/console/plugins/official-catalog',
        baseUrl: 'http://127.0.0.1:7800'
      })
    ).rejects.toEqual(
      expect.objectContaining({
        name: 'ApiClientError',
        status: 504,
        message: 'request failed: 504',
        body: gatewayBody
      })
    );
  });

  test('apiFetch supports FormData bodies without forcing JSON content-type', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: { ok: true }, meta: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );
    const formData = new FormData();
    formData.set('file', new Blob(['hello']), 'hello.1flowbasepkg');

    await apiFetch<{ ok: boolean }>({
      path: '/api/console/plugins/install-upload',
      method: 'POST',
      rawBody: formData,
      contentType: null,
      baseUrl: 'http://127.0.0.1:7800'
    });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/plugins/install-upload',
      expect.objectContaining({
        body: formData,
        headers: {}
      })
    );
  });

  test('AC-003 apiFetchBlob decodes an RFC 5987 UTF-8 download filename', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(new Blob(['{}'], { type: 'application/json' }), {
        status: 200,
        headers: {
          'content-type': 'application/json; charset=utf-8',
          'content-disposition':
            'attachment; filename="DeepSeek-V4-.1flowbase-application.json"; filename*=UTF-8\'\'DeepSeek-V4-%E6%B5%8B%E8%AF%95.1flowbase-application.json'
        }
      })
    );

    const response = await apiFetchBlob({
      path: '/api/console/applications/archive/export',
      method: 'POST',
      body: { application_ids: ['application-1'] },
      baseUrl: 'http://127.0.0.1:7800'
    });

    expect(response.filename).toBe(
      'DeepSeek-V4-测试.1flowbase-application.json'
    );
    expect(response.contentType).toBe('application/json; charset=utf-8');
  });
});

describe('getDefaultApiBaseUrl', () => {
  test('defaults browser callers to the current frontend origin', () => {
    expect(
      getDefaultApiBaseUrl({
        protocol: 'http:',
        hostname: '127.0.0.1',
        port: '3100',
        origin: 'http://127.0.0.1:3100'
      })
    ).toBe('http://127.0.0.1:3100');
  });

  test('falls back to a relative base when no browser location is available', () => {
    expect(getDefaultApiBaseUrl(undefined)).toBe('');
  });
});
