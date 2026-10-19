import { describe, expect, test, vi } from 'vitest';
import * as transport from '../../../transport';

import {
  fetchConsoleApiDocsCatalog,
  fetchConsoleApiDocsCategoryOperations,
  fetchConsoleApiDocsCategorySpec,
  fetchConsoleApiOperationSpec
} from '../../../console/api-docs';

describe('console-api-docs client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(async (input) => input as never);

  test('reads the API docs catalog route', async () => {
    await expect(fetchConsoleApiDocsCatalog()).resolves.toMatchObject({
      path: '/api/console/docs/catalog'
    });
  });

  test('reads paged category operations with search query parameters', async () => {
    await expect(
      fetchConsoleApiDocsCategoryOperations('console tools', {
        offset: 20,
        limit: 20,
        q: 'api key'
      })
    ).resolves.toMatchObject({
      path: '/api/console/docs/categories/console%20tools/operations?offset=20&limit=20&q=api+key'
    });
  });

  test('omits empty category operation query parameters', async () => {
    await expect(
      fetchConsoleApiDocsCategoryOperations('console', {
        offset: 0,
        limit: 20,
        q: null
      })
    ).resolves.toMatchObject({
      path: '/api/console/docs/categories/console/operations?offset=0&limit=20'
    });
  });

  test('reads raw OpenAPI documents without success unwrapping', async () => {
    await expect(fetchConsoleApiDocsCategorySpec('console')).resolves.toMatchObject({
      path: '/api/console/docs/categories/console/openapi.json',
      unwrapSuccess: false
    });

    await expect(fetchConsoleApiOperationSpec('patch_me')).resolves.toMatchObject({
      path: '/api/console/docs/operations/patch_me/openapi.json',
      unwrapSuccess: false
    });
  });
});
