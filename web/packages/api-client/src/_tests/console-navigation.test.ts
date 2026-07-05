import { describe, expect, test, vi } from 'vitest';

import * as transport from '../transport';
import { getConsoleNavigation } from '../console-navigation';

describe('console navigation client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('reads the console navigation route', async () => {
    await expect(getConsoleNavigation()).resolves.toMatchObject({
      path: '/api/console/navigation'
    });
  });
});
