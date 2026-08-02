import { beforeEach, describe, expect, expectTypeOf, test, vi } from 'vitest';

import * as transport from '../../../transport';
import {
  activateI18nCatalogUpdate,
  activateInstalledI18nCatalog,
  deleteCustomI18nCatalogKey,
  getI18nCatalogState,
  getI18nCatalogEntry,
  getI18nCatalogUpdateStatus,
  listI18nCatalogEntries,
  previewInstalledI18nCatalog,
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
        locale: 'zh_Hans',
        search: 'settings text',
        origin: 'official_override',
        offset: 20,
        limit: 10
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/entries?locale=zh_Hans&search=settings+text&origin=official_override&offset=20&limit=10'
    });

    const detail = getI18nCatalogEntry({
      key: 'Settings',
      locale: 'zh_Hans'
    });
    await expect(detail).resolves.toMatchObject({
      path: '/api/console/settings/i18n/entries/detail?key=Settings&locale=zh_Hans'
    });
    expectTypeOf(detail).toEqualTypeOf<Promise<I18nCatalogManagementEntry>>();
  });

  test('AC-008 and AC-009 preserve one action per mutation route', async () => {
    const translation = {
      key: 'Settings',
      locale: 'zh_Hans',
      translation: '设置',
      expected_revision: 7
    };
    const restore = {
      key: translation.key,
      locale: translation.locale,
      expected_revision: 8
    };

    await expect(
      upsertI18nCatalogOverride(translation, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/overrides',
      method: 'PUT',
      body: translation,
      csrfToken: 'csrf'
    });
    await expect(
      restoreI18nCatalogOverride(restore, 'csrf')
    ).resolves.toMatchObject({
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
          key: 'custom.key',
          expected_revision: 9
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/custom-keys',
      method: 'DELETE',
      body: {
        key: 'custom.key',
        expected_revision: 9
      }
    });
    await expect(
      restoreAllI18nCatalogOverrides({ expected_revision: 10 }, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/restore-overrides',
      method: 'POST',
      body: { expected_revision: 10 }
    });
  });

  test('AC-005 owns official and installed catalog activation routes', async () => {
    await expect(getI18nCatalogState()).resolves.toMatchObject({
      path: '/api/console/settings/i18n/catalog'
    });
    await expect(getI18nCatalogUpdateStatus()).resolves.toMatchObject({
      path: '/api/console/settings/i18n/update-check'
    });
    await expect(
      activateI18nCatalogUpdate({ expected_revision: 8 }, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/activate',
      method: 'POST',
      body: { expected_revision: 8 },
      csrfToken: 'csrf'
    });
    await expect(
      previewInstalledI18nCatalog('installation-1')
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/installed-extension/installation-1/preview'
    });
    await expect(
      activateInstalledI18nCatalog(
        'installation-1',
        {
          expected_revision: 8,
          integrity_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['checksum_mismatch']
          }
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/i18n/installed-extension/installation-1/activate',
      method: 'POST',
      body: {
        expected_revision: 8,
        integrity_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['checksum_mismatch']
        }
      },
      csrfToken: 'csrf'
    });
  });
});
