import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleMcpUpstreamConnection,
  createConsoleMcpTool,
  deleteConsoleMcpUpstreamConnection,
  deleteConsoleMcpUpstreamConnectionCredentials,
  discoverConsoleMcpUpstreamConnection,
  fetchConsoleMcpUpstreamConnections,
  importConsoleMcpUpstreamTools,
  saveConsoleMcpUpstreamConnectionCredentials,
  testConsoleMcpUpstreamConnection,
  updateConsoleMcpUpstreamConnection,
  updateConsoleMcpTool,
  executeConsoleMcpProxyToolDebug
} from '../console-mcp-management';

describe('console MCP upstream connection client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );

  test('AC-004 routes connection CRUD and credentials through the console API', async () => {
    const connection = {
      name: 'Acme MCP',
      endpoint: 'https://mcp.acme.example/mcp',
      transport: 'streamable_http' as const,
      auth_type: 'bearer' as const,
      custom_header_name: null,
      status: 'enabled' as const
    };

    await expect(fetchConsoleMcpUpstreamConnections()).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections'
    });
    await expect(
      createConsoleMcpUpstreamConnection(connection, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections',
      method: 'POST',
      body: connection,
      csrfToken: 'csrf-123'
    });
    await expect(
      updateConsoleMcpUpstreamConnection(
        '019b-connection',
        connection,
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/019b-connection',
      method: 'PUT',
      body: connection,
      csrfToken: 'csrf-123'
    });
    await expect(
      saveConsoleMcpUpstreamConnectionCredentials(
        '019b-connection',
        { kind: 'bearer', token: 'secret-token' },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/019b-connection/credentials',
      method: 'PUT',
      body: { kind: 'bearer', token: 'secret-token' },
      csrfToken: 'csrf-123'
    });
    await expect(
      deleteConsoleMcpUpstreamConnectionCredentials(
        '019b-connection',
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/019b-connection/credentials',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
    await expect(
      deleteConsoleMcpUpstreamConnection('019b-connection', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/019b-connection',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });

  test('AC-007 AC-008 uses backend-owned test, discovery, and import routes', async () => {
    await expect(
      testConsoleMcpUpstreamConnection('connection/slash', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/connection%2Fslash/test',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
    await expect(
      discoverConsoleMcpUpstreamConnection('connection/slash', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/connection%2Fslash/discover',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
    await expect(
      importConsoleMcpUpstreamTools(
        'connection/slash',
        { remote_tool_names: ['search', 'nested.read'] },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/upstream-connections/connection%2Fslash/imports',
      method: 'POST',
      body: { remote_tool_names: ['search', 'nested.read'] },
      csrfToken: 'csrf-123'
    });
  });

  test('AC-014 routes proxy debug by local tool id', async () => {
    await expect(
      executeConsoleMcpProxyToolDebug(
        'proxy/tool',
        { arguments: { query: 'status' } },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/tools/proxy%2Ftool/debug',
      method: 'POST',
      body: { arguments: { query: 'status' } },
      csrfToken: 'csrf-123'
    });
  });

  test('AC-002 preserves discriminated execution targets and proxy mapping keys', async () => {
    const common = {
      des_id: 'des12345',
      name: 'Proxy search',
      short_description: 'Search upstream',
      full_description: 'Search upstream documents',
      parameter_schema: { type: 'object' },
      result_schema: { type: 'object' },
      permission_code: null,
      risk_level: 'high',
      status: 'draft'
    };
    await expect(
      createConsoleMcpTool(
        {
          ...common,
          tool_id: 'proxy_search',
          execution_target: {
            kind: 'mcp_proxy',
            upstream_connection_id: '019b-connection',
            remote_tool_name: 'search_documents',
            source_schema_hash: 'sha256:source'
          },
          input_mapping: {
            mappings: [
              {
                local_path: 'request.query',
                remote_path: 'query.text',
                required: true
              }
            ]
          },
          output_mapping: {
            mappings: [
              {
                remote_path: 'document.title',
                local_path: 'result.title',
                required: true
              }
            ]
          }
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      body: {
        execution_target: {
          kind: 'mcp_proxy',
          upstream_connection_id: '019b-connection',
          remote_tool_name: 'search_documents',
          source_schema_hash: 'sha256:source'
        },
        input_mapping: {
          mappings: [
            {
              local_path: 'request.query',
              remote_path: 'query.text',
              required: true
            }
          ]
        },
        output_mapping: {
          mappings: [
            {
              remote_path: 'document.title',
              local_path: 'result.title',
              required: true
            }
          ]
        }
      }
    });

    await expect(
      updateConsoleMcpTool(
        'interface-tool',
        {
          ...common,
          execution_target: {
            kind: 'interface_wrapper',
            interface_id: 'get_runtime_profile'
          },
          input_mapping: { mappings: [] },
          output_mapping: { type: 'object' }
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      body: {
        execution_target: {
          kind: 'interface_wrapper',
          interface_id: 'get_runtime_profile'
        }
      }
    });
  });
});
