import { describe, expect, test, vi } from 'vitest';
import * as transport from '../../../transport';

import { fetchConsoleReleaseStatus } from '../../../console/administration/system';

describe('console-system client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads the release status route', async () => {
    await expect(fetchConsoleReleaseStatus()).resolves.toMatchObject({
      path: '/api/console/system/release-status'
    });
  });
});
