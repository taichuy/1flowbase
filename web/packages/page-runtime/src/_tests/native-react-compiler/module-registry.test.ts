import { describe, expect, test, vi } from 'vitest';

import {
  createNativeReactModuleRegistry,
  NativeReactModuleRegistryError,
  type NativeReactCatalogModuleLock
} from '../../index';

const hostReact = {
  default: null as unknown,
  createElement: vi.fn(),
  useState: vi.fn((initial: unknown) => [initial, vi.fn()])
};
hostReact.default = hostReact;

const hostModules = {
  react: hostReact,
  'react/jsx-runtime': { jsx: vi.fn(), jsxs: vi.fn() },
  antd: {},
  '@1flowbase/ui': {}
};

describe('Native React Host Module Registry', () => {
  test('D2-AC-003 single-flights the same identity, resolves different exports, and uses the Host React Hooks singleton', async () => {
    const source = `
import React from 'react';
export function Counter() { return React.useState(7)[0]; }
export const importedReact = React;
`;
    const registration = await lock(source, ['Counter', 'importedReact']);
    const fetchAsset = vi.fn(async () => response(source));
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [registration],
      hostModules,
      fetchAsset
    });

    const [first, second, moduleMap] = await Promise.all([
      registry.load(registration.module_source),
      registry.load(registration.module_source),
      registry.resolveModuleMap([registration.module_source])
    ]);

    expect(fetchAsset).toHaveBeenCalledTimes(1);
    expect(first).toBe(second);
    expect(moduleMap[registration.module_source]).toBe(first);
    expect(first.importedReact).toBe(hostReact);
    expect((first.Counter as () => unknown)()).toBe(7);
    expect(hostReact.useState).toHaveBeenCalledWith(7);
  });

  test.each([
    {
      name: 'missing export',
      source: 'export const Present = 1;',
      exports: ['Missing'],
      expectedCode: 'module_export_missing'
    },
    {
      name: 'illegal dependency',
      source: "import value from 'react-copy'; export { value };",
      exports: ['value'],
      expectedCode: 'module_dependency_denied'
    }
  ])(
    'D2-AC-004 fails closed for $name',
    async ({ source, exports, expectedCode }) => {
      const registration = await lock(source, exports);
      const registry = createNativeReactModuleRegistry({
        dependencyLock: [registration],
        hostModules,
        fetchAsset: async () => response(source)
      });

      await expect(
        registry.load(registration.module_source)
      ).rejects.toMatchObject({
        code: expectedCode
      });
    }
  );

  test('D2-AC-004 rejects unregistered modules and a Catalog-provided second React', async () => {
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [],
      hostModules,
      fetchAsset: async () => response('')
    });
    await expect(registry.load('missing')).rejects.toMatchObject({
      code: 'module_not_registered'
    });

    expect(() =>
      createNativeReactModuleRegistry({
        dependencyLock: [
          {
            module_source: 'react',
            module_version: '99.0.0',
            browser_asset: { sha256: '0'.repeat(64), url: '/react.js' },
            exports: ['default']
          }
        ],
        hostModules,
        fetchAsset: async () => response('')
      })
    ).toThrowError(
      expect.objectContaining({ code: 'invalid_dependency_lock' })
    );
  });

  test('D2-AC-004 rejects cross-origin assets and registered dependency cycles', async () => {
    const surfaceSource = 'export const Surface = 1;';
    const surface = await lock(surfaceSource, ['Surface']);
    expect(() =>
      createNativeReactModuleRegistry({
        dependencyLock: [
          {
            ...surface,
            browser_asset: {
              ...surface.browser_asset,
              url: `https://plugins.example/${surface.browser_asset.sha256}`
            }
          }
        ],
        hostModules,
        fetchAsset: async () => response(surfaceSource)
      })
    ).toThrowError(
      expect.objectContaining({ code: 'invalid_dependency_lock' })
    );

    const sourceA =
      "import { valueB } from '@catalog/b'; export const valueA = valueB;";
    const sourceB =
      "import { valueA } from '@catalog/a'; export const valueB = valueA;";
    const moduleA = await lock(sourceA, ['valueA'], '@catalog/a');
    const moduleB = await lock(sourceB, ['valueB'], '@catalog/b');
    const sources = new Map([
      [moduleA.browser_asset.url, sourceA],
      [moduleB.browser_asset.url, sourceB]
    ]);
    const cyclic = createNativeReactModuleRegistry({
      dependencyLock: [moduleA, moduleB],
      hostModules,
      fetchAsset: async (url) => response(sources.get(String(url)) ?? '')
    });

    await expect(
      Promise.all([
        cyclic.load(moduleA.module_source),
        cyclic.load(moduleB.module_source)
      ])
    ).rejects.toMatchObject({ code: 'module_dependency_cycle' });
  });

  test('D2-AC-004 returns stable failures for an HTTP error and invalid ESM', async () => {
    const validSource = 'export const Surface = 1;';
    const valid = await lock(validSource, ['Surface']);
    const responseFailure = createNativeReactModuleRegistry({
      dependencyLock: [valid],
      hostModules,
      fetchAsset: async () => new Response('', { status: 503 })
    });
    await expect(
      responseFailure.load(valid.module_source)
    ).rejects.toMatchObject({ code: 'module_fetch_failed' });

    const invalidSource = 'export const =;';
    const invalid = await lock(invalidSource, ['Surface']);
    const invalidModule = createNativeReactModuleRegistry({
      dependencyLock: [invalid],
      hostModules,
      fetchAsset: async () => response(invalidSource)
    });
    await expect(
      invalidModule.load(invalid.module_source)
    ).rejects.toMatchObject({ code: 'module_invalid' });
  });

  test('D2-AC-004 keeps digest mismatch and a late failed fetch as stable single-flight failures', async () => {
    const source = 'export const Surface = 1;';
    const registration = await lock(source, ['Surface']);
    const mismatch = createNativeReactModuleRegistry({
      dependencyLock: [
        {
          ...registration,
          browser_asset: {
            sha256: '0'.repeat(64),
            url: `/api/console/frontstage/workspace-1/component-module-assets/${'0'.repeat(64)}`
          }
        }
      ],
      hostModules,
      fetchAsset: async () => response(source)
    });
    await expect(
      mismatch.load(registration.module_source)
    ).rejects.toMatchObject({
      code: 'module_digest_mismatch'
    });

    let rejectFetch!: (reason: Error) => void;
    const fetchAsset = vi.fn(
      () =>
        new Promise<Response>((_resolve, reject) => {
          rejectFetch = reject;
        })
    );
    const lateFailure = createNativeReactModuleRegistry({
      dependencyLock: [registration],
      hostModules,
      fetchAsset
    });
    const first = lateFailure.load(registration.module_source);
    const second = lateFailure.load(registration.module_source);
    rejectFetch(new Error('late network failure'));

    await expect(first).rejects.toBeInstanceOf(NativeReactModuleRegistryError);
    await expect(second).rejects.toMatchObject({ code: 'module_fetch_failed' });
    expect(fetchAsset).toHaveBeenCalledTimes(1);
  });
});

async function lock(
  source: string,
  exports: string[],
  moduleSource = '@1flowbase/native-components'
): Promise<NativeReactCatalogModuleLock> {
  const bytes = new TextEncoder().encode(source);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  const sha256 = [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  return {
    module_source: moduleSource,
    module_version: '1.0.0',
    browser_asset: {
      sha256,
      url: `/api/console/frontstage/workspace-1/component-module-assets/${sha256}`
    },
    exports
  };
}

function response(source: string): Response {
  return new Response(source, { status: 200 });
}
