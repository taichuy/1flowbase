import { fetchConsoleMcpInstances } from '@1flowbase/api-client';
import { beforeEach, describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  fetchConsoleMcpInstances: vi.fn()
}));

import { fetchAgentFlowMcpInstanceOptions } from '../../api/mcp-instance-options';

describe('fetchAgentFlowMcpInstanceOptions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('queries enabled MCP instances directly instead of full catalog', async () => {
    vi.mocked(fetchConsoleMcpInstances).mockResolvedValue([
      {
        id: 'inst-1',
        workspace_id: 'ws-1',
        instance_id: 'ops_mcp',
        name: 'Ops MCP',
        description_short: null,
        status: 'enabled',
        default_entry_path: '/',
        webmcp_exposure: 'disabled',
        managed_by: null,
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: '2026-11-27T00:00:00Z',
        updated_at: '2026-11-27T00:00:00Z',
        llm_tool_registration: {
          prefix: 'ops_mcp',
          tools: []
        }
      },
      {
        id: 'inst-2',
        workspace_id: 'ws-1',
        instance_id: 'disabled_mcp',
        name: 'Disabled MCP',
        description_short: null,
        status: 'disabled',
        default_entry_path: '/',
        webmcp_exposure: 'disabled',
        managed_by: null,
        created_by: 'user-1',
        updated_by: 'user-1',
        created_at: '2026-11-27T00:00:00Z',
        updated_at: '2026-11-27T00:00:00Z',
        llm_tool_registration: {
          prefix: 'disabled_mcp',
          tools: []
        }
      }
    ]);

    const options = await fetchAgentFlowMcpInstanceOptions();

    expect(fetchConsoleMcpInstances).toHaveBeenCalledExactlyOnceWith({
      status: 'enabled'
    });
    expect(options).toEqual([
      {
        value: 'ops_mcp',
        label: 'Ops MCP',
        registrationPrefix: 'ops_mcp'
      }
    ]);
  });
});
