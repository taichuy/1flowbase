import { afterEach, describe, expect, expectTypeOf, test, vi } from 'vitest';

import {
  getRuntimeI18nBundle,
  getRuntimeI18nManifest,
  type ConditionalI18nCatalogResponse,
  type RuntimeI18nBundle,
  type RuntimeI18nManifest
} from '..';
import { getRuntimeI18nManifest as manifestFromPackageEntry } from '../../../index';

describe('AC-011 runtime i18n catalog client contract', () => {
  afterEach(() => vi.unstubAllGlobals());

  test('exports the runtime surface and sends encoded paths with conditional headers', async () => {
    expect(manifestFromPackageEntry).toBe(getRuntimeI18nManifest);
    const fetchMock = vi
      .fn()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            catalog_revision: 7,
            locale: 'zh_Hans',
            modules: []
          }),
          { status: 200, headers: { etag: '"manifest"' } }
        )
      );
    vi.stubGlobal('fetch', fetchMock);

    const manifest = getRuntimeI18nManifest(
      { locale: 'zh_Hans', ifNoneMatch: '"old"' },
      'https://api.example.test'
    );
    await expect(manifest).resolves.toEqual({
      kind: 'ok',
      value: { catalog_revision: 7, locale: 'zh_Hans', modules: [] },
      etag: '"manifest"'
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'https://api.example.test/api/console/i18n/manifest?locale=zh_Hans',
      { credentials: 'include', headers: { 'if-none-match': '"old"' } }
    );
    expectTypeOf(manifest).toEqualTypeOf<
      Promise<ConditionalI18nCatalogResponse<RuntimeI18nManifest>>
    >();

    fetchMock.mockResolvedValueOnce(
      new Response(null, { status: 304, headers: { etag: '"sha256:new"' } })
    );
    const bundle = getRuntimeI18nBundle(
      {
        module: '@taichuy/platform/common',
        locale: 'zh_Hans',
        digest: 'sha256:abc',
        ifNoneMatch: '"sha256:abc"'
      },
      'https://api.example.test'
    );
    await expect(bundle).resolves.toEqual({
      kind: 'not_modified',
      etag: '"sha256:new"'
    });
    expect(fetchMock).toHaveBeenLastCalledWith(
      'https://api.example.test/api/console/i18n/bundles/sha256%3Aabc?module=%40taichuy%2Fplatform%2Fcommon&locale=zh_Hans',
      { credentials: 'include', headers: { 'if-none-match': '"sha256:abc"' } }
    );
    expectTypeOf(bundle).toEqualTypeOf<
      Promise<ConditionalI18nCatalogResponse<RuntimeI18nBundle>>
    >();
  });
});
