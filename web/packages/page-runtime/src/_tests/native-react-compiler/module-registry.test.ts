import { describe, expect, test, vi } from 'vitest';

import {
  createNativeReactModuleRegistry,
  NativeReactModuleRegistryError
} from '../../index';

describe('Native React frontend module registry', () => {
  test('AC-003/006 single-flights the current frontend module implementation', async () => {
    const load = vi.fn(async () => ({
      module: { Surface: 'current-implementation' }
    }));
    const registry = createNativeReactModuleRegistry([
      {
        module_source: '@1flowbase/native-components',
        exports: ['Surface'],
        load
      }
    ]);

    expect(registry.definitions).toEqual([
      {
        module_source: '@1flowbase/native-components',
        exports: ['Surface']
      }
    ]);
    await expect(
      Promise.all([
        registry.load('@1flowbase/native-components'),
        registry.load('@1flowbase/native-components')
      ])
    ).resolves.toEqual([
      { Surface: 'current-implementation' },
      { Surface: 'current-implementation' }
    ]);
    expect(load).toHaveBeenCalledTimes(1);
  });

  test('AC-009 returns registered styles as ShadowRoot assets', async () => {
    const registry = createNativeReactModuleRegistry([
      {
        module_source: '@1flowbase/native-components',
        exports: ['Surface'],
        load: async () => ({
          module: { Surface: 'surface' },
          styles: [{ css: '.surface { display: block; }' }]
        })
      }
    ]);

    const assets = await registry.resolveModuleAssets([
      '@1flowbase/native-components'
    ]);
    expect(assets).toHaveLength(1);
    expect(assets[0]).toMatchObject({
      module_source: '@1flowbase/native-components',
      role: 'shadow_style',
      media_type: 'text/css; charset=utf-8',
      sha256: expect.stringMatching(/^[a-f0-9]{64}$/),
      url: expect.stringMatching(/^frontend-module-style:/)
    });
    expect(new TextDecoder().decode(assets[0]!.bytes)).toContain('.surface');
  });

  test('AC-008 fails clearly for an unavailable module or export', async () => {
    const registry = createNativeReactModuleRegistry([
      {
        module_source: '@1flowbase/native-components',
        exports: ['Surface'],
        load: async () => ({ module: {} })
      }
    ]);

    await expect(registry.load('@missing/module')).rejects.toMatchObject({
      code: 'module_not_registered',
      path: 'modules.@missing/module'
    });
    await expect(
      registry.load('@1flowbase/native-components')
    ).rejects.toMatchObject({
      code: 'module_export_missing',
      path: 'modules.@1flowbase/native-components.Surface'
    });
  });

  test('loads only requested named exports when loadExports is provided, and falls back to full load on namespace imports', async () => {
    const load = vi.fn(async () => ({
      module: { FormBlock: 'form', TableBlock: 'table' },
      styles: [{ css: '.table{}' }, { css: '.form{}' }]
    }));
    const loadExports = vi.fn(async (requested: readonly string[]) => ({
      module: Object.fromEntries(requested.map((name) => [name, name.toLowerCase()])),
      styles: [{ css: '.form-only{}' }]
    }));
    const registry = createNativeReactModuleRegistry([
      {
        module_source: '@1flowbase/data-blocks',
        exports: ['TableBlock', 'FormBlock'],
        load,
        loadExports
      }
    ]);

    const selective = await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/data-blocks',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/data-blocks',
            imported: 'FormBlock',
            local: 'FormBlock'
          }
        ]
      }
    ]);
    expect(selective).toEqual({
      '@1flowbase/data-blocks': { FormBlock: 'formblock' }
    });
    expect(loadExports).toHaveBeenCalledTimes(1);
    expect(loadExports).toHaveBeenCalledWith(['FormBlock']);
    expect(load).not.toHaveBeenCalled();

    const assets = await registry.resolveInjectedModuleAssets!([
      {
        source: '@1flowbase/data-blocks',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/data-blocks',
            imported: 'FormBlock',
            local: 'FormBlock'
          }
        ]
      }
    ]);
    expect(loadExports).toHaveBeenCalledTimes(1);
    expect(assets).toHaveLength(1);
    expect(new TextDecoder().decode(assets[0]!.bytes)).toBe('.form-only{}');

    const fullNamespace = await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/data-blocks',
        bindings: [
          {
            kind: 'namespace',
            source: '@1flowbase/data-blocks',
            local: 'DataBlocks'
          }
        ]
      }
    ]);
    expect(fullNamespace).toEqual({
      '@1flowbase/data-blocks': { FormBlock: 'form', TableBlock: 'table' }
    });
    expect(load).toHaveBeenCalledTimes(1);
  });

  test('rejects duplicate or malformed frontend registrations', () => {
    expect(() =>
      createNativeReactModuleRegistry([
        { module_source: 'react', exports: ['default'], load: async () => ({ module: {} }) },
        { module_source: 'react', exports: ['default'], load: async () => ({ module: {} }) }
      ])
    ).toThrowError(NativeReactModuleRegistryError);
  });
});
