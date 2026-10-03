import { afterEach, expect, test, vi } from 'vitest';
import { revalidateConsoleFrontstageBlockNodeCode } from '../requests';

afterEach(() => vi.unstubAllGlobals());

test('conditional source GET handles 304 without parsing JSON and forwards cancellation', async () => {
  const fetch = vi.fn(
    async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(null, { status: 304 })
  );
  vi.stubGlobal('fetch', fetch);
  const controller = new AbortController();
  await expect(
    revalidateConsoleFrontstageBlockNodeCode('page/1', 'block/1', {
      baseUrl: 'http://localhost',
      source_sha256: 'abc',
      signal: controller.signal
    })
  ).resolves.toEqual({ status: 'not_modified' });
  expect(fetch).toHaveBeenCalledWith(
    'http://localhost/api/console/frontstage/pages/page%2F1/blocks/block%2F1/code',
    {
      method: 'GET',
      credentials: 'include',
      cache: 'no-store',
      headers: { 'if-none-match': '"abc"' },
      signal: controller.signal
    }
  );
});

test('changed source returns the canonical DTO and explicit refresh omits the condition', async () => {
  const value = {
    block_id: 'b',
    page_id: 'p',
    source_code: 'new source',
    source_sha256: 'def'
  };
  const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
    Response.json({ data: value, meta: null })
  );
  vi.stubGlobal('fetch', fetch);
  await expect(
    revalidateConsoleFrontstageBlockNodeCode('p', 'b')
  ).resolves.toEqual({ status: 'modified', value });
  expect(fetch.mock.calls[0]?.[1]).toMatchObject({ headers: undefined });
});

test('a matching revision cannot hide an authorization failure', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      Response.json(
        { error: { code: 'forbidden', message: 'Forbidden' } },
        { status: 403 }
      )
    )
  );
  await expect(
    revalidateConsoleFrontstageBlockNodeCode('p', 'b', { source_sha256: 'abc' })
  ).rejects.toMatchObject({ status: 403 });
});
