import { fireEvent, screen, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { useAuthStore } from '../../../../../state/auth-store';
import {
  mcpManagementApi,
  renderPanelWithMountedTool,
  setupMcpManagementPanelFixture
} from './mcp-management-panel/fixtures/panel';

function grant(permissions: string[]) {
  const { actor, me } = useAuthStore.getState();
  useAuthStore.setState({
    actor: { ...actor!, effective_display_role: 'readonly.audit.20261102' },
    me: { ...me!, effective_display_role: 'readonly.audit.20261102', permissions }
  });
}

describe('MCP exact operation controls', () => {
  setupMcpManagementPanelFixture();

  test('readonly retains instance, directory and client configuration inspection', async () => {
    grant(['mcp.instances.view', 'mcp.client_credential.reveal']);
    renderPanelWithMountedTool();
    const panel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    expect(within(panel).getByText('Ops MCP')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: '新增' })).toBeDisabled();
    fireEvent.click(within(panel).getByRole('button', { name: '查看' }));
    const editor = await screen.findByRole('dialog', { name: '查看' });
    expect(editor.querySelector('button.ant-btn-primary')).toBeDisabled();
    expect(mcpManagementApi.updateSettingsMcpInstance).not.toHaveBeenCalled();
  });

  test('one instance-create grant enables create without enabling tool create', () => {
    grant(['mcp.instances.create']);
    renderPanelWithMountedTool();
    expect(within(screen.getByRole('tabpanel', { name: 'MCP 实例' })).getByRole('button', { name: '新增' })).toBeEnabled();
    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    expect(within(screen.getByRole('tabpanel', { name: 'Tool 配置' })).getByRole('button', { name: '新增' })).toBeDisabled();
  });

  test('root retains instance and tool creation', () => {
    renderPanelWithMountedTool();
    expect(within(screen.getByRole('tabpanel', { name: 'MCP 实例' })).getByRole('button', { name: '新增' })).toBeEnabled();
    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    expect(within(screen.getByRole('tabpanel', { name: 'Tool 配置' })).getByRole('button', { name: '新增' })).toBeEnabled();
  });
});
