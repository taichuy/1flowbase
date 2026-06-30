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
      enableConsoleAuthCenterAuthenticator('password-local', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/password-local/actions/enable',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('creates an auth center authenticator', async () => {
    await expect(
      createConsoleAuthCenterAuthenticator(
        {
          name: 'staff_password',
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
        name: 'staff_password',
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
        'staff_password',
        {
          name: 'staff_password_backup',
          title: 'Staff Password Backup',
          sort_order: 30
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/staff_password/copy',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        name: 'staff_password_backup',
        title: 'Staff Password Backup',
        sort_order: 30
      }
    });
  });

  test('deletes an auth center authenticator', async () => {
    await expect(
      deleteConsoleAuthCenterAuthenticator('staff_password', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/staff_password',
      method: 'DELETE',
      csrfToken: 'csrf-123',
      expectJson: false
    });
  });

  test('reorders auth center authenticators', async () => {
    await expect(
      reorderConsoleAuthCenterAuthenticators(
        {
          names: ['staff_password', 'password-local']
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/authenticators/order',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        names: ['staff_password', 'password-local']
      }
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
