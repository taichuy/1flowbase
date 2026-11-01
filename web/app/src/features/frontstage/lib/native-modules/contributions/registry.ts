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

/** Catalog entries have already passed workspace assignment and grant resolution. */
export function collectContributionModules(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[]
): ConsolePluginSettingsCodeModule[] {
  const modules = new Map<string, ConsolePluginSettingsCodeModule>();
  for (const entry of entries) {
    if (entry.raw.disable_reason !== null) continue;
    for (const module of entry.raw.code_modules) {
      const current = modules.get(module.source);
      if (current && JSON.stringify(current) !== JSON.stringify(module))
        throw new Error(`Frontend module identity conflict: ${module.source}`);
      modules.set(module.source, module);
    }
  }
  return [...modules.values()];
}

export function createContributionModuleRegistry(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[],
  hostRegistryFactory: () => NativeReactModuleRegistry = createFrontstageNativeReactModuleRegistry
) {
  const modules = collectContributionModules(entries);
  const loader = createFetchedNativeReactModuleRegistry({
    modules,
    hostRegistryFactory: (registrations) => {
      // Preserve caller-owned registry facades (including test/custom hosts)
      // while adding only the authorized fetched registrations.
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
  return { ...loader, modules };
}
