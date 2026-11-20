import type { Plugin, ResolvedConfig } from 'vite';

import { collectNativeModuleDeclarationGraph } from './native-module-declarations/graph';
import { serializeDeclarationIndex } from './native-module-declarations/index';
import { createDeclarationResources } from './native-module-declarations/resources';
export type { CollectedNativeModuleDeclaration } from './native-module-declarations/resources';

export const NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID =
  'virtual:1flowbase-native-module-declarations';
const RESOLVED_VIRTUAL_ID = `\0${NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID}`;
const RESOURCE_PREFIX = 'assets/native-declarations/';

type Options = { moduleSources: readonly string[]; projectRoot: string };

export function collectNativeModuleDeclarations(options: Options) {
  return collectNativeModuleDeclarationGraph(options).extraLibs;
}

export function nativeModuleDeclarationsPlugin(options: Options): Plugin {
  let graph: ReturnType<typeof collectNativeModuleDeclarationGraph> | undefined;
  let resources: ReturnType<typeof createDeclarationResources> | undefined;
  let config: ResolvedConfig;
  const collect = () => {
    graph ??= collectNativeModuleDeclarationGraph(options);
    resources ??= createDeclarationResources(graph);
    return resources;
  };
  return {
    name: '1flowbase-native-module-declarations',
    enforce: 'pre',
    configResolved(resolved) {
      config = resolved;
    },
    resolveId: {
      filter: { id: /^virtual:1flowbase-native-module-declarations$/u },
      handler(id) {
        return id === NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID
          ? RESOLVED_VIRTUAL_ID
          : undefined;
      }
    },
    load: {
      filter: { id: /^\x00virtual:1flowbase-native-module-declarations$/u },
      handler(id) {
        if (id !== RESOLVED_VIRTUAL_ID) return undefined;
        const index = collect();
        for (const file of graph!.watchedFiles) this.addWatchFile(file);
        return serializeDeclarationIndex(index, (hash) => {
          const source = index.resources[hash]!;
          const fileName = `${RESOURCE_PREFIX}${hash}.json`;
          return config.command === 'build'
            ? `import.meta.ROLLUP_FILE_URL_${this.emitFile({ type: 'asset', fileName, source })}`
            : JSON.stringify(`${config.base}${fileName}`);
        });
      }
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const name = request.url?.split('?')[0]?.split(RESOURCE_PREFIX)[1];
        if (!name || !/^[a-f0-9]{64}\.json$/.test(name)) return next();
        const content = collect().resources[name.slice(0, -5)];
        if (!content) return next();
        response.setHeader('Content-Type', 'application/json; charset=utf-8');
        response.setHeader(
          'Cache-Control',
          'public, max-age=31536000, immutable'
        );
        response.end(content);
      });
    },
    watchChange() {
      graph = undefined;
      resources = undefined;
    }
  };
}
