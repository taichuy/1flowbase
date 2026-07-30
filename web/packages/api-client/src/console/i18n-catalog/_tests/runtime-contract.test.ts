import { afterEach, describe, expect, expectTypeOf, test, vi } from 'vitest';

import {
  getRuntimeI18nCatalog,
  type ConditionalI18nCatalogResponse,
  type RuntimeI18nCatalog
} from '..';
import { getRuntimeI18nCatalog as catalogFromPackageEntry } from '../../../index';

describe('runtime i18n catalog client contract', () => {
  afterEach(() => vi.unstubAllGlobals());

  test('requests one global locale catalog with conditional ETag', async () => {
    expect(catalogFromPackageEntry).toBe(getRuntimeI18nCatalog);
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          catalog_revision: 7,
          locale: 'zh_Hans',
          digest: 'sha256:abc',
          messages: { Save: '保存' }
        }),
        { status: 200, headers: { etag: '"sha256:abc"' } }
      )
    );
    vi.stubGlobal('fetch', fetchMock);

    const catalog = getRuntimeI18nCatalog(
      { locale: 'zh_Hans', ifNoneMatch: '"old"' },
      'https://api.example.test'
    );
    await expect(catalog).resolves.toEqual({
      kind: 'ok',
      value: {
        catalog_revision: 7,
        locale: 'zh_Hans',
        digest: 'sha256:abc',
        messages: { Save: '保存' }
      },
      etag: '"sha256:abc"'
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'https://api.example.test/api/console/i18n/catalog?locale=zh_Hans',
      { credentials: 'include', headers: { 'if-none-match': '"old"' } }
    );
    expectTypeOf(catalog).toEqualTypeOf<
      Promise<ConditionalI18nCatalogResponse<RuntimeI18nCatalog>>
    >();
  });
});
