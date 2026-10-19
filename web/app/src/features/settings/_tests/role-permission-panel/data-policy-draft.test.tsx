import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { resetAuthStore } from '../../../../state/auth-store';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import {
  settingsRoleDataPolicyQueryKey,
  type SettingsRoleDataPolicy,
  type ReplaceSettingsRoleDataPolicyInput
} from '../../api/identity-access/roles';
import { settingsAllDataModelsQueryKey } from '../../api/data-models';
import { authenticate, defaultDataModels, defaultDataPolicy } from './fixtures';

const api = vi.hoisted(() => ({
  fetchSettingsRoleDataPolicy: vi.fn(),
  replaceSettingsRoleDataPolicy: vi.fn(),
  fetchSettingsAllDataModels: vi.fn()
}));
vi.mock('../../api/identity-access/roles', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/identity-access/roles')>()),
  fetchSettingsRoleDataPolicy: api.fetchSettingsRoleDataPolicy,
  replaceSettingsRoleDataPolicy: api.replaceSettingsRoleDataPolicy
}));
vi.mock('../../api/data-models', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/data-models')>()),
  fetchSettingsAllDataModels: api.fetchSettingsAllDataModels
}));

let RoleDataPolicySection: typeof import('../../components/role-permissions/RoleDataPolicySection').RoleDataPolicySection;

function setup(
  section: 'default-policy' | 'single-model-policy' = 'default-policy'
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  function view(roleCode: string) {
    return (
      <QueryClientProvider client={client}>
        <RoleDataPolicySection
          roleCode={roleCode}
          canEdit
          section={section}
          formId="data-policy"
        />
        <button type="submit" form="data-policy">
          Save policy
        </button>
      </QueryClientProvider>
    );
  }
  const { rerender } = render(view('member'));
  return {
    client,
    switchRole: (roleCode: string) => rerender(view(roleCode))
  };
}

describe('role data policy draft ownership', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await loadApplicationI18nResources();
    await appI18n.changeLanguage('zh_Hans');
    ({ RoleDataPolicySection } =
      await import('../../components/role-permissions/RoleDataPolicySection'));
    resetAuthStore();
    authenticate();
    api.fetchSettingsRoleDataPolicy.mockResolvedValue(defaultDataPolicy);
    api.fetchSettingsAllDataModels.mockResolvedValue(defaultDataModels());
    api.replaceSettingsRoleDataPolicy.mockImplementation(
      async (roleCode: string, input: ReplaceSettingsRoleDataPolicyInput) => ({
        role_code: roleCode,
        ...input
      })
    );
  });

  test('blocks external submission during a delayed role switch and never sends the previous role draft', async () => {
    let resolvePolicy!: (policy: SettingsRoleDataPolicy) => void;
    const pendingPolicy = new Promise<SettingsRoleDataPolicy>((resolve) => {
      resolvePolicy = resolve;
    });
    api.fetchSettingsRoleDataPolicy.mockImplementation((roleCode: string) =>
      roleCode === 'auditor'
        ? pendingPolicy
        : Promise.resolve(defaultDataPolicy)
    );
    const { switchRole } = setup();
    const create = await screen.findByRole('checkbox', { name: '新增 启用' });
    await waitFor(() => expect(create).toBeEnabled());
    fireEvent.click(create);
    expect(create).not.toBeChecked();
    switchRole('auditor');
    expect(screen.getByRole('checkbox', { name: '新增 启用' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
    expect(api.replaceSettingsRoleDataPolicy).not.toHaveBeenCalled();
    const auditorPolicy = {
      ...defaultDataPolicy,
      role_code: 'auditor'
    } as SettingsRoleDataPolicy;
    resolvePolicy(auditorPolicy);
    await waitFor(() =>
      expect(screen.getByRole('checkbox', { name: '新增 启用' })).toBeEnabled()
    );
    expect(screen.getByRole('checkbox', { name: '新增 启用' })).toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
    await waitFor(() =>
      expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalledWith(
        'auditor',
        expect.objectContaining({
          default_policy: auditorPolicy.default_policy
        }),
        'csrf-123'
      )
    );
  });

  test.each(['default-policy', 'single-model-policy'] as const)(
    'preserves dirty fields through policy and model refetch in %s while reconciling model ids',
    async (section) => {
      const { client } = setup(section);
      const checkboxName =
        section === 'default-policy' ? '新增 启用' : '新增 Orders';
      const checkbox = await screen.findByRole('checkbox', {
        name: checkboxName
      });
      await waitFor(() => expect(checkbox).toBeEnabled());
      fireEvent.click(checkbox);
      expect(checkbox).not.toBeChecked();
      const refreshedPolicy = {
        ...defaultDataPolicy,
        model_policies: defaultDataPolicy.model_policies.map((policy) => ({
          ...policy,
          update_scope_override: 'scope_all' as const
        }))
      };
      const models = defaultDataModels();
      const added = {
        ...models[0],
        id: 'model-new',
        title: 'New model',
        code: 'new_model'
      };
      // Simulate successful background query refreshes, not UI events.
      await act(async () => {
        client.setQueryData(
          settingsRoleDataPolicyQueryKey('member'),
          refreshedPolicy
        );
        client.setQueryData(settingsAllDataModelsQueryKey, [models[0], added]);
        // Query notification batching schedules the observer update on a timer.
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
      });
      expect(
        screen.getByRole('checkbox', { name: checkboxName })
      ).not.toBeChecked();
      fireEvent.click(screen.getByRole('button', { name: 'Save policy' }));
      await waitFor(() =>
        expect(api.replaceSettingsRoleDataPolicy).toHaveBeenCalled()
      );
      const [roleCode, payload] =
        api.replaceSettingsRoleDataPolicy.mock.calls[0];
      expect(roleCode).toBe('member');
      expect(payload.default_policy.can_create).toBe(
        section === 'default-policy' ? false : true
      );
      expect(payload.model_policies).toEqual([
        expect.objectContaining({
          data_model_id: 'model-orders',
          can_create_override: section === 'single-model-policy' ? false : null,
          update_scope_override: 'scope_all'
        }),
        {
          data_model_id: 'model-new',
          can_create_override: null,
          view_scope_override: null,
          update_scope_override: null,
          delete_scope_override: null
        }
      ]);
    }
  );
});
