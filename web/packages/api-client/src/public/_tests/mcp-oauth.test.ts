import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  decideMcpOAuthAuthorization,
  fetchMcpOAuthAuthorization,
  fetchMcpOAuthConfiguration,
  verifyMcpOAuthApiKey
} from '../mcp-oauth';

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' }
  });
afterEach(() => vi.unstubAllGlobals());

describe('MCP OAuth public client transport', () => {
  test('reads direct OAuth DTOs and safely encodes identifiers', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(
        json({
          enabled: false,
          server_url: null,
          registration_method: 'dynamic_client_registration',
          scope: 'mcp:invoke'
        })
      )
      .mockResolvedValueOnce(
        json({
          client_name: 'ChatGPT',
          instance_id: 'demo',
          scope: 'mcp:invoke'
        })
      );
    vi.stubGlobal('fetch', fetch);
    expect(await fetchMcpOAuthConfiguration('demo&other=value')).toMatchObject({
      enabled: false
    });
    expect(fetch.mock.calls[0]?.[0]).toContain(
      'instance_id=demo%26other%3Dvalue'
    );
    expect(await fetchMcpOAuthAuthorization('request&id')).toMatchObject({
      client_name: 'ChatGPT'
    });
    expect(fetch.mock.calls[1]?.[0]).toContain('request_id=request%26id');
  });

  test('sends Key only in JSON body, keeps browser binding cookie, and never sends it in decision', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(
        json({
          approval_token: 'approval',
          workspace_name: 'Team',
          instance_name: 'Demo',
          scope: 'mcp:invoke'
        })
      )
      .mockResolvedValueOnce(
        json({
          redirect_uri: 'https://chatgpt.com/connector/oauth/test?code=code'
        })
      );
    vi.stubGlobal('fetch', fetch);
    await verifyMcpOAuthApiKey({
      request_id: 'req',
      api_key: 'pat_secret_canary'
    });
    const [url, request] = fetch.mock.calls[0]!;
    expect(url).not.toContain('pat_secret_canary');
    expect(request).toMatchObject({
      method: 'POST',
      credentials: 'include',
      headers: { 'content-type': 'application/json' }
    });
    expect(JSON.parse(request.body)).toEqual({
      request_id: 'req',
      api_key: 'pat_secret_canary'
    });
    expect(JSON.stringify(request.headers)).not.toContain('pat_secret_canary');
    await decideMcpOAuthAuthorization({
      request_id: 'req',
      approval_token: 'approval',
      approved: true
    });
    expect(JSON.stringify(fetch.mock.calls[1])).not.toContain(
      'pat_secret_canary'
    );
  });

  test('propagates an expired request as failure instead of an empty successful DTO', async () => {
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(
          json({ error: 'invalid_request', error_description: 'Expired' }, 400)
        )
    );
    await expect(fetchMcpOAuthAuthorization('expired')).rejects.toMatchObject({
      status: 400
    });
  });
});
