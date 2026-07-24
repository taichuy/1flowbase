import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  copyConsoleAuthCenterAuthenticator,
  createConsoleAuthCenterAuthenticator,
  deleteConsoleAuthCenterAuthenticator,
  enableConsoleAuthCenterAuthenticator,
  fetchConsoleAuthCenterOverview,
  reorderConsoleAuthCenterAuthenticators,
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
      enableConsoleAuthCenterAuthenticator('auth-password-local', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/auth-password-local/actions/enable',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('creates an auth center authenticator', async () => {
    await expect(
      createConsoleAuthCenterAuthenticator(
        {
          auth_type: 'password-local',
          title: 'Staff Password',
          description: 'Staff login',
          enabled: false,
          sort_order: 20
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        auth_type: 'password-local',
        title: 'Staff Password',
        description: 'Staff login',
        enabled: false,
        sort_order: 20
      }
    });
  });

  test('copies an auth center authenticator', async () => {
    await expect(
      copyConsoleAuthCenterAuthenticator(
        'auth-staff-password',
        {
          title: 'Staff Password Backup',
          sort_order: 30
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/auth-staff-password/copy',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        title: 'Staff Password Backup',
        sort_order: 30
      }
    });
  });

  test('deletes an auth center authenticator', async () => {
    await expect(
      deleteConsoleAuthCenterAuthenticator('auth-staff-password', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/auth-staff-password',
      method: 'DELETE',
      csrfToken: 'csrf-123',
      expectJson: false
    });
  });

  test('reorders auth center authenticators', async () => {
    await expect(
      reorderConsoleAuthCenterAuthenticators(
        {
          ids: ['auth-staff-password', 'auth-password-local']
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/order',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        ids: ['auth-staff-password', 'auth-password-local']
      }
    });
  });

  test('updates an auth center authenticator config and its public Block truth', async () => {
    await expect(
      updateConsoleAuthCenterAuthenticatorConfig(
        'auth-oidc-main',
        {
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login',
          self_registration_enabled: false,
          public_ui_block: 'export default { main };',
          extension_config: { issuer: 'https://id.example.com' }
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/auth-oidc-main/config',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        title: 'OIDC Login',
        enabled: true,
        description: 'Primary OIDC login',
        self_registration_enabled: false,
        public_ui_block: 'export default { main };',
        extension_config: { issuer: 'https://id.example.com' }
      }
    });
  });
});
