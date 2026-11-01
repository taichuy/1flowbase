import {
  fetchConsoleFrontendModuleAsset,
  type ConsolePluginSettingsCodeModule
} from '@1flowbase/api-client';
import {
  createNativeReactModuleRegistry,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import { createFetchedNativeReactModuleRegistry } from '../../../../../shared/code-block/fetched-modules/registry';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../block-catalog';
import { createFrontstageNativeReactModuleRegistry } from '../registry';
import { getFrontstageBlockCatalogApiBaseUrl } from '../../../api/block-catalog';

type Module = ConsolePluginSettingsCodeModule;
export interface ContributionModuleInventory {
  modules: Module[];
  hostBindingsByModule: ReadonlyMap<string, readonly Module[]>;
  policy: {
    source: string;
    owner: string;
    module: Module;
    hostBindings: Module[];
  }[];
}

/** Keep dependency ownership after flattening the workspace's authorized catalog. */
export function collectContributionModules(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[]
): ContributionModuleInventory {
  const inventory = new Map<
    string,
    ContributionModuleInventory['policy'][number]
  >();
  for (const entry of entries) {
    if (entry.raw.disable_reason !== null) continue;
    const owner = JSON.stringify([
      entry.installationId,
      entry.pluginId,
      entry.pluginVersion
    ]);
    const hostBindings = entry.raw.code_modules.filter(
      ({ binding }) => binding === 'host'
    );
    for (const module of entry.raw.code_modules) {
      if (module.binding === 'host') continue;
      const item = { source: module.source, owner, module, hostBindings };
      const current = inventory.get(module.source);
      if (current && JSON.stringify(current) !== JSON.stringify(item))
        throw new Error(`Frontend module identity conflict: ${module.source}`);
      inventory.set(module.source, item);
    }
  }
  const policy = [...inventory.values()];
  return {
    modules: policy.map(({ module }) => module),
    hostBindingsByModule: new Map(
      policy.map(({ source, hostBindings }) => [source, hostBindings])
    ),
    policy
  };
}

export function createContributionModuleRegistry(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[],
  hostRegistryFactory: () => NativeReactModuleRegistry = createFrontstageNativeReactModuleRegistry
) {
  const inventory = collectContributionModules(entries);
  const loader = createFetchedNativeReactModuleRegistry({
    modules: inventory.modules,
    hostBindingsByModule: inventory.hostBindingsByModule,
    hostRegistryFactory: (registrations) => {
      const host = hostRegistryFactory();
      const fetched = createNativeReactModuleRegistry(registrations);
      const fetchedSources = new Set(
        fetched.definitions.map(({ module_source }) => module_source)
      );
      const registryFor = (source: string) =>
        fetchedSources.has(source) ? fetched : host;
      return {
        definitions: [...host.definitions, ...fetched.definitions],
        load: (source) => registryFor(source).load(source),
        async resolveModuleMap(sources) {
          return Object.fromEntries(
            await Promise.all(
              [...new Set(sources)].map(async (source) => [
                source,
                await registryFor(source).load(source)
              ])
            )
          );
        },
        async resolveModuleAssets(sources) {
          const unique = [...new Set(sources)];
          const assets = await Promise.all([
            host.resolveModuleAssets(
              unique.filter((source) => !fetchedSources.has(source))
            ),
            fetched.resolveModuleAssets(
              unique.filter((source) => fetchedSources.has(source))
            )
          ]);
          return assets.flat();
        }
      };
    },
    fetchAsset: (asset, signal) =>
      fetchConsoleFrontendModuleAsset(
        asset.sha256,
        signal,
        getFrontstageBlockCatalogApiBaseUrl()
      )
  });
  return { ...loader, modules: inventory.modules, policy: inventory.policy };
}
