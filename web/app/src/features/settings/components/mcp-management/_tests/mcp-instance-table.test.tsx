import { render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { i18nText } from '../../../../../shared/i18n/text';
import { McpInstanceTable } from '../McpInstancesTab/McpInstanceTable';

describe('McpInstanceTable', () => {
  test('shows instance_id without rendering the derived LLM registration column', () => {
    const { container } = render(
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
            webmcp_exposure: 'disabled',
            managed_by: null,
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
        onRestoreDefault={vi.fn()}
      />
    );

    expect(screen.getByText('workspace_ops')).toBeInTheDocument();
    expect(container.querySelector('code')).toBeNull();
  });

  test('shows bundle provenance without disabling instance editors', () => {
    render(
      <McpInstanceTable
        canManage
        groupCounts={new Map()}
        toolCounts={new Map()}
        instances={[
          {
            id: 'managed-instance',
            workspace_id: 'workspace-1',
            instance_id: 'frontstage_browser',
            name: 'Frontstage Browser',
            description_short: null,
            status: 'enabled',
            default_entry_path: '/frontstage',
            webmcp_exposure: 'authenticated_session',
            managed_by: {
              organization: '1flowbase',
              bundle_id: 'frontstage_assistant',
              bundle_version: '1.0.2'
            },
            created_by: 'user-1',
            updated_by: 'user-1',
            created_at: '2026-08-17T00:00:00Z',
            updated_at: '2026-08-17T00:00:00Z',
            llm_tool_registration: { prefix: '', tools: [] }
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
        onRestoreDefault={vi.fn()}
      />
    );

    expect(
      screen.getByText('1flowbase/frontstage_assistant@1.0.2')
    ).toBeInTheDocument();
    expect(screen.getByLabelText('编辑')).toBeEnabled();
    expect(
      screen.getByLabelText(i18nText('settings', 'auto.directory_editor'))
    ).toBeEnabled();
  });
});
