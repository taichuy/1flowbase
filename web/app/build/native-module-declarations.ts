import { existsSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import type { Plugin, ResolvedConfig } from 'vite';

import {
  FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES,
  isFrontstageNativeReactResolvedDeclarationSource
} from '../src/features/frontstage/lib/native-modules/resolved-dependency-sources';

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
    // Reuse the editor's explicit public entries and discovered DnD package
    // roots. Deep antd/dayjs imports stay indexed, but do not split primaries.
    resources ??= createDeclarationResources(
      graph,
      options.moduleSources.filter(
        (source) =>
          FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES.some(
            (primary) => primary === source
          ) ||
          (source.startsWith('@dnd-kit/') &&
            isFrontstageNativeReactResolvedDeclarationSource(source))
      )
    );
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
    configurePreviewServer(server) {
      const distDir = resolve(server.config.root, server.config.build.outDir);
      const basePrefix = server.config.base.endsWith('/')
        ? server.config.base
        : `${server.config.base}/`;
      const urlPrefix = `${basePrefix}${RESOURCE_PREFIX}`;
      server.middlewares.use((request, response, next) => {
        const pathname = request.url?.split('?')[0] ?? '';
        if (!pathname.startsWith(urlPrefix)) return next();
        const name = pathname.slice(urlPrefix.length);
        if (!/^[a-f0-9]{64}\.json$/u.test(name)) return next();
        const emittedPath = join(distDir, RESOURCE_PREFIX, name);
        if (!existsSync(emittedPath) || !statSync(emittedPath).isFile()) {
          response.statusCode = 404;
          response.end();
          return;
        }
        response.setHeader(
          'Cache-Control',
          'public, max-age=31536000, immutable'
        );
        return next();
      });
    },
    watchChange() {
      graph = undefined;
      resources = undefined;
    }
  };
}
