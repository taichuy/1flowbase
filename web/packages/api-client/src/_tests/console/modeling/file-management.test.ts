import { describe, expect, test, vi } from 'vitest';
import * as transport from '../../../transport';

import {
  createConsoleFileShareLink,
  createConsoleFileStorage,
  createConsoleFileTable,
  deleteConsoleFileStorage,
  deleteConsoleFileTable,
  fetchConsoleFileStorages,
  fetchConsoleFileRecords,
  fetchConsoleFileTables,
  updateConsoleFileStorage,
  updateConsoleFileTableBinding,
  uploadConsoleFile
} from '../../../console/modeling/file-management';

describe('console-file-management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test.each([
    {
      name: 'storage collection',
      request: () => fetchConsoleFileStorages(),
      expected: { path: '/api/console/settings/files/storages' }
    },
    {
      name: 'file-table collection',
      request: () => fetchConsoleFileTables(),
      expected: { path: '/api/console/settings/files/tables' }
    }
  ])('reads the $name route', async ({ request, expected }) => {
    await expect(request()).resolves.toMatchObject(expected);
  });

  test('encodes filename filtering and sorting with server pagination', async () => {
    const controller = new AbortController();
    const result = (await fetchConsoleFileRecords(
      'files/demo',
      2,
      10,
      controller.signal,
      { filename: 'a&b.png', sort: 'size:asc' }
    )) as unknown as transport.ApiRequestOptions;
    const url = new URL(result.path, 'https://example.test');
    expect(url.pathname).toBe('/api/runtime/models/files%2Fdemo/list');
    expect(url.searchParams.get('page')).toBe('2');
    expect(url.searchParams.get('page_size')).toBe('10');
    expect(JSON.parse(url.searchParams.get('filter')!)).toEqual({
      filename: { $includes: 'a&b.png' }
    });
    expect(url.searchParams.get('sort')).toBe('size:asc');
    expect(result.signal).toBe(controller.signal);
  });

  test('multipart upload includes the selected table and File and preserves cancellation and CSRF', async () => {
    const file = new File(['hello'], 'hello.txt', { type: 'text/plain' });
    const controller = new AbortController();
    const result = (await uploadConsoleFile(
      file,
      'table-1',
      'csrf-123',
      controller.signal
    )) as unknown as transport.ApiRequestOptions;
    expect(result.path).toBe('/api/console/files/upload');
    expect(result.method).toBe('POST');
    expect(result.csrfToken).toBe('csrf-123');
    expect(result.signal).toBe(controller.signal);
    expect(result.contentType).toBeUndefined();
    expect((result.rawBody as FormData).get('file')).toBe(file);
    expect((result.rawBody as FormData).get('file_table_id')).toBe('table-1');
  });

  test('file sharing preserves IDs, expiration, CSRF and cancellation', async () => {
    const controller = new AbortController();
    await expect(
      createConsoleFileShareLink(
        'table/1',
        'record 1',
        { expires_in_seconds: 123 },
        'csrf',
        controller.signal
      )
    ).resolves.toMatchObject({
      path: '/api/console/files/table%2F1/records/record%201/access-links',
      method: 'POST',
      body: { expires_in_seconds: 123 },
      csrfToken: 'csrf',
      signal: controller.signal
    });
  });

  test('clearing a binding sends null rather than an invalid UUID', async () => {
    await expect(
      updateConsoleFileTableBinding(
        'table-1',
        { bound_storage_id: null },
        'csrf-123'
      )
    ).resolves.toMatchObject({ body: { bound_storage_id: null } });
  });

  test.each([
    {
      name: 'file-table binding update',
      request: () =>
        updateConsoleFileTableBinding(
          'table-1',
          { bound_storage_id: 'storage-1' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/settings/files/tables/table-1/binding',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'storage creation',
      request: () =>
        createConsoleFileStorage(
          {
            code: 'local-default',
            title: 'Local',
            driver_type: 'local',
            enabled: true,
            is_default: true,
            config_json: { root_path: 'api/storage' },
            rule_json: {}
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/settings/files/storages',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'file-table creation',
      request: () =>
        createConsoleFileTable(
          {
            code: 'workspace_assets',
            title: 'Workspace Assets'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/settings/files/tables',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'storage update',
      request: () =>
        updateConsoleFileStorage(
          'storage-1',
          {
            title: 'Archive Local',
            enabled: false
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/settings/files/storages/storage-1',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'storage deletion',
      request: () => deleteConsoleFileStorage('storage-1', 'csrf-123'),
      expected: {
        path: '/api/console/settings/files/storages/storage-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'file-table deletion',
      request: () => deleteConsoleFileTable('table-1', 'csrf-123'),
      expected: {
        path: '/api/console/settings/files/tables/table-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    }
  ])(
    'writes $name through the console file-management route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );
});
