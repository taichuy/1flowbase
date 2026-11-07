import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  fetchConsoleMcpInstanceDirectory,
  replaceConsoleMcpInstanceDirectory,
  type ConsoleMcpInstanceDirectory,
  type ReplaceConsoleMcpInstanceDirectoryBody
} from '../mcp-management';

afterEach(() => vi.unstubAllGlobals());

const snapshot: ConsoleMcpInstanceDirectory = {
  instance_record_id: 'instance-record',
  groups: [
    {
      id: 'group-record',
      instance_record_id: 'instance-record',
      path: '/ops',
      display_name: 'Operations',
      description_short: null,
      enabled: true,
      sort_order: 3
    }
  ],
  bindings: [
    {
      id: 'binding-record',
      instance_record_id: 'instance-record',
      tool_record_id: 'tool-record',
      tool_id: 'read',
      group_path: '/ops',
      display_alias: null,
      visible: true,
      sort_order: 4
    }
  ]
};

describe('MCP instance directory wire contract', () => {
  test('GET unwraps the canonical snapshot and encodes the instance selector', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ data: snapshot, meta: null }), {
          status: 200
        })
      );
    vi.stubGlobal('fetch', fetch);
    await expect(
      fetchConsoleMcpInstanceDirectory('instance/name', 'https://example.test')
    ).resolves.toEqual(snapshot);
    expect(fetch.mock.calls[0][0]).toBe(
      'https://example.test/api/console/mcp/instances/instance%2Fname/directory'
    );
    expect(fetch.mock.calls[0][1].method ?? 'GET').toBe('GET');
  });

  test('PUT sends one expected snapshot and desired directory with CSRF and returns committed IDs', async () => {
    const body: ReplaceConsoleMcpInstanceDirectoryBody = {
      expected_snapshot: snapshot,
      groups: [
        {
          id: 'group-record',
          path: '/moved',
          display_name: 'Moved',
          description_short: null,
          enabled: false,
          sort_order: -1
        }
      ],
      bindings: [
        {
          tool_id: 'read',
          group_path: '/',
          display_alias: 'Root read',
          visible: false,
          sort_order: 7
        }
      ]
    };
    const committed: ConsoleMcpInstanceDirectory = {
      instance_record_id: snapshot.instance_record_id,
      groups: [{ ...snapshot.groups[0], ...body.groups[0] }],
      bindings: [
        {
          ...snapshot.bindings[0],
          ...body.bindings[0],
          id: 'generated-binding'
        }
      ]
    };
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ data: committed, meta: null }), {
          status: 200
        })
      );
    vi.stubGlobal('fetch', fetch);
    await expect(
      replaceConsoleMcpInstanceDirectory(
        'instance/name',
        body,
        'csrf-value',
        'https://example.test'
      )
    ).resolves.toEqual(committed);
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0][0]).toBe(
      'https://example.test/api/console/mcp/instances/instance%2Fname/directory'
    );
    expect(fetch.mock.calls[0][1].method).toBe('PUT');
    expect(fetch.mock.calls[0][1].headers['x-csrf-token']).toBe('csrf-value');
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(body);
    expect(body.expected_snapshot).toEqual(snapshot);
  });

  test('conflicts keep the backend error and never retry or overwrite the supplied snapshot', async () => {
    const error = {
      status: 409,
      code: 'mcp_directory_snapshot',
      message: 'conflict: mcp_directory_snapshot'
    };
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify(error), {
          status: 409,
          headers: { 'content-type': 'application/json' }
        })
      );
    vi.stubGlobal('fetch', fetch);
    const body: ReplaceConsoleMcpInstanceDirectoryBody = {
      expected_snapshot: snapshot,
      groups: [],
      bindings: []
    };
    await expect(
      replaceConsoleMcpInstanceDirectory('instance', body, 'csrf')
    ).rejects.toMatchObject({ status: 409, code: error.code, body: error });
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(body.expected_snapshot).toEqual(snapshot);
  });
});
