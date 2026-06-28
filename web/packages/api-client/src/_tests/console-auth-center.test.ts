import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import { fetchConsoleAuthCenterOverview } from '../console-auth-center';

describe('console auth center client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads the auth center overview route', async () => {
    await expect(fetchConsoleAuthCenterOverview()).resolves.toMatchObject({
      path: '/api/console/settings/auth-center/overview'
    });
  });
});
