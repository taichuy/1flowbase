import { beforeEach, describe, expect, test, vi } from 'vitest';

const apiClient = vi.hoisted(() => ({
  applyConsoleInstalledMcpExtension: vi.fn(),
  checkConsoleExtensionUpdates: vi.fn(),
  deleteConsoleInstalledExtension: vi.fn(),
  getConsoleExtensionCatalogEntry: vi.fn(),
  getConsoleInstalledMcpExtensionConflict: vi.fn(),
  getConsoleInstalledMcpExtensionIntegrityChallenge: vi.fn(),
  getConsoleExtensionRiskChallenge: vi.fn(),
  installConsoleExtension: vi.fn(),
  listConsoleExtensionCatalog: vi.fn(),
  listConsoleInstalledExtensions: vi.fn(),
  previewConsoleInstalledMcpExtension: vi.fn()
}));

vi.mock('@1flowbase/api-client', () => apiClient);

import {
  checkSettingsExtensionUpdates,
  fetchSettingsExtensionCatalog,
  settingsExtensionCatalogQueryKey
} from '../extensions';

describe('settings extension catalog query contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('AC-003/AC-007 isolates remote pages by q, slot_code, and cursor', () => {
    expect(
      settingsExtensionCatalogQueryKey('runtime-extensions', {
        q: 'postgres',
        slot_code: 'data_source',
        cursor: 'page-2'
      })
    ).toEqual([
      'settings',
      'extension-center',
      'catalog',
      'runtime-extensions',
      'postgres',
      'data_source',
      'page-2'
    ]);
    expect(
      settingsExtensionCatalogQueryKey('runtime-extensions', {
        q: 'openai',
        slot_code: undefined,
        cursor: undefined
      })
    ).not.toEqual(
      settingsExtensionCatalogQueryKey('runtime-extensions', {
        q: 'postgres',
        slot_code: undefined,
        cursor: undefined
      })
    );
  });

  test('AC-003 forwards generic catalog filters without hardcoding model_provider', () => {
    fetchSettingsExtensionCatalog('runtime-extensions', {
      q: 'postgres',
      cursor: 'page-2'
    });

    expect(apiClient.listConsoleExtensionCatalog).toHaveBeenCalledWith(
      'runtime-extensions',
      { q: 'postgres', cursor: 'page-2' }
    );
  });

  test('API-F2 forwards update checks with category and items only', () => {
    const input = {
      category: 'runtime-extensions' as const,
      items: [
        {
          catalog_id: 'runtime-extensions:taichuy/openai',
          current_version: '1.0.0',
          installed_versions: ['1.0.0']
        }
      ]
    };

    checkSettingsExtensionUpdates(input, 'csrf');

    expect(apiClient.checkConsoleExtensionUpdates).toHaveBeenCalledWith(
      input,
      'csrf'
    );
  });
});
