import { describe, expect, test } from 'vitest';
import type { ConsolePluginSettingsCodeModule } from '@1flowbase/api-client';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../../block-catalog';
import { collectContributionModules } from '../registry';

const module: ConsolePluginSettingsCodeModule = {
  source: '@fixture/calendar',
  version: '0.1.0',
  exports: ['Calendar'],
  binding: 'fetched',
  assets: [],
  type_declarations: 'declare module "@fixture/calendar" {}'
};
const entry = (
  code_modules: ConsolePluginSettingsCodeModule[],
  disable_reason: NormalizedFrontstageBlockCatalogEntry['raw']['disable_reason'] = null
) =>
  ({
    raw: { code_modules, disable_reason }
  }) as NormalizedFrontstageBlockCatalogEntry;

describe('authorized contribution module inventory', () => {
  test('deduplicates one package shared by multiple component contributions', () => {
    expect(
      collectContributionModules([entry([module]), entry([{ ...module }])])
    ).toEqual([module]);
  });
  test('never admits modules from a disabled contribution', () => {
    expect(
      collectContributionModules([entry([module], 'permission_denied')])
    ).toEqual([]);
  });
  test('rejects conflicting identities instead of silently choosing a plugin version', () => {
    expect(() =>
      collectContributionModules([
        entry([module]),
        entry([{ ...module, version: '0.2.0' }])
      ])
    ).toThrow('Frontend module identity conflict');
  });
});
