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

// A fetched bundle must share the host React instance, not bundle a second one.
describe('fetched module host factory', () => {
  const react = { createElement: () => 'host-element', secret: 'not-declared' };
  const hostBinding: ConsolePluginSettingsCodeModule = {
    source: 'react',
    version: '19.2.0',
    exports: ['createElement'],
    binding: 'host',
    assets: [],
    type_declarations: 'declare module "react" {}'
  };
  const hostRegistryFactory = (
    registrations: Parameters<typeof createNativeReactModuleRegistry>[0]
  ) =>
    createNativeReactModuleRegistry([
      {
        module_source: 'react',
        exports: ['createElement', 'secret'],
        load: async () => ({ module: react })
      },
      ...registrations
    ]);

  test('passes only declared host exports to a verified factory and preserves identity', async () => {
    const factory = vi.fn((host: Record<string, Record<string, unknown>>) => {
      expect(Object.keys(host)).toEqual(['react']);
      expect(host.react!.createElement).toBe(react.createElement);
      expect(host.react).not.toHaveProperty('secret');
      return { answer: host.react!.createElement };
    });
    const loader = make({
      modules: [module, hostBinding],
      hostRegistryFactory,
      importModule: async () => ({ createFrontendModule: factory })
    });
    try {
      expect((await loader.registry.load(module.source)).answer).toBe(
        react.createElement
      );
      expect(factory).toHaveBeenCalledTimes(1);
    } finally {
      loader.dispose();
    }
  });

  test('rejects a factory result that lacks a declared export', async () => {
    const loader = make({
      modules: [module, hostBinding],
      hostRegistryFactory,
      importModule: async () => ({
        createFrontendModule: () => ({ other: true })
      })
    });
    try {
      await expect(loader.registry.load(module.source)).rejects.toThrow(
        'declared export missing'
      );
    } finally {
      loader.dispose();
    }
  });

  test('does not invoke a host factory when executable bytes fail integrity', async () => {
    const factory = vi.fn(() => ({ answer: 42 }));
    const loader = make({
      modules: [module, hostBinding],
      hostRegistryFactory,
      fetchAsset: async () => bytes('tampered'),
      importModule: async () => ({ createFrontendModule: factory })
    });
    try {
      await expect(loader.registry.load(module.source)).rejects.toThrow(
        'SHA-256 mismatch'
      );
      expect(factory).not.toHaveBeenCalled();
    } finally {
      loader.dispose();
    }
  });
});

// The workspace catalog is a set of independently authorized owners, not a
// single manifest. Loading one bundle must not widen its declared host exports.
describe('per-owner factory dependencies', () => {
  const hostBinding = (exports: string[]): ConsolePluginSettingsCodeModule => ({
    source: 'react',
    version: '19.2.5',
    exports,
    binding: 'host',
    assets: [],
    type_declarations: 'declare module "react" {}'
  });
  const other = { ...module, source: '@fixture/other' };
  const hostRegistryFactory = (
    registrations: Parameters<typeof createNativeReactModuleRegistry>[0]
  ) =>
    createNativeReactModuleRegistry([
      {
        module_source: 'react',
        exports: ['useState', 'useEffect'],
        load: async () => ({
          module: { useState: 'state', useEffect: 'effect' }
        })
      },
      ...registrations
    ]);

  test('a bundle with no dependencies receives none even when another owner declares React', async () => {
    const received: Record<string, Record<string, unknown>>[] = [];
    const loader = make({
      modules: [module, other],
      hostRegistryFactory,
      hostBindingsByModule: new Map([
        [module.source, []],
        [other.source, [hostBinding(['useState'])]]
      ]),
      importModule: async () => ({
        createFrontendModule: (
          bindings: Record<string, Record<string, unknown>>
        ) => {
          received.push(bindings);
          return { answer: () => 42 };
        }
      })
    });
    try {
      await loader.registry.load(module.source);
      await loader.registry.load(other.source);
      expect(received[0]).toEqual({});
      expect(received[1]).toEqual({ react: { useState: 'state' } });
      expect(Object.isFrozen(received[0])).toBe(true);
      expect(Object.isFrozen(received[1]!.react)).toBe(true);
    } finally {
      loader.dispose();
    }
  });

  test('each bundle gets only its own compatible host-export subset', async () => {
    const received: Record<string, unknown>[] = [];
    const loader = make({
      modules: [module, other],
      hostRegistryFactory,
      hostBindingsByModule: new Map([
        [module.source, [hostBinding(['useState'])]],
        [other.source, [hostBinding(['useEffect'])]]
      ]),
      importModule: async () => ({
        createFrontendModule: (bindings: Record<string, unknown>) => {
          received.push(bindings);
          return { answer: 42 };
        }
      })
    });
    try {
      await loader.registry.load(module.source);
      await loader.registry.load(other.source);
      expect(received).toEqual([
        { react: { useState: 'state' } },
        { react: { useEffect: 'effect' } }
      ]);
    } finally {
      loader.dispose();
    }
  });

  test('rejects dependency declarations without a fetched owner or with undeclared exports', () => {
    expect(() =>
      make({
        modules: [module],
        hostRegistryFactory,
        hostBindingsByModule: new Map([
          ['@fixture/missing', [hostBinding(['useState'])]]
        ])
      })
    ).toThrow('dependency owner is missing');
    expect(() =>
      make({
        modules: [module],
        hostRegistryFactory,
        hostBindingsByModule: new Map([
          [module.source, [hostBinding(['notRegistered'])]]
        ])
      })
    ).toThrow('Invalid host');
  });
});
