import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { authenticateEditor } from '../../../../agent-flow/_tests/fixtures/editor-auth';

import { i18nText } from '../../../../../shared/i18n/text';
import { McpInstanceTable } from '../McpInstancesTab/McpInstanceTable';

beforeEach(() =>
  authenticateEditor(['mcp.instances.update', 'mcp.directory.view'], 'member')
);

describe('McpInstanceTable', () => {
  test('delegates paging to the server and displays backend directory counts', () => {
    const instances = Array.from({ length: 41 }, (_, index) => ({
      id: `instance-${index + 1}`,
      instance_id: `instance_${index + 1}`,
      name: `Instance ${index + 1}`,
      workspace_id: 'workspace-1',
      description_short: null,
      status: 'enabled',
      default_entry_path: '/',
      webmcp_exposure: 'disabled' as const,
      managed_by: null,
      created_by: 'user-1',
      updated_by: 'user-1',
      created_at: '2026-11-11T00:00:00Z',
      updated_at: '2026-11-11T00:00:00Z',
      llm_tool_registration: { prefix: '', tools: [] },
      group_count: index + 1,
      tool_count: (index + 1) * 2
    }));
    const onEdit = vi.fn();
    const onPageChange = vi.fn();
    const props = {
      instances: instances.slice(0, 20),
      total: 41,
      page: 1,
      pageSize: 20,
      onPageChange,
      onCreate: vi.fn(),
      onEdit,
      onOpenDirectory: vi.fn(),
      onConnect: vi.fn(),
      onEditDiscoveryPolicy: vi.fn(),
      onCopy: vi.fn(),
      onExport: vi.fn(),
      onRestoreDefault: vi.fn(),
      onDelete: vi.fn(async () => undefined)
    };
    const { rerender } = render(<McpInstanceTable {...props} />);
    expect(screen.getByText('instance_1')).toBeInTheDocument();
    expect(screen.getByText('1 / 2')).toBeInTheDocument();
    expect(screen.queryByText('instance_21')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /^(页|Page) 2$/i }));
    expect(onPageChange).toHaveBeenCalledWith(2, 20);
    rerender(
      <McpInstanceTable
        {...props}
        instances={instances.slice(20, 40)}
        page={2}
      />
    );
    expect(screen.queryByText('instance_1')).not.toBeInTheDocument();
    const row = screen.getByText('instance_21').closest('tr')!;
    fireEvent.click(within(row).getByLabelText('编辑'));
    expect(onEdit).toHaveBeenCalledWith(instances[20]);
  });

  test('shows instance_id without rendering the derived LLM registration column', () => {
    const { container } = render(
      <McpInstanceTable
        canManage
        total={1}
        page={1}
        pageSize={20}
        onPageChange={vi.fn()}
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
            group_count: 0,
            tool_count: 0,
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
        total={1}
        page={1}
        pageSize={20}
        onPageChange={vi.fn()}
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
            group_count: 0,
            tool_count: 0,
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
