import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import { fetchPublicLoginEntries, signInWithPassword } from '../public-auth';

describe('public auth client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads public login entries', async () => {
    await expect(fetchPublicLoginEntries()).resolves.toMatchObject({
      path: '/api/public/auth/login-entries'
    });
  });

  test('submits password sign-in with selected login entry', async () => {
    await expect(
      signInWithPassword({
        login_entry_id: 'auth-staff-password',
        identifier: 'root',
        password: 'change-me'
      })
    ).resolves.toMatchObject({
      path: '/api/public/auth/sign-in',
      method: 'POST',
      body: {
        login_entry_id: 'auth-staff-password',
        identifier: 'root',
        password: 'change-me'
      }
    });
  });
});
