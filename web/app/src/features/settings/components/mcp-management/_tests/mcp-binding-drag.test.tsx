import type { ConsoleMcpCatalog, ConsoleMcpToolBinding } from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, waitFor } from '@testing-library/react';
import { App, Form } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import type { McpDirectoryTreeDropInfo } from '../McpInstancesTab/directory-tree';
import { useMcpDirectoryEditor } from '../McpInstancesTab/use-mcp-directory-editor';

const api = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog'],
  createSettingsMcpToolBinding: vi.fn(),
  updateSettingsMcpToolBinding: vi.fn(),
  deleteSettingsMcpGroup: vi.fn(),
  deleteSettingsMcpToolBinding: vi.fn(),
  moveSettingsMcpGroup: vi.fn(),
  upsertSettingsMcpGroup: vi.fn()
}));
vi.mock('../../../api/mcp-management', () => api);

const draggedBinding: ConsoleMcpToolBinding = {
  id: 'binding-dragged', instance_record_id: 'instance-record', tool_record_id: 'tool-dragged',
  tool_id: 'dragged_tool', group_path: '/origin', display_alias: 'Dragged alias', visible: true, sort_order: 10
};
const selectedBinding: ConsoleMcpToolBinding = {
  ...draggedBinding, id: 'binding-selected', tool_record_id: 'tool-selected',
  tool_id: 'selected_tool', group_path: '/target', display_alias: 'Selected alias', sort_order: 20
};
const catalog: ConsoleMcpCatalog = {
  instances: [{
    id: 'instance-record', workspace_id: 'workspace', instance_id: 'ops', name: 'Ops',
    description_short: null, status: 'enabled', default_entry_path: '/', webmcp_exposure: 'disabled', managed_by: null,
    created_by: 'user', updated_by: 'user', created_at: '2026-11-02T00:00:00Z', updated_at: '2026-11-02T00:00:00Z',
    llm_tool_registration: { prefix: 'ops', tools: [] }
  }],
  groups: ['origin', 'target'].map((name) => ({
    id: `group-${name}`, instance_record_id: 'instance-record', path: `/${name}`,
    display_name: name, description_short: null, enabled: true, sort_order: 0
  })),
  tools: [], bindings: [draggedBinding, selectedBinding], discovery_policies: []
};

function grant(permissions: string[]) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-drag',
    actor: { id: 'user', account: 'auditor', effective_display_role: 'readonly.audit.20261102', current_workspace_id: 'workspace' },
    me: {
      id: 'user', account: 'auditor', email: 'auditor@example.com', phone: null, nickname: 'Auditor', name: 'Auditor',
      avatar_url: null, introduction: '', effective_display_role: 'readonly.audit.20261102', permissions
    }
  });
}

const clients: QueryClient[] = [];
function renderEditor() {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  clients.push(client);
  let current: ReturnType<typeof useMcpDirectoryEditor>;
  function Harness() {
    current = useMcpDirectoryEditor(catalog);
    return <><Form form={current.groupForm} /><Form form={current.bindingForm} /></>;
  }
  render(<App><QueryClientProvider client={client}><Harness /></QueryClientProvider></App>);
  return { client, get editor() { return current; } };
}

function dropInfo(dropToGap: boolean): McpDirectoryTreeDropInfo {
  return {
    dragNode: { key: `binding:${draggedBinding.id}` },
    node: dropToGap
      ? { key: `binding:${selectedBinding.id}`, pos: '0-0-1' }
      : { key: 'group:/target', pos: '0-0' },
    dropToGap,
    dropPosition: dropToGap ? 2 : 0
  } as McpDirectoryTreeDropInfo;
}

beforeEach(() => {
  vi.clearAllMocks();
  resetAuthStore();
  api.updateSettingsMcpToolBinding.mockResolvedValue({ ...draggedBinding, group_path: '/target', sort_order: 30 });
});
afterEach(() => {
  act(() => {
    clients.splice(0).forEach((client) => client.clear());
    resetAuthStore();
  });
});

describe('MCP binding drag update identity', () => {
  test.each([false, true])('update-only grant moves the dragged binding with gap=%s while preserving another draft', async (dropToGap) => {
    grant(['mcp.tool_bindings.update']);
    const view = renderEditor();
    act(() => {
      view.editor.setEditingBinding(selectedBinding);
      view.editor.bindingForm.setFieldsValue({ instance_id: 'ops', group_path: '/target', tool_id: 'selected_tool', visible: false, sort_order: 99 });
    });
    await act(async () => {
      view.editor.handleTreeDrop(dropInfo(dropToGap));
      await waitFor(() => expect(view.client.isMutating()).toBe(0));
    });
    await waitFor(() => expect(api.updateSettingsMcpToolBinding).toHaveBeenCalledWith(
      draggedBinding.id,
      { group_path: '/target', tool_id: 'dragged_tool', display_alias: 'Dragged alias', visible: true, sort_order: 30 },
      'csrf-drag'
    ));
    expect(api.updateSettingsMcpToolBinding).toHaveBeenCalledTimes(1);
    expect(api.createSettingsMcpToolBinding).not.toHaveBeenCalled();
    expect(view.editor.editingBinding).toEqual(selectedBinding);
    expect(view.editor.bindingForm.getFieldsValue(true)).toMatchObject({ tool_id: 'selected_tool', visible: false, sort_order: 99 });
  });

  test('moving a binding without an editor draft still uses update rather than create', async () => {
    grant(['mcp.tool_bindings.update']);
    const view = renderEditor();
    await act(async () => {
      view.editor.handleTreeDrop(dropInfo(false));
      await waitFor(() => expect(view.client.isMutating()).toBe(0));
    });
    await waitFor(() => expect(api.updateSettingsMcpToolBinding).toHaveBeenCalledTimes(1));
    expect(api.updateSettingsMcpToolBinding.mock.calls[0]?.[0]).toBe(draggedBinding.id);
    expect(api.createSettingsMcpToolBinding).not.toHaveBeenCalled();
  });

  test('a create-only grant cannot move an existing binding', async () => {
    grant(['mcp.tool_bindings.create']);
    const view = renderEditor();
    await act(async () => view.editor.handleTreeDrop(dropInfo(false)));
    expect(view.editor.saveBindingMutation.isIdle).toBe(true);
    expect(api.updateSettingsMcpToolBinding).not.toHaveBeenCalled();
    expect(api.createSettingsMcpToolBinding).not.toHaveBeenCalled();
  });
});
