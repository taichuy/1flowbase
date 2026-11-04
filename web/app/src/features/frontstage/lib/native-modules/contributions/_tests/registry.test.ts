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
const host = (exports: string[]): ConsolePluginSettingsCodeModule => ({
  source: 'react',
  version: '19.2.5',
  exports,
  binding: 'host',
  assets: [],
  type_declarations: 'declare module "react" {}'
});
const entry = (
  code_modules: ConsolePluginSettingsCodeModule[],
  disable_reason: NormalizedFrontstageBlockCatalogEntry['raw']['disable_reason'] = null,
  owner = 'installation-1'
) =>
  ({
    installationId: owner,
    pluginId: 'fixture@0.1.0',
    pluginVersion: '0.1.0',
    raw: { code_modules, disable_reason }
  }) as NormalizedFrontstageBlockCatalogEntry;

describe('authorized contribution module inventory', () => {
  test('deduplicates an identical package and dependencies shared by sibling components', () => {
    const result = collectContributionModules([
      entry([module, host(['useState'])]),
      entry([{ ...module }, host(['useState'])])
    ]);
    expect(result.modules).toEqual([module]);
    expect(result.hostBindingsByModule.get(module.source)).toEqual([
      host(['useState'])
    ]);
  });
  test('never admits modules or dependencies from a disabled contribution', () => {
    const result = collectContributionModules([
      entry([module, host(['useState'])], 'permission_denied')
    ]);
    expect(result.modules).toEqual([]);
    expect(result.hostBindingsByModule.size).toBe(0);
  });
  test('rejects incompatible fetched versions and cross-installation source collisions', () => {
    for (const other of [
      entry([{ ...module, version: '0.2.0' }]),
      entry([module], null, 'installation-2')
    ]) {
      expect(() =>
        collectContributionModules([entry([module]), other])
      ).toThrow('Frontend module identity conflict');
    }
  });
  test('independent owners can request different legal subsets of the same host module', () => {
    const other = { ...module, source: '@fixture/other' };
    const result = collectContributionModules([
      entry([module, host(['useState'])]),
      entry([other, host(['useEffect'])], null, 'installation-2')
    ]);
    expect(result.modules).toEqual([module, other]);
    expect(result.hostBindingsByModule.get(module.source)).toEqual([
      host(['useState'])
    ]);
    expect(result.hostBindingsByModule.get(other.source)).toEqual([
      host(['useEffect'])
    ]);
  });
  test('a dependency declared by another owner does not become an undeclared dependency', () => {
    const other = { ...module, source: '@fixture/other' };
    const result = collectContributionModules([
      entry([module]),
      entry([other, host(['useState'])], null, 'installation-2')
    ]);
    expect(result.hostBindingsByModule.get(module.source)).toEqual([]);
    expect(result.hostBindingsByModule.get(other.source)).toEqual([
      host(['useState'])
    ]);
    expect(result.policy[0]?.hostBindings).toEqual([]);
  });
});
