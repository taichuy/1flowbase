import { render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { McpInstanceTable } from '../McpInstancesTab/McpInstanceTable';

describe('McpInstanceTable', () => {
  test('shows the backend-owned LLM tool registration prefix', () => {
    render(
      <McpInstanceTable
        canManage
        groupCounts={new Map()}
        toolCounts={new Map()}
        instances={[
          {
            id: 'instance-1',
            workspace_id: 'workspace-1',
            instance_id: 'workspace_ops',
            name: 'Workspace Ops',
            description_short: null,
            status: 'enabled',
            default_entry_path: '/',
            created_by: 'user-1',
            updated_by: 'user-1',
            created_at: '2026-08-08T00:00:00Z',
            updated_at: '2026-08-08T00:00:00Z',
            llm_tool_registration: {
              prefix: 'workspace_ops',
              tools: [
                { operation: 'list', name: 'workspace_ops_mcp_list' },
                { operation: 'get', name: 'workspace_ops_mcp_get' },
                { operation: 'result', name: 'workspace_ops_mcp_result' },
                { operation: 'call', name: 'workspace_ops_mcp_call' }
              ]
            }
          }
        ]}
        onConnect={vi.fn()}
        onCopy={vi.fn()}
        onCreate={vi.fn()}
        onDelete={vi.fn(async () => undefined)}
        onEdit={vi.fn()}
        onEditDiscoveryPolicy={vi.fn()}
        onExport={vi.fn()}
        onOpenDirectory={vi.fn()}
      />
    );

    expect(
      screen
        .getAllByText('workspace_ops')
        .find((element) => element.tagName === 'CODE')
    ).toBeDefined();
  });
});
