import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsolePersonalAccessToken,
  listConsolePersonalAccessTokenRoleOptions,
  listConsolePersonalAccessTokens,
  revokeConsolePersonalAccessToken
} from '../console/personal-access-tokens';

describe('console personal access tokens client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('lists user API keys from the console route', async () => {
    await expect(listConsolePersonalAccessTokens()).resolves.toMatchObject({
      path: '/api/console/user-api-keys'
    });
  });

  test('creates user API keys with expiration policy and csrf token', async () => {
    await expect(
      createConsolePersonalAccessToken(
        {
          name: 'CI diagnostics',
          role_code: 'member',
          expiration_policy: '1y'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/user-api-keys',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        name: 'CI diagnostics',
        role_code: 'member',
        expiration_policy: '1y'
      }
    });
  });

  test('lists role options for user API keys from the console route', async () => {
    await expect(
      listConsolePersonalAccessTokenRoleOptions()
    ).resolves.toMatchObject({
      path: '/api/console/user-api-keys/role-options'
    });
  });

  test('revokes user API keys through the revoke action route', async () => {
    await expect(
      revokeConsolePersonalAccessToken('key-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/user-api-keys/key-1/revoke',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });
});
