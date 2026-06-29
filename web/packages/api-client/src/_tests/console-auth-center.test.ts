import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  enableConsoleAuthCenterAuthenticator,
  fetchConsoleAuthCenterOverview,
  updateConsoleAuthCenterAuthenticatorConfig
} from '../console-auth-center';

describe('console auth center client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads the auth center overview route', async () => {
    await expect(fetchConsoleAuthCenterOverview()).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/overview'
    });
  });

  test('enables an auth center authenticator', async () => {
    await expect(
      enableConsoleAuthCenterAuthenticator('password-local', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/password-local/actions/enable',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('updates an auth center authenticator config without extension_config', async () => {
    await expect(
      updateConsoleAuthCenterAuthenticatorConfig(
        'oidc-main',
        {
          name: 'oidc-main',
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/oidc-main/config',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        name: 'oidc-main',
        title: 'OIDC Login',
        enabled: true,
        description: 'Primary OIDC login'
      }
    });
  });
});
