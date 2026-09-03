import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  copyConsoleAuthCenterLoginEntry,
  createConsoleAuthCenterLoginEntry,
  deleteConsoleAuthCenterLoginEntry,
  fetchConsoleAuthCenterOverview,
  reorderConsoleAuthCenterLoginEntries,
  updateConsoleAuthCenterLoginEntryEnabled,
  updateConsoleAuthCenterLoginEntryConfig,
  updateConsoleAuthCenterLoginEntryPublicUiBlock
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

  test('updates whether an auth center login entry is enabled', async () => {
    await expect(
      updateConsoleAuthCenterLoginEntryEnabled(
        'auth-password-local',
        { enabled: false },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/auth-password-local/enabled',
      method: 'PUT',
      body: { enabled: false },
      csrfToken: 'csrf-123'
    });
  });

  test('creates an auth center login entry', async () => {
    await expect(
      createConsoleAuthCenterLoginEntry(
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
      path: '/api/console/settings/auth-center/login-entries',
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

  test('copies an auth center login entry', async () => {
    await expect(
      copyConsoleAuthCenterLoginEntry(
        'auth-staff-password',
        {
          title: 'Staff Password Backup',
          sort_order: 30
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/auth-staff-password/copy',
      method: 'POST',
      csrfToken: 'csrf-123',
      body: {
        title: 'Staff Password Backup',
        sort_order: 30
      }
    });
  });

  test('deletes an auth center login entry', async () => {
    await expect(
      deleteConsoleAuthCenterLoginEntry('auth-staff-password', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/auth-staff-password',
      method: 'DELETE',
      csrfToken: 'csrf-123',
      expectJson: false
    });
  });

  test('reorders auth center login entries', async () => {
    await expect(
      reorderConsoleAuthCenterLoginEntries(
        {
          ids: ['auth-staff-password', 'auth-password-local']
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/order',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        ids: ['auth-staff-password', 'auth-password-local']
      }
    });
  });

  test('AC-017 updates login entry config without the public Block', async () => {
    await expect(
      updateConsoleAuthCenterLoginEntryConfig(
        'auth-oidc-main',
        {
          title: 'OIDC Login',
          enabled: true,
          description: 'Primary OIDC login',
          self_registration_enabled: false,
          extension_config: { issuer: 'https://id.example.com' }
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/auth-oidc-main/config',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: {
        title: 'OIDC Login',
        enabled: true,
        description: 'Primary OIDC login',
        self_registration_enabled: false,
        extension_config: { issuer: 'https://id.example.com' }
      }
    });
  });

  test('AC-018 updates only the login entry public UI Block', async () => {
    await expect(
      updateConsoleAuthCenterLoginEntryPublicUiBlock(
        'auth-oidc-main',
        { public_ui_block: 'export default { main };' },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/login-entries/auth-oidc-main/public-ui-block',
      method: 'PUT',
      csrfToken: 'csrf-123',
      body: { public_ui_block: 'export default { main };' }
    });
  });
});
