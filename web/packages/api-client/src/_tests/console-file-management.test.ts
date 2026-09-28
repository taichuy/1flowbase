import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleFileStorage,
  createConsoleFileTable,
  deleteConsoleFileStorage,
  deleteConsoleFileTable,
  fetchConsoleFileStorages,
  fetchConsoleFileTables,
  updateConsoleFileStorage,
  updateConsoleFileTableBinding
} from '../console-file-management';

describe('console-file-management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(async (input) => input as never);

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
  ])('writes $name through the console file-management route', async ({ request, expected }) => {
    await expect(request()).resolves.toMatchObject(expected);
  });
});
