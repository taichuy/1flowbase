import { describe, expect, test, vi } from 'vitest';

import { listConsoleApplicationManagement } from '../console/applications';
import * as transport from '../transport';

describe('console application management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-006 serializes resource filters, sorting, and pagination', async () => {
    await expect(
      listConsoleApplicationManagement({
        page: 2,
        page_size: 20,
        filter: {
          application_type: 'workflow',
          publication_status: 'unpublished'
        },
        sort: 'updated_at:desc'
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/applications?page=2&page_size=20&filter=%7B%22application_type%22%3A%22workflow%22%2C%22publication_status%22%3A%22unpublished%22%7D&sort=updated_at%3Adesc'
    });
  });
});
