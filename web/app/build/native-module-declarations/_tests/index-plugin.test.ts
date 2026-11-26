import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { PreviewServer, ResolvedConfig } from 'vite';
import { describe, expect, test, vi } from 'vitest';

import {
  NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID,
  nativeModuleDeclarationsPlugin
} from '../../native-module-declarations';
import { createDeclarationResources } from '../resources';

const graph = vi.hoisted(() => ({
  extraLibs: [
    'react',
    'shared',
    '@dnd-kit/core',
    'dayjs/plugin/a',
    'dayjs/plugin/b'
  ].map((name) => ({
    content: name,
    filePath: `file:///node_modules/${name}/index.d.ts`,
    source: name
  })),
  roots: {
    react: ['file:///node_modules/react/index.d.ts'],
    'react/jsx-runtime': ['file:///node_modules/react/index.d.ts'],
    '@dnd-kit/core': ['file:///node_modules/@dnd-kit/core/index.d.ts'],
    'dayjs/plugin/a': ['file:///node_modules/dayjs/plugin/a/index.d.ts'],
    'dayjs/plugin/b': ['file:///node_modules/dayjs/plugin/b/index.d.ts']
  },
  dependencies: {
    'file:///node_modules/@dnd-kit/core/index.d.ts': [
      'file:///node_modules/shared/index.d.ts'
    ],
    'file:///node_modules/react/index.d.ts': [
      'file:///node_modules/shared/index.d.ts'
    ]
  },
  watchedFiles: ['/fixture/entry/index.d.ts']
}));
vi.mock('../graph', () => ({
  collectNativeModuleDeclarationGraph: () => graph
}));

describe('declaration index plugin serialization', () => {
  test.each(['build', 'serve'] as const)(
    '%s keeps emitted assets and base-aware URLs unchanged',
    (command) => {
      const plugin = nativeModuleDeclarationsPlugin({
        projectRoot: '/fixture',
        moduleSources: Object.keys(graph.roots)
      });
      const configResolved = plugin.configResolved;
      const load = plugin.load;
      if (
        typeof configResolved !== 'function' ||
        !load ||
        typeof load !== 'object'
      ) {
        throw new Error(
          'Expected configResolved function and filtered load hook'
        );
      }
      Reflect.apply(configResolved, {}, [
        { command, base: '/nested/' } as ResolvedConfig
      ]);
      const emitted: { type: string; fileName: string; source: string }[] = [];
      const context = {
        addWatchFile: vi.fn(),
        emitFile(asset: { type: string; fileName: string; source: string }) {
          emitted.push(asset);
          return `asset${emitted.length - 1}`;
        }
      };
      const source: string = Reflect.apply(load.handler, context, [
        `\0${NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID}`
      ]);
      const index = createDeclarationResources(graph, [
        'react',
        'react/jsx-runtime',
        '@dnd-kit/core'
      ]);
      expect(source).toContain('const groups=');
      expect(context.addWatchFile).toHaveBeenCalledWith(graph.watchedFiles[0]);
      if (command === 'build') {
        expect(emitted).toEqual(
          Object.entries(index.resources)
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([hash, content]) => ({
              type: 'asset',
              fileName: `assets/native-declarations/${hash}.json`,
              source: content
            }))
        );
        expect(source.match(/import\.meta\.ROLLUP_FILE_URL_/g)).toHaveLength(
          emitted.length
        );
      } else {
        expect(emitted).toHaveLength(0);
      }
      const resolved = source.replace(
        /import\.meta\.ROLLUP_FILE_URL_asset(\d+)/g,
        (_, id: string) =>
          JSON.stringify(`/nested/${emitted[Number(id)]!.fileName}`)
      );
      const actual = new Function(
        resolved.replace('export default', 'return')
      )();
      expect(actual).toEqual({
        version: 1,
        roots: index.roots,
        resources: Object.fromEntries(
          Object.keys(index.resources).map((hash) => [
            hash,
            `/nested/assets/native-declarations/${hash}.json`
          ])
        )
      });
      expect(actual.roots.react).toBe(actual.roots['react/jsx-runtime']);
      expect(actual.roots['dayjs/plugin/a']).toBe(
        actual.roots['dayjs/plugin/b']
      );
      const dndFiles = actual.roots['@dnd-kit/core'].flatMap((hash: string) =>
        JSON.parse(index.resources[hash]!)
      );
      expect(
        dndFiles.map((file: { filePath: string }) => file.filePath)
      ).not.toContain('file:///node_modules/react/index.d.ts');
    }
  );

  test('configurePreviewServer marks emitted declaration JSON immutable and returns 404 without immutable on misses', () => {
    const tempRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), 'native-decl-preview-')
    );
    try {
      const distDir = path.join(tempRoot, 'dist');
      const existingHash = 'a'.repeat(64);
      const missingHash = 'b'.repeat(64);
      const emittedFile = path.join(
        distDir,
        'assets/native-declarations',
        `${existingHash}.json`
      );
      fs.mkdirSync(path.dirname(emittedFile), { recursive: true });
      fs.writeFileSync(emittedFile, '[]');

      const plugin = nativeModuleDeclarationsPlugin({
        projectRoot: '/fixture',
        moduleSources: Object.keys(graph.roots)
      });
      const middlewares: Array<
        (
          req: { url?: string },
          res: {
            statusCode: number;
            headers: Record<string, string>;
            ended: boolean;
            setHeader: (k: string, v: string) => void;
            end: () => void;
          },
          next: () => void
        ) => void
      > = [];
      const configurePreviewServer = plugin.configurePreviewServer;
      if (typeof configurePreviewServer !== 'function') {
        throw new Error('Expected configurePreviewServer function');
      }
      Reflect.apply(configurePreviewServer, {}, [
        {
          config: {
            root: tempRoot,
            base: '/nested/',
            build: { outDir: 'dist' }
          },
          middlewares: {
            use(fn: (typeof middlewares)[number]) {
              middlewares.push(fn);
            }
          }
        } as unknown as PreviewServer
      ]);

      expect(middlewares).toHaveLength(1);
      const middleware = middlewares[0]!;

      const createRes = () => {
        const res = {
          statusCode: 200,
          headers: {} as Record<string, string>,
          ended: false,
          setHeader(key: string, value: string) {
            res.headers[key.toLowerCase()] = value;
          },
          end() {
            res.ended = true;
          }
        };
        return res;
      };

      // Hit: sets immutable header and delegates to static file middleware
      const hitRes = createRes();
      const hitNext = vi.fn();
      middleware(
        { url: `/nested/assets/native-declarations/${existingHash}.json` },
        hitRes,
        hitNext
      );
      expect(hitNext).toHaveBeenCalledOnce();
      expect(hitRes.ended).toBe(false);
      expect(hitRes.headers['cache-control']).toBe(
        'public, max-age=31536000, immutable'
      );

      // Miss: returns 404 without SPA fallback or immutable header
      const missRes = createRes();
      const missNext = vi.fn();
      middleware(
        { url: `/nested/assets/native-declarations/${missingHash}.json` },
        missRes,
        missNext
      );
      expect(missNext).not.toHaveBeenCalled();
      expect(missRes.statusCode).toBe(404);
      expect(missRes.ended).toBe(true);
      expect(missRes.headers['cache-control']).toBeUndefined();
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  });
});
