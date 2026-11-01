import type { ConsolePluginSettingsCodeModule } from '@1flowbase/api-client';
import {
  sha256Bytes,
  type NativeReactFrontendModuleRegistration,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime/browser';

type Asset = ConsolePluginSettingsCodeModule['assets'][number];

/** Each page epoch owns its authenticated fetches and module/style flights. */
export function createFetchedNativeReactModuleRegistry({
  modules,
  hostRegistryFactory,
  fetchAsset,
  importModule = (url) => import(/* @vite-ignore */ url),
  createObjectURL = (blob) => URL.createObjectURL(blob),
  revokeObjectURL = (url) => URL.revokeObjectURL(url)
}: {
  modules: readonly ConsolePluginSettingsCodeModule[];
  hostRegistryFactory: (
    registrations: readonly NativeReactFrontendModuleRegistration[]
  ) => NativeReactModuleRegistry;
  fetchAsset: (asset: Asset, signal: AbortSignal) => Promise<ArrayBuffer>;
  importModule?: (url: string) => Promise<Record<string, unknown>>;
  createObjectURL?: (blob: Blob) => string;
  revokeObjectURL?: (url: string) => void;
}): { registry: NativeReactModuleRegistry; dispose(): void } {
  const host = hostRegistryFactory([]);
  const hostExportsBySource = new Map<string, Set<string>>();
  for (const definition of host.definitions) {
    if (!hostExportsBySource.has(definition.module_source)) {
      hostExportsBySource.set(
        definition.module_source,
        new Set(definition.exports)
      );
    }
  }
  const sources = new Set<string>();
  const controller = new AbortController();
  const urls = new Set<string>();
  let disposed = false;
  const assertCurrent = () => {
    if (disposed) throw new Error('Frontend module page epoch disposed.');
  };
  const verifiedBytes = async (asset: Asset) => {
    assertCurrent();
    const bytes = await fetchAsset(asset, controller.signal);
    assertCurrent();
    // HTTP LAN/IP deployments have no Web Crypto subtle API. Use the shared
    // byte-level primitive so integrity checks apply on every supported origin.
    const actual = sha256Bytes(new Uint8Array(bytes));
    if (actual !== asset.sha256)
      throw new Error('Frontend module SHA-256 mismatch.');
    assertCurrent();
    return bytes;
  };
  const registrations: NativeReactFrontendModuleRegistration[] = [];
  for (const module of modules) {
    if (
      !module.source.trim() ||
      !module.version.trim() ||
      !module.type_declarations.trim() ||
      sources.has(module.source) ||
      !module.exports.length ||
      new Set(module.exports).size !== module.exports.length ||
      module.exports.some((name) => !/^[A-Za-z_$][\w$]*$/.test(name))
    ) {
      throw new Error('Invalid frontend module declaration.');
    }
    sources.add(module.source);
    const hostExports = hostExportsBySource.get(module.source);
    if (module.binding === 'host') {
      if (
        !hostExports ||
        module.assets.length ||
        module.exports.some((name) => !hostExports.has(name))
      ) {
        throw new Error('Invalid host frontend module declaration.');
      }
      continue;
    }
    if (module.binding !== 'fetched' || hostExports)
      throw new Error('Frontend module source collision.');
    const paths = new Set<string>();
    for (const asset of module.assets) {
      if (
        !asset.path ||
        asset.path.startsWith('/') ||
        asset.path.includes('\\') ||
        asset.path
          .split('/')
          .some((part) => !part || part === '.' || part === '..') ||
        paths.has(asset.path) ||
        !/^[a-f0-9]{64}$/.test(asset.sha256) ||
        !['browser_module', 'shadow_style', 'support'].includes(asset.role) ||
        (asset.role === 'browser_module' &&
          asset.media_type !== 'text/javascript; charset=utf-8') ||
        (asset.role === 'shadow_style' &&
          asset.media_type !== 'text/css; charset=utf-8')
      ) {
        throw new Error('Invalid frontend module asset declaration.');
      }
      paths.add(asset.path);
    }
    const browserAssets = module.assets.filter(
      ({ role }) => role === 'browser_module'
    );
    if (browserAssets.length !== 1)
      throw new Error('Frontend module needs exactly one browser asset.');
    registrations.push({
      module_source: module.source,
      exports: module.exports,
      async load() {
        // Verify every executable/style byte before evaluating the module.
        const assets = module.assets.filter(({ role }) => role !== 'support');
        const bytes = await Promise.all(assets.map(verifiedBytes));
        assertCurrent();
        const index = assets.findIndex(({ role }) => role === 'browser_module');
        const url = createObjectURL(
          new Blob([bytes[index]!], { type: 'text/javascript' })
        );
        urls.add(url);
        try {
          const imported = await importModule(url);
          assertCurrent();
          // Optional bundle factory: bind only manifest-declared host exports.
          // This shares React/AntD identity without global variables, import maps,
          // or another React copy inside the verified plugin bundle.
          let namespace = imported;
          let hostStyles: {
            css: string;
            media_type: 'text/css; charset=utf-8';
          }[] = [];
          if (Object.hasOwn(imported, 'createFrontendModule')) {
            if (typeof imported.createFrontendModule !== 'function')
              throw new Error('Invalid frontend module factory.');
            const bindings = modules.filter(
              ({ binding }) => binding === 'host'
            );
            const resolved = await host.resolveModuleMap(
              bindings.map(({ source }) => source)
            );
            assertCurrent();
            const hostModules = Object.fromEntries(
              bindings.map((binding) => [
                binding.source,
                Object.freeze(
                  Object.fromEntries(
                    binding.exports.map((name) => [
                      name,
                      resolved[binding.source]![name]
                    ])
                  )
                )
              ])
            );
            const created: unknown = await imported.createFrontendModule(
              Object.freeze(hostModules)
            );
            assertCurrent();
            if (!created || typeof created !== 'object')
              throw new Error('Invalid frontend module factory result.');
            namespace = created as Record<string, unknown>;
            const dependencyAssets = await host.resolveModuleAssets(
              bindings.map(({ source }) => source)
            );
            assertCurrent();
            hostStyles = dependencyAssets
              .filter(({ role }) => role === 'shadow_style')
              .map((asset) => ({
                css: new TextDecoder('utf-8', { fatal: true }).decode(
                  asset.bytes
                ),
                media_type: 'text/css; charset=utf-8' as const
              }));
          }
          assertCurrent();
          if (module.exports.some((name) => !Object.hasOwn(namespace, name)))
            throw new Error('Frontend module declared export missing.');
          return {
            module: Object.fromEntries(
              module.exports.map((name) => [name, namespace[name]])
            ),
            styles: [
              ...hostStyles,
              ...assets.flatMap((asset, assetIndex) =>
                asset.role === 'shadow_style'
                  ? [
                      {
                        css: new TextDecoder('utf-8', { fatal: true }).decode(
                          bytes[assetIndex]!
                        ),
                        media_type: 'text/css; charset=utf-8' as const
                      }
                    ]
                  : []
              )
            ]
          };
        } finally {
          if (urls.delete(url)) revokeObjectURL(url);
        }
      }
    });
  }
  return {
    registry: hostRegistryFactory(registrations),
    dispose() {
      disposed = true;
      controller.abort();
      for (const url of urls) revokeObjectURL(url);
      urls.clear();
    }
  };
}
