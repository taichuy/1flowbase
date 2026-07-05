import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  fetchConsoleRoleDataPolicy,
  replaceConsoleRoleDataPolicy,
  type ReplaceConsoleRoleDataPolicyInput
} from '../console-roles';

describe('console roles client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );

  test('fetches role data policy through the data-policy route', async () => {
    await expect(fetchConsoleRoleDataPolicy('manager')).resolves.toMatchObject({
      path: '/api/console/roles/manager/data-policy'
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
      replaceConsoleRoleDataPolicy('manager', input, 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/roles/manager/data-policy',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: input
    });
  });
});
