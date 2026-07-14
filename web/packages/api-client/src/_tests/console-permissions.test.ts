import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import { listConsolePermissions } from '../console-permissions';

describe('console permissions client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('uses the role SettingsFeature permission-options route (Issue #1256 AC-005)', async () => {
    await expect(listConsolePermissions()).resolves.toMatchObject({
      path: '/api/console/settings/roles/permission-options'
    });
  });
});
