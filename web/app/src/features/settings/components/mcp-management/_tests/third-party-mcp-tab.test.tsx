import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const upstreamApi = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog'],
  settingsMcpUpstreamConnectionsQueryKey: [
    'settings',
    'mcp-management',
    'upstream-connections'
  ],
  createSettingsMcpUpstreamConnection: vi.fn(),
  deleteSettingsMcpUpstreamConnection: vi.fn(),
  deleteSettingsMcpUpstreamConnectionCredentials: vi.fn(),
  discoverSettingsMcpUpstreamConnection: vi.fn(),
  fetchSettingsMcpUpstreamConnections: vi.fn(),
  importSettingsMcpUpstreamTools: vi.fn(),
  saveSettingsMcpUpstreamConnectionCredentials: vi.fn(),
  testSettingsMcpUpstreamConnection: vi.fn(),
  updateSettingsMcpUpstreamConnection: vi.fn()
}));

vi.mock('../../../api/mcp-management', () => upstreamApi);

import { AppProviders } from '../../../../../app/AppProviders';
import { ThirdPartyMcpTab } from '../upstream/ThirdPartyMcpTab';

const connection = {
  connection_id: '019b-connection',
  workspace_id: 'workspace-1',
  name: 'Acme MCP',
  endpoint: 'https://mcp.acme.example/mcp',
  transport: 'streamable_http' as const,
  auth_type: 'none' as const,
  status: 'enabled',
  credentials_status: 'not_required',
  custom_header_name: null,
  last_connected_at: null,
  last_discovered_at: null,
  last_error: null,
  created_at: '2026-07-14T08:00:00Z',
  updated_at: '2026-07-14T08:00:00Z'
};

function renderTab(onImported = vi.fn()) {
  render(
    <AppProviders>
      <ThirdPartyMcpTab canManage onImported={onImported} />
    </AppProviders>
  );
  return { onImported };
}

describe('ThirdPartyMcpTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    upstreamApi.fetchSettingsMcpUpstreamConnections.mockResolvedValue([
      connection
    ]);
    upstreamApi.createSettingsMcpUpstreamConnection.mockResolvedValue({
      ...connection,
      connection_id: '019b-created'
    });
    upstreamApi.updateSettingsMcpUpstreamConnection.mockResolvedValue(
      connection
    );
    upstreamApi.testSettingsMcpUpstreamConnection.mockResolvedValue({
      connection_id: connection.connection_id,
      ok: true,
      server_name: 'Acme Server',
      server_version: '1.2.3',
      protocol_version: '2025-03-26',
      tested_at: '2026-07-14T08:30:00Z',
      error: null
    });
    upstreamApi.discoverSettingsMcpUpstreamConnection.mockResolvedValue({
      connection_id: connection.connection_id,
      server_name: 'Acme Server',
      server_version: '1.2.3',
      protocol_version: '2025-03-26',
      discovered_at: '2026-07-14T08:30:00Z',
      items: [
        {
          remote_tool_name: 'search_documents',
          description: 'Search documents',
          input_schema: { type: 'object' },
          output_schema: { type: 'object' },
          source_status: 'not_imported',
          imported_tool_id: null,
          schema_hash: 'sha256:new'
        },
        {
          remote_tool_name: 'get_document',
          description: 'Get a document',
          input_schema: { type: 'object' },
          output_schema: { type: 'object' },
          source_status: 'definition_changed',
          imported_tool_id: 'get_document',
          schema_hash: 'sha256:changed'
        }
      ]
    });
    upstreamApi.importSettingsMcpUpstreamTools.mockResolvedValue([]);
  });

  test('AC-004 keeps the connection list as the tab view and creates in a modal', async () => {
    renderTab();

    expect(await screen.findByText('Acme MCP')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '新增连接' }));

    const dialog = screen.getByRole('dialog', { name: '新增第三方 MCP 连接' });
    expect(
      within(dialog).getByText('HTTPS Streamable HTTP')
    ).toBeInTheDocument();
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'Transport' })
    );
    expect(
      await screen.findByRole('option', { name: 'HTTPS Streamable HTTP' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('option', { name: /SSE|stdio/i })
    ).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('option', { name: 'HTTPS Streamable HTTP' })
    );
    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: '认证类型' })
    );
    expect(
      await screen.findByRole('option', { name: 'none' })
    ).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'bearer' })).toBeInTheDocument();
    expect(
      screen.getByRole('option', { name: 'custom_header' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('option', { name: /oauth/i })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('option', { name: 'none' }));
    fireEvent.change(within(dialog).getByLabelText('连接名称'), {
      target: { value: 'New MCP' }
    });
    fireEvent.change(within(dialog).getByLabelText('Endpoint'), {
      target: { value: 'https://new.example/mcp' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(
        upstreamApi.createSettingsMcpUpstreamConnection
      ).toHaveBeenCalledWith(
        {
          name: 'New MCP',
          endpoint: 'https://new.example/mcp',
          transport: 'streamable_http',
          auth_type: 'none',
          custom_header_name: null,
          status: 'enabled'
        },
        ''
      );
    });
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '新增第三方 MCP 连接' })
      ).not.toBeInTheDocument();
    });
  });

  test('AC-004 tests a connection in a modal with backend result fields', async () => {
    renderTab();
    expect(await screen.findByText('Acme MCP')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '测试连接 Acme MCP' }));

    const dialog = await screen.findByRole('dialog', { name: '连接测试' });
    expect(await within(dialog).findByText('Acme Server')).toBeInTheDocument();
    expect(within(dialog).getByText('2025-03-26')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '连接测试' })
      ).not.toBeInTheDocument();
    });
  });

  test('AC-007 AC-008 AC-015 discovers, searches, previews differences, and imports a selection', async () => {
    const { onImported } = renderTab();
    expect(await screen.findByText('Acme MCP')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '发现 Tool Acme MCP' }));

    const dialog = await screen.findByRole('dialog', {
      name: '发现与导入 Tool'
    });
    expect(
      await within(dialog).findByText('search_documents')
    ).toBeInTheDocument();
    expect(within(dialog).getByText('定义已变化')).toBeInTheDocument();
    fireEvent.change(within(dialog).getByPlaceholderText('搜索远端 Tool'), {
      target: { value: 'search' }
    });
    expect(within(dialog).queryByText('get_document')).not.toBeInTheDocument();
    fireEvent.click(
      within(dialog).getByRole('checkbox', { name: '选择 search_documents' })
    );
    fireEvent.click(
      within(dialog).getByRole('button', { name: '导入所选 Tool' })
    );

    await waitFor(() => {
      expect(upstreamApi.importSettingsMcpUpstreamTools).toHaveBeenCalledWith(
        connection.connection_id,
        { remote_tool_names: ['search_documents'] },
        ''
      );
    });
    expect(onImported).toHaveBeenCalledTimes(1);
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '发现与导入 Tool' })
      ).not.toBeInTheDocument();
    });
  });
});
