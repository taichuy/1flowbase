import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import { fetchPublicLoginInstances, signInWithPassword } from '../public-auth';

describe('public auth client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads public login instances', async () => {
    await expect(fetchPublicLoginInstances()).resolves.toMatchObject({
      path: '/api/public/auth/login-instances'
    });
  });

  test('submits password sign-in with selected authenticator', async () => {
    await expect(
      signInWithPassword({
        authenticator: 'staff_password',
        identifier: 'root',
        password: 'change-me'
      })
    ).resolves.toMatchObject({
      path: '/api/public/auth/providers/password-local/sign-in',
      method: 'POST',
      body: {
        authenticator: 'staff_password',
        identifier: 'root',
        password: 'change-me'
      }
    });
  });
});
