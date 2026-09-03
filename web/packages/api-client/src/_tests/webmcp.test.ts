import { describe, expect, test, vi } from 'vitest';

import * as transport from '../transport';
import { fetchWebMcpRegistrations, invokeWebMcpTool } from '../webmcp';

describe('WebMCP client', () => {
  test('lists registrations through the authenticated browser endpoint', async () => {
    const apiFetch = vi
      .spyOn(transport, 'apiFetch')
      .mockImplementation(async (input) => input as never);

    await expect(fetchWebMcpRegistrations()).resolves.toMatchObject({
      path: '/api/webmcp/registrations',
      signal: undefined
    });
    expect(apiFetch).toHaveBeenCalledOnce();
  });

  test('invokes a namespaced tool with CSRF and the browser cancellation signal', async () => {
    const signal = new AbortController().signal;
    const apiFetch = vi.spyOn(transport, 'apiFetch').mockResolvedValue({
      content: { items: [] },
      is_error: false
    });

    await expect(
      invokeWebMcpTool(
        'workspace/ops',
        'list',
        { path: '/' },
        'csrf-123',
        signal
      )
    ).resolves.toEqual({ items: [] });
    expect(apiFetch).toHaveBeenCalledWith({
      path: '/api/webmcp/workspace%2Fops/tools/list',
      method: 'POST',
      body: { arguments: { path: '/' } },
      csrfToken: 'csrf-123',
      signal,
      baseUrl: undefined
    });
  });

  test('rejects tool-level errors returned by WebMCP', async () => {
    vi.spyOn(transport, 'apiFetch').mockResolvedValue({
      content: { code: 'permission_denied' },
      is_error: true
    });

    await expect(
      invokeWebMcpTool('ops', 'call', {}, 'csrf-123')
    ).rejects.toThrow('permission_denied');
  });
});
