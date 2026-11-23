import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import {
  buildBootstrapRequestPath,
  clearCachedBootstrapSnapshots,
  fetchWithConditionalBootstrapCache,
  isBootstrapEndpointMarkedUnsupported,
  parseEarlyFrontstageBootstrapQuery,
  resolveEarlyBootstrapPath
} from '../../../../app/bootstrap/early-console-startup-handoff';

describe('early-console-startup-flight', () => {
  beforeEach(() => {
    clearCachedBootstrapSnapshots();
  });

  afterEach(() => {
    clearCachedBootstrapSnapshots();
    vi.unstubAllGlobals();
  });

  test('parses Frontstage candidate pathnames and skips non-Frontstage console routes', () => {
    expect(parseEarlyFrontstageBootstrapQuery('/')).toEqual({ code_limit: 12 });
    expect(parseEarlyFrontstageBootstrapQuery('/sign-in')).toEqual({
      code_limit: 12
    });
    expect(parseEarlyFrontstageBootstrapQuery('/reports')).toEqual({
      slug: 'reports',
      code_limit: 12
    });
    expect(
      parseEarlyFrontstageBootstrapQuery('/reports/pages/page-1/tabs/tab-2')
    ).toEqual({
      slug: 'reports',
      page_id: 'page-1',
      tab_reference: 'tab-2',
      code_limit: 12
    });
    expect(parseEarlyFrontstageBootstrapQuery('/settings/general')).toBeNull();
    expect(parseEarlyFrontstageBootstrapQuery('/applications')).toBeNull();
    expect(parseEarlyFrontstageBootstrapQuery('/me')).toBeNull();

    for (const candidate of [
      '/',
      '/sign-in',
      '/reports',
      '/reports/pages/page-1',
      '/reports/tabs/tab-2',
      '/reports/pages/page-1/tabs/tab-2'
    ]) {
      const parsed = parseEarlyFrontstageBootstrapQuery(candidate)!;
      expect(resolveEarlyBootstrapPath(candidate)).toBe(
        buildBootstrapRequestPath(parsed)
      );
    }
    expect(resolveEarlyBootstrapPath('/settings/general')).toBeNull();
  });

  test('revalidates cached bootstrap snapshot via If-None-Match across canonical slug/page_id normalization and reuses payload on 304', async () => {
    const samplePayload = {
      pages: [{ id: 'p1', kind: 'page', placement: 'topbar', slug: 'reports', children: [] }],
      resolved_page_id: 'p1',
      resolved_tab_id: 't1',
      tabs: [],
      page_detail: null,
      block_roots: [],
      block_codes: []
    };

    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        headers: new Headers({ etag: '"etag-v1"' }),
        json: async () => ({ data: samplePayload })
      })
      .mockResolvedValueOnce({
        ok: false,
        status: 304,
        headers: new Headers({ etag: '"etag-v1"' }),
        json: async () => {
          throw new Error('304 has no JSON body');
        }
      });
    vi.stubGlobal('fetch', fetchMock);

    const first = await fetchWithConditionalBootstrapCache({
      actorId: 'u1',
      workspaceId: 'w1',
      role: 'root',
      locale: 'zh_Hans',
      query: { slug: 'reports', code_limit: 12 },
      baseUrl: 'https://ai.example.test'
    });
    expect(first).toEqual(samplePayload);
    expect(fetchMock.mock.calls[0]?.[1]?.headers).toEqual({});

    const second = await fetchWithConditionalBootstrapCache({
      actorId: 'u1',
      workspaceId: 'w1',
      role: 'root',
      locale: 'zh_Hans',
      query: { slug: 'reports', page_id: 'p1', code_limit: 12 },
      baseUrl: 'https://ai.example.test'
    });
    expect(second).toEqual(samplePayload);
    expect(fetchMock.mock.calls[1]?.[1]?.headers).toEqual({
      'if-none-match': '"etag-v1"'
    });
  });

  test('memoizes 404 unsupported bootstrap endpoint per origin and role', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      headers: new Headers()
    });
    vi.stubGlobal('fetch', fetchMock);

    await expect(
      fetchWithConditionalBootstrapCache({
        actorId: 'u1',
        workspaceId: 'w1',
        role: 'root',
        locale: 'zh_Hans',
        query: { code_limit: 12 },
        baseUrl: 'https://legacy.example.test'
      })
    ).rejects.toThrow();

    expect(
      isBootstrapEndpointMarkedUnsupported('https://legacy.example.test', 'root')
    ).toBe(true);

    await expect(
      fetchWithConditionalBootstrapCache({
        actorId: 'u1',
        workspaceId: 'w1',
        role: 'root',
        locale: 'zh_Hans',
        query: { code_limit: 12 },
        baseUrl: 'https://legacy.example.test'
      })
    ).rejects.toThrow('frontstage_bootstrap_endpoint_unsupported');
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
