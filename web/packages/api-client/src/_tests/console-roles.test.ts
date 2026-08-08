import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleRole,
  deleteConsoleRole,
  fetchConsoleRoleDataPolicy,
  fetchConsoleRoleConsolePolicy,
  fetchConsoleRoleConsolePolicyCatalog,
  fetchConsoleRoleFrontstageRoutes,
  fetchConsoleRolePermissions,
  listConsoleRoles,
  replaceConsoleRoleConsolePolicy,
  replaceConsoleRoleDataPolicy,
  replaceConsoleRoleFrontstageRoutes,
  replaceConsoleRolePermissions,
  updateConsoleRole,
  type ConsolePolicyCatalog,
  type ReplaceConsoleRoleDataPolicyInput
} from '../console-roles';

describe('console roles client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );

  test('uses the Settings namespace for every role-management route (Issue #1256 AC-003)', async () => {
    const role = {
      code: 'member',
      name: 'Member',
      introduction: 'Workspace member'
    };

    await expect(listConsoleRoles()).resolves.toMatchObject({
      path: '/api/console/settings/roles'
    });
    await expect(createConsoleRole(role, 'csrf-123')).resolves.toMatchObject({
      path: '/api/console/settings/roles',
      method: 'POST'
    });
    await expect(
      updateConsoleRole('member', role, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member',
      method: 'PATCH'
    });
    await expect(
      deleteConsoleRole('member', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member',
      method: 'DELETE'
    });
    await expect(fetchConsoleRolePermissions('member')).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/permissions'
    });
    await expect(
      replaceConsoleRolePermissions(
        'member',
        { permission_codes: [] },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/permissions',
      method: 'PUT'
    });
    await expect(
      fetchConsoleRoleFrontstageRoutes('member')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/frontstage-routes'
    });
    await expect(
      replaceConsoleRoleFrontstageRoutes(
        'member',
        { page_ids: [], tab_ids: [] },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/frontstage-routes',
      method: 'PUT'
    });
    await expect(fetchConsoleRoleDataPolicy('member')).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/data-policy'
    });
  });

  test('requests the localized console policy catalog and exposes its echoed locale (Issue #1259 AC-003/004/009)', async () => {
    const catalog: ConsolePolicyCatalog = {
      schema_version: '2026-07-15',
      locale: 'en_US',
      settings_order_revision: 0,
      group_strategy_options: [
        { value: 'full', label: 'Full', description: 'All operations' },
        { value: 'custom', label: 'Custom', description: 'Explicit operations' }
      ],
      groups: [],
      resources: []
    };
    vi.mocked(transport.apiFetch).mockResolvedValueOnce(catalog as never);

    await expect(
      fetchConsoleRoleConsolePolicyCatalog('en_US')
    ).resolves.toEqual(catalog);
    expect(transport.apiFetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/roles/console-policy-catalog?locale=en_US',
      baseUrl: undefined
    });
  });

  test('keeps group activation independent from its retained custom interface policy (Issue #1485 AC-002)', async () => {
    const input = {
      groups: [
        {
          kind: 'settings_feature' as const,
          group_id: 'settings.applications',
          enabled: false,
          strategy: 'custom' as const,
          operations: [
            {
              operation_id: 'applications.read',
              kind: 'row' as const,
              scope: 'own' as const
            },
            {
              operation_id: 'applications.publish',
              kind: 'simple' as const,
              enabled: true
            }
          ]
        }
      ]
    };

    await expect(
      fetchConsoleRoleConsolePolicy('member')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/console-policy'
    });
    await expect(
      replaceConsoleRoleConsolePolicy('member', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/console-policy',
      method: 'PUT',
      body: input,
      csrfToken: 'csrf-123'
    });
  });

  test('replaces role data policy with backend field names', async () => {
    const input: ReplaceConsoleRoleDataPolicyInput = {
      default_policy: {
        can_view: true,
        can_create: false,
        can_update: true,
        can_delete: false,
        default_view_scope: 'own',
        default_update_scope: 'scope_all',
        default_delete_scope: 'own'
      },
      model_policies: [
        {
          data_model_id: 'model-orders',
          can_create_override: false,
          view_scope_override: null,
          update_scope_override: 'own',
          delete_scope_override: 'scope_all'
        }
      ]
    };

    await expect(
      replaceConsoleRoleDataPolicy('member', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/roles/member/data-policy',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: input
    });
  });
});
