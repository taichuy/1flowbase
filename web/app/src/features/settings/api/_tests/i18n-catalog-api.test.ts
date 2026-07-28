import { describe, expect, test, vi } from 'vitest';

const client = vi.hoisted(() => ({
  listI18nCatalogEntries: vi.fn(),
  getI18nCatalogEntry: vi.fn(),
  upsertI18nCatalogOverride: vi.fn(),
  upsertCustomI18nCatalogTranslation: vi.fn(),
  restoreI18nCatalogOverride: vi.fn(),
  deleteCustomI18nCatalogKey: vi.fn(),
  restoreAllI18nCatalogOverrides: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  ...client
}));

import {
  fetchSettingsI18nCatalogEntries,
  restoreAllSettingsI18nCatalogOverrides,
  saveSettingsI18nCatalogOverride,
  settingsI18nCatalogListQueryKey
} from '../i18n-catalog';

describe('settings i18n catalog API', () => {
  test('AC-008 preserves list filter and pagination request semantics', async () => {
    const request = {
      module: '@1flowbase/common',
      locale: 'zh_Hans',
      search: 'settings',
      origin: 'official_override' as const,
      offset: 20,
      limit: 20
    };
    client.listI18nCatalogEntries.mockResolvedValue({
      entries: [],
      total: 0,
      revision: 7
    });

    await fetchSettingsI18nCatalogEntries(request);

    expect(client.listI18nCatalogEntries).toHaveBeenCalledWith(request);
    expect(settingsI18nCatalogListQueryKey(request)).toEqual([
      'settings',
      'i18n-catalog',
      'list',
      request
    ]);
  });

  test('AC-008 forwards expected revision for entry and global writes', async () => {
    client.upsertI18nCatalogOverride.mockResolvedValue({ revision: 9 });
    client.restoreAllI18nCatalogOverrides.mockResolvedValue({ revision: 10 });
    const input = {
      module: 'common',
      msgid: 'Settings',
      locale: 'zh_Hans',
      translation: '设置',
      expected_revision: 8
    };

    await saveSettingsI18nCatalogOverride(input, 'csrf-123');
    await restoreAllSettingsI18nCatalogOverrides(
      { expected_revision: 9 },
      'csrf-123'
    );

    expect(client.upsertI18nCatalogOverride).toHaveBeenCalledWith(
      input,
      'csrf-123'
    );
    expect(client.restoreAllI18nCatalogOverrides).toHaveBeenCalledWith(
      { expected_revision: 9 },
      'csrf-123'
    );
  });
});
