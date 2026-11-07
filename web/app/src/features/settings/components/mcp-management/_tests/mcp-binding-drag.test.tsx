import type {
  ConsoleMcpCatalog,
  ConsoleMcpToolBinding
} from '@1flowbase/api-client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, waitFor } from '@testing-library/react';
import { App, Form } from 'antd';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import type { McpDirectoryTreeDropInfo } from '../McpInstancesTab/directory-tree';
import { useMcpDirectoryEditor } from '../McpInstancesTab/use-mcp-directory-editor';
import { installDirectorySnapshotApi } from './mcp-management-panel/fixtures/directory-snapshot';

const api = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog'],
  fetchSettingsMcpInstanceDirectory: vi.fn(),
  replaceSettingsMcpInstanceDirectory: vi.fn(),
  createSettingsMcpToolBinding: vi.fn(),
  updateSettingsMcpToolBinding: vi.fn(),
  deleteSettingsMcpGroup: vi.fn(),
  deleteSettingsMcpToolBinding: vi.fn(),
  moveSettingsMcpGroup: vi.fn(),
  upsertSettingsMcpGroup: vi.fn()
}));
vi.mock('../../../api/mcp-management', () => api);

const draggedBinding: ConsoleMcpToolBinding = {
  id: 'binding-dragged',
  instance_record_id: 'instance-record',
  tool_record_id: 'tool-dragged',
  tool_id: 'dragged_tool',
  group_path: '/origin',
  display_alias: 'Dragged alias',
  visible: true,
  sort_order: 10
};
const selectedBinding: ConsoleMcpToolBinding = {
  ...draggedBinding,
  id: 'binding-selected',
  tool_record_id: 'tool-selected',
  tool_id: 'selected_tool',
  group_path: '/target',
  display_alias: 'Selected alias',
  sort_order: 20
};
const catalog: ConsoleMcpCatalog = {
  instances: [
    {
      id: 'instance-record',
      workspace_id: 'workspace',
      instance_id: 'ops',
      name: 'Ops',
      description_short: null,
      status: 'enabled',
      default_entry_path: '/',
      webmcp_exposure: 'disabled',
      managed_by: null,
      created_by: 'user',
      updated_by: 'user',
      created_at: '2026-11-02T00:00:00Z',
      updated_at: '2026-11-02T00:00:00Z',
      llm_tool_registration: { prefix: 'ops', tools: [] }
    }
  ],
  groups: ['origin', 'target'].map((name) => ({
    id: `group-${name}`,
    instance_record_id: 'instance-record',
    path: `/${name}`,
    display_name: name,
    description_short: null,
    enabled: true,
    sort_order: 0
  })),
  tools: [],
  bindings: [draggedBinding, selectedBinding],
  discovery_policies: []
};

function grant(permissions: string[]) {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-drag',
    actor: {
      id: 'user',
      account: 'auditor',
      effective_display_role: 'readonly.audit.20261102',
      current_workspace_id: 'workspace'
    },
    me: {
      id: 'user',
      account: 'auditor',
      email: 'auditor@example.com',
      phone: null,
      nickname: 'Auditor',
      name: 'Auditor',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'readonly.audit.20261102',
      permissions
    }
  });
}

const clients: QueryClient[] = [];
function renderEditor() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } }
  });
  clients.push(client);
  let current: ReturnType<typeof useMcpDirectoryEditor>;
  function Harness() {
    current = useMcpDirectoryEditor(catalog);
    return (
      <>
        <Form form={current.groupForm} />
        <Form form={current.bindingForm} />
      </>
    );
  }
  render(
    <App>
      <QueryClientProvider client={client}>
        <Harness />
      </QueryClientProvider>
    </App>
  );
  return {
    client,
    get editor() {
      return current;
    }
  };
}

async function dropAndSettle(
  view: ReturnType<typeof renderEditor>,
  info: McpDirectoryTreeDropInfo
) {
  await act(async () => {
    view.editor.handleTreeDrop(info);

    if (view.client.isMutating() === 0) return;
    await new Promise<void>((resolve) => {
      const unsubscribe = view.client.getMutationCache().subscribe(() => {
        if (view.client.isMutating() !== 0) return;
        unsubscribe();
        resolve();
      });
    });
  });
  expect(view.client.isMutating()).toBe(0);
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
  installDirectorySnapshotApi(api, catalog);
});
afterEach(() => {
  act(() => {
    clients.splice(0).forEach((client) => client.clear());
    resetAuthStore();
  });
});

async function openEditor() {
  const view = renderEditor();
  await act(async () => {
    await view.editor.openDirectory(catalog.instances[0]!);
  });
  return view;
}

function expectNoImmediateWrites() {
  expect(api.updateSettingsMcpToolBinding).not.toHaveBeenCalled();
  expect(api.createSettingsMcpToolBinding).not.toHaveBeenCalled();
  expect(api.moveSettingsMcpGroup).not.toHaveBeenCalled();
  expect(api.replaceSettingsMcpInstanceDirectory).not.toHaveBeenCalled();
}

describe('MCP directory draft drag', () => {
  test.each([false, true])(
    'stages the dragged binding with gap=%s and preserves another node form',
    async (dropToGap) => {
      grant(['mcp.directory.view', 'mcp.directory.replace']);
      const view = await openEditor();
      act(() =>
        view.editor.selectDirectoryNode(`binding:${selectedBinding.id}`)
      );
      act(() => view.editor.editCurrentNode());
      act(() => {
        view.editor.bindingForm.setFieldsValue({
          instance_id: 'ops',
          group_path: '/target',
          tool_id: 'selected_tool',
          visible: false,
          sort_order: 99
        });
      });
      await dropAndSettle(view, dropInfo(dropToGap));
      expectNoImmediateWrites();
      expect(view.editor.bindingForm.getFieldsValue(true)).toMatchObject({
        tool_id: 'selected_tool',
        visible: false,
        sort_order: 99
      });
      await act(async () => {
        await view.editor.saveDirectoryMutation.mutateAsync();
      });
      await waitFor(() =>
        expect(api.replaceSettingsMcpInstanceDirectory).toHaveBeenCalledTimes(1)
      );
      const [instanceId, body, csrfToken] =
        api.replaceSettingsMcpInstanceDirectory.mock.calls[0]!;
      expect(instanceId).toBe('ops');
      expect(csrfToken).toBe('csrf-drag');
      expect(body.expected_snapshot.bindings).toEqual([
        draggedBinding,
        selectedBinding
      ]);
      expect(body.bindings).toEqual([
        {
          id: draggedBinding.id,
          tool_id: 'dragged_tool',
          group_path: '/target',
          display_alias: 'Dragged alias',
          visible: true,
          sort_order: 30
        },
        {
          id: selectedBinding.id,
          tool_id: 'selected_tool',
          group_path: '/target',
          display_alias: 'Selected alias',
          visible: true,
          sort_order: 20
        }
      ]);
      expect(view.editor.bindingForm.getFieldsValue(true)).toMatchObject({
        tool_id: 'selected_tool',
        visible: false,
        sort_order: 99
      });
      expect(api.updateSettingsMcpToolBinding).not.toHaveBeenCalled();
      expect(api.createSettingsMcpToolBinding).not.toHaveBeenCalled();
    }
  );

  test('view-only and old item-update grants cannot move an existing binding', async () => {
    grant(['mcp.directory.view', 'mcp.tool_bindings.update']);
    const view = await openEditor();
    await dropAndSettle(view, dropInfo(false));
    expectNoImmediateWrites();
    expect(view.editor.directoryHasChanges).toBe(false);
  });

  test('group moves and descendant binding paths are submitted together without immediate writes', async () => {
    grant(['mcp.directory.view', 'mcp.directory.replace']);
    const view = await openEditor();
    await dropAndSettle(view, {
      dragNode: { key: 'group:/origin' },
      node: { key: 'group:/target', pos: '0-0' },
      dropToGap: false,
      dropPosition: 0
    } as McpDirectoryTreeDropInfo);
    expectNoImmediateWrites();
    await act(async () => {
      await view.editor.saveDirectoryMutation.mutateAsync();
    });
    const body = api.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1];
    expect(body.groups).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'group-origin', path: '/target/origin' }),
        expect.objectContaining({ id: 'group-target', path: '/target' })
      ])
    );
    expect(body.bindings).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: draggedBinding.id,
          group_path: '/target/origin'
        }),
        expect.objectContaining({
          id: selectedBinding.id,
          group_path: '/target'
        })
      ])
    );
    expect(api.moveSettingsMcpGroup).not.toHaveBeenCalled();
  });
});
