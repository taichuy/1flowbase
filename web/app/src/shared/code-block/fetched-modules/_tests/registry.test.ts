import { createHash } from 'node:crypto';
import { afterEach, describe, expect, test, vi } from 'vitest';
import { createNativeReactModuleRegistry } from '@1flowbase/page-runtime/browser';
import type { ConsolePluginSettingsCodeModule } from '@1flowbase/api-client';
import { createFetchedNativeReactModuleRegistry } from '../registry';

const js = 'export function answer() { return 42; }';
const css = '.terminal { color: green; }';
const hash = (text: string) => createHash('sha256').update(text).digest('hex');
const bytes = (text: string) => new TextEncoder().encode(text).buffer;
const module: ConsolePluginSettingsCodeModule = {
  source: '@fixture/terminal',
  version: '1.0.0',
  exports: ['answer'],
  binding: 'fetched',
  type_declarations:
    "declare module '@fixture/terminal' { export function answer(): number; }",
  assets: [
    {
      path: 'ui/terminal.js',
      role: 'browser_module',
      media_type: 'text/javascript; charset=utf-8',
      sha256: hash(js)
    },
    {
      path: 'ui/terminal.css',
      role: 'shadow_style',
      media_type: 'text/css; charset=utf-8',
      sha256: hash(css)
    }
  ]
};
const make = (
  overrides: Partial<
    Parameters<typeof createFetchedNativeReactModuleRegistry>[0]
  > = {}
) => {
  const revokeObjectURL = vi.fn();
  const importModule = vi.fn(async () => ({ answer: () => 42 }));
  const loader = createFetchedNativeReactModuleRegistry({
    modules: [module],
    hostRegistryFactory: createNativeReactModuleRegistry,
    fetchAsset: async (asset) =>
      bytes(asset.role === 'browser_module' ? js : css),
    createObjectURL: () => 'blob:verified',
    revokeObjectURL,
    importModule,
    ...overrides
  });
  return { ...loader, importModule, revokeObjectURL };
};

describe('settings page fetched native modules', () => {
  afterEach(() => vi.unstubAllGlobals());

  test('loads verified JS and CSS on HTTP origins without Web Crypto', async () => {
    vi.stubGlobal('crypto', { subtle: undefined });
    const loader = make();
    try {
      expect((await loader.registry.load(module.source)).answer).toBeTypeOf(
        'function'
      );
      const assets = await loader.registry.resolveModuleAssets([module.source]);
      expect(new TextDecoder().decode(assets[0]!.bytes)).toBe(css);
    } finally {
      loader.dispose();
    }
  });

  test('verified JS loads through the canonical registry and CSS uses its shadow asset path', async () => {
    const loader = make();
    expect((await loader.registry.load(module.source)).answer).toBeTypeOf(
      'function'
    );
    const assets = await loader.registry.resolveModuleAssets([module.source]);
    expect(assets).toHaveLength(1);
    expect(assets[0]).toMatchObject({
      role: 'shadow_style',
      sha256: hash(css),
      module_source: module.source
    });
    expect(new TextDecoder().decode(assets[0]!.bytes)).toBe(css);
    expect(loader.importModule).toHaveBeenCalledTimes(1);
    expect(loader.revokeObjectURL).toHaveBeenCalledWith('blob:verified');
    loader.dispose();
  });

  test('evaluates self-contained ESM bytes after integrity verification', async () => {
    const loader = make({
      modules: [{ ...module, assets: [module.assets[0]!] }],
      createObjectURL: () => `data:text/javascript,${encodeURIComponent(js)}`,
      importModule: (url) => import(/* @vite-ignore */ url)
    });
    const loaded = await loader.registry.load(module.source);
    expect((loaded.answer as () => number)()).toBe(42);
    loader.dispose();
  });

  test.each(['browser_module', 'shadow_style'])(
    'tampered %s prevents any module evaluation',
    async (role) => {
      vi.stubGlobal('crypto', { subtle: undefined });
      const loader = make({
        fetchAsset: async (asset) =>
          bytes(
            asset.role === role
              ? 'tampered'
              : asset.role === 'browser_module'
                ? js
                : css
          )
      });
      await expect(loader.registry.load(module.source)).rejects.toThrow(
        'SHA-256 mismatch'
      );
      expect(loader.importModule).not.toHaveBeenCalled();
      loader.dispose();
    }
  );

  test('missing bytes and missing declared exports fail closed', async () => {
    const missing = make({
      fetchAsset: async () => {
        throw new Error('missing');
      }
    });
    await expect(missing.registry.load(module.source)).rejects.toThrow(
      'missing'
    );
    expect(missing.importModule).not.toHaveBeenCalled();
    const exportMissing = make({ importModule: async () => ({ other: true }) });
    await expect(exportMissing.registry.load(module.source)).rejects.toThrow(
      'declared export missing'
    );
    missing.dispose();
    exportMissing.dispose();
  });

  test('escape, malformed declaration and source collisions are rejected before fetching', () => {
    expect(() => make({ modules: [module, module] })).toThrow(
      'Invalid frontend module declaration'
    );
    expect(() =>
      make({ modules: [{ ...module, exports: ['bad-export'] }] })
    ).toThrow();
    expect(() =>
      make({
        modules: [
          {
            ...module,
            assets: [{ ...module.assets[0]!, path: '../escape.js' }]
          }
        ]
      })
    ).toThrow();
    expect(() =>
      make({
        hostRegistryFactory: (entries) =>
          createNativeReactModuleRegistry([
            {
              module_source: module.source,
              exports: ['answer'],
              load: async () => ({ module: { answer: true } })
            },
            ...entries
          ])
      })
    ).toThrow('collision');
  });

  test('disposal cancels a pending fetch and cannot import stale page bytes', async () => {
    let release!: (value: ArrayBuffer) => void;
    const pending = new Promise<ArrayBuffer>((resolve) => {
      release = resolve;
    });
    let signal: AbortSignal | undefined;
    const loader = make({
      modules: [{ ...module, assets: [module.assets[0]!] }],
      fetchAsset: async (_, received) => {
        signal = received;
        return pending;
      }
    });
    const flight = loader.registry.load(module.source);
    await Promise.resolve();
    loader.dispose();
    expect(signal?.aborted).toBe(true);
    release(bytes(js));
    await expect(flight).rejects.toThrow('disposed');
    expect(loader.importModule).not.toHaveBeenCalled();
  });

  test('module-free pages preserve host definitions', () => {
    const loader = make({
      modules: [],
      hostRegistryFactory: (entries) =>
        createNativeReactModuleRegistry([
          {
            module_source: 'react',
            exports: ['useState'],
            load: async () => ({ module: { useState: true } })
          },
          ...entries
        ])
    });
    expect(loader.registry.definitions).toEqual([
      { module_source: 'react', exports: ['useState'] }
    ]);
    loader.dispose();
  });
});
