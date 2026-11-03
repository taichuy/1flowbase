import type { ReactNode } from 'react';
import { act, cleanup, render, renderHook, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const saveOrder = vi.hoisted(() => vi.fn());
vi.mock('../../../api/identity-access/permissions', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../api/identity-access/permissions')>()),
  replaceSettingsConsolePolicyOrder: saveOrder
}));

import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { type SettingsConsolePolicyCatalog, settingsConsolePolicyCatalogQueryKey } from '../../../api/identity-access/permissions';
import { PolicyDragHandle, SortablePolicyRow, SortablePolicyTable } from '../SortablePolicyTable';
import { useConsoleSettingsOrder } from '../useConsoleSettingsOrder';

const catalog: SettingsConsolePolicyCatalog = {
  schema_version: '2026-07-14', locale: 'en_US', settings_order_revision: 4,
  group_strategy_options: [], resources: [],
  groups: ['first', 'second'].map((group_id) => ({ kind: 'settings_feature', group_id, settings_feature_id: null, label: group_id, description: '', operations: [] }))
};
let client: QueryClient;
beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  saveOrder.mockReset().mockResolvedValue(catalog);
});
afterEach(() => { cleanup(); client.clear(); resetAuthStore(); });

function wrapper({ children }: { children: ReactNode }) {
  return <App><QueryClientProvider client={client}>{children}</QueryClientProvider></App>;
}

function setOperations(role: string, permissions: string[]) {
  useAuthStore.setState({ csrfToken: 'csrf',
    actor: { id: 'actor', account: 'actor', effective_display_role: role, current_workspace_id: 'workspace-1' },
    me: { id: 'actor', account: 'actor', email: '', phone: null, nickname: '', name: '', avatar_url: null, introduction: '', effective_display_role: role, permissions }
  });
}

test.each([
  { role: 'member', permissions: ['roles.console_policy_catalog.view'], allowed: false },
  { role: 'member', permissions: ['roles.update', 'roles.console_policy.replace'], allowed: false },
  { role: 'member', permissions: ['roles.console_settings_order.replace'], allowed: true },
  { role: 'root', permissions: [], allowed: true }
])('only the settings-order operation enables $role sorting with $permissions', async ({ role, permissions, allowed }) => {
  setOperations(role, permissions);
  client.setQueryData(settingsConsolePolicyCatalogQueryKey('en_US'), catalog);
  const { result } = renderHook(() => {
    const { message } = App.useApp();
    return useConsoleSettingsOrder({ csrfToken: 'csrf', locale: 'en_US', catalog, queryClient: client, messageApi: message });
  }, { wrapper });
  render(<SortablePolicyTable itemIds={['first', 'second']} onReorder={result.current.reorder}>
    <table><tbody>{['first', 'second'].map((id) => <SortablePolicyRow key={id} data-row-key={id}><td>{id}<PolicyDragHandle /></td></SortablePolicyRow>)}</tbody></table>
  </SortablePolicyTable>, { wrapper });
  expect(screen.getAllByRole('button').every((button) => !button.hasAttribute('disabled'))).toBe(allowed);
  act(() => result.current.reorder(0, 1));
  if (allowed) {
    await waitFor(() => expect(saveOrder).toHaveBeenCalledWith(4, ['second', 'first'], 'csrf', 'en_US'));
  } else {
    expect(saveOrder).not.toHaveBeenCalled();
    expect(client.getQueryData(settingsConsolePolicyCatalogQueryKey('en_US'))).toEqual(catalog);
  }
});


test('settings ordering submits plugin settings_feature_id while preserving canonical plugin identity', async () => {
  setOperations('root', []);
  const mixedCatalog: SettingsConsolePolicyCatalog = {
    ...catalog,
    groups: [
      catalog.groups[0],
      { kind: 'plugin', group_id: 'calendar', settings_feature_id: 'settings.calendar', label: 'Calendar', description: '', operations: [] },
      { kind: 'plugin', group_id: 'worker', settings_feature_id: null, label: 'Worker', description: '', operations: [] },
      catalog.groups[1]
    ]
  };
  // Keep the optimistic state available for inspection before the server response.
  saveOrder.mockImplementation(() => new Promise(() => {}));
  client.setQueryData(settingsConsolePolicyCatalogQueryKey('en_US'), mixedCatalog);
  const { result } = renderHook(() => {
    const { message } = App.useApp();
    return useConsoleSettingsOrder({ csrfToken: 'csrf', locale: 'en_US', catalog: mixedCatalog, queryClient: client, messageApi: message });
  }, { wrapper });
  act(() => result.current.reorder(1, 0));
  await waitFor(() => expect(saveOrder).toHaveBeenCalledWith(4, ['settings.calendar', 'first', 'second'], 'csrf', 'en_US'));
  const optimistic = client.getQueryData<SettingsConsolePolicyCatalog>(settingsConsolePolicyCatalogQueryKey('en_US'))!;
  expect(optimistic.groups.map(({ kind, group_id }) => [kind, group_id])).toEqual([
    ['plugin', 'calendar'], ['settings_feature', 'first'], ['settings_feature', 'second'], ['plugin', 'worker']
  ]);
});
