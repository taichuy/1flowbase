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
  settingsI18nCatalogListQueryKey
} from '../i18n-catalog';

describe('settings i18n catalog API', () => {
  test('AC-008 preserves list filter and pagination request semantics', async () => {
    const request = {
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
});
