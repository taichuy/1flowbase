import { beforeEach, describe, expect, expectTypeOf, test, vi } from 'vitest';

import * as transport from '../../../transport';
import {
  deleteCustomI18nCatalogKey,
  getI18nCatalogEntry,
  listI18nCatalogEntries,
  restoreAllI18nCatalogOverrides,
  restoreI18nCatalogOverride,
  upsertCustomI18nCatalogTranslation,
  upsertI18nCatalogOverride,
  type I18nCatalogManagementEntry
} from '..';
import { listI18nCatalogEntries as listFromPackageEntry } from '../../../index';

describe('i18n catalog management client contract', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(transport, 'apiFetch').mockImplementation(
      async (input) => input as never
    );
  });

  test('exports the feature module from the package entrypoint', () => {
    expect(listFromPackageEntry).toBe(listI18nCatalogEntries);
  });

  test('AC-007 uses semantic filters and the detail identity contract', async () => {
    await expect(
      listI18nCatalogEntries({
        module: '@taichuy/platform/common',
        locale: 'zh_Hans',
        search: 'settings text',
        origin: 'official_override',
        offset: 20,
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/entries?module=%40taichuy%2Fplatform%2Fcommon&locale=zh_Hans&search=settings+text&origin=official_override&offset=20&limit=10'
    });

    const detail = getI18nCatalogEntry({
      module: '@taichuy/platform/common',
      msgid: 'Settings',
      locale: 'zh_Hans'
    });
    await expect(detail).resolves.toMatchObject({
      path: '/api/console/settings/i18n/entries/detail?module=%40taichuy%2Fplatform%2Fcommon&msgid=Settings&locale=zh_Hans'
    });
    expectTypeOf(detail).toEqualTypeOf<Promise<I18nCatalogManagementEntry>>();
  });

  test('AC-008 and AC-009 preserve one action per mutation route', async () => {
    const translation = {
      module: '@taichuy/platform/common',
      msgid: 'Settings',
      locale: 'zh_Hans',
      translation: '设置',
      expected_revision: 7
    };
    const restore = {
      module: translation.module,
      msgid: translation.msgid,
      locale: translation.locale,
      expected_revision: 8
    };

    await expect(upsertI18nCatalogOverride(translation, 'csrf')).resolves.toMatchObject({
      path: '/api/console/settings/i18n/overrides',
      method: 'PUT',
      body: translation,
      csrfToken: 'csrf'
    });
    await expect(restoreI18nCatalogOverride(restore, 'csrf')).resolves.toMatchObject({
      path: '/api/console/settings/i18n/overrides',
      method: 'DELETE',
      body: restore
    });
    await expect(
      upsertCustomI18nCatalogTranslation(translation, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/custom-translations',
      method: 'PUT',
      body: translation
    });
    await expect(
      deleteCustomI18nCatalogKey(
        {
          module: translation.module,
          msgid: 'custom.key',
          expected_revision: 9
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/custom-keys',
      method: 'DELETE'
    });
    await expect(
      restoreAllI18nCatalogOverrides({ expected_revision: 10 }, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/restore-overrides',
      method: 'POST',
      body: { expected_revision: 10 }
    });
  });
});
