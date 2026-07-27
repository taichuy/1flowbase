import { describe, expect, test, vi } from 'vitest';

import {
  createNativeReactModuleRegistry,
  NativeReactModuleRegistryError,
  type NativeReactCatalogModuleLock,
  type NativeReactModuleAssetLock
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
  antd: { Button: vi.fn() }
};

describe('Native React Host Module Registry', () => {
  test('single-flights fetched modules and uses the registered Host React singleton', async () => {
    const source = `
import React from 'react';
export function Counter() { return React.useState(7)[0]; }
export const importedReact = React;
`;
    const registration = await fetchedLock(source, [
      'Counter',
      'importedReact'
    ]);
    const fetchAsset = vi.fn(async () => response(source));
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [
        hostLock('react', ['default', 'useState']),
        registration
      ],
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
  });

  test('resolves Host modules only when their version and exports are locked', async () => {
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [hostLock('react', ['default', 'useState'])],
      hostModules
    });
    await expect(registry.load('react')).resolves.toBe(hostReact);
    expect(() =>
      createNativeReactModuleRegistry({
        dependencyLock: [hostLock('react', ['missing'])],
        hostModules
      })
    ).toThrowError(
      expect.objectContaining({ code: 'invalid_dependency_lock' })
    );
  });

  test('single-flights and verifies ShadowRoot style/support assets', async () => {
    const source = 'export const Surface = 1;';
    const style = '.surface { color: red; }';
    const support = 'support-bytes';
    const registration = await fetchedLock(source, ['Surface'], undefined, [
      await assetLock('shadow_style', style, 'text/css; charset=utf-8'),
      await assetLock('support', support, 'application/octet-stream')
    ]);
    const bodies = new Map(
      registration.assets.map((asset) => [
        asset.url,
        {
          source:
            asset.role === 'browser_module'
              ? source
              : asset.role === 'shadow_style'
                ? style
                : support,
          mediaType: asset.media_type
        }
      ])
    );
    const fetchAsset = vi.fn(async (url) => {
      const body = bodies.get(String(url))!;
      return response(body.source, body.mediaType);
    });
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [registration],
      hostModules,
      fetchAsset
    });

    const [first, second] = await Promise.all([
      registry.resolveModuleAssets([registration.module_source]),
      registry.resolveModuleAssets([registration.module_source])
    ]);

    expect(first.map((asset) => asset.role)).toEqual([
      'shadow_style',
      'support'
    ]);
    expect(second[0]?.bytes).toBe(first[0]?.bytes);
    expect(fetchAsset).toHaveBeenCalledTimes(2);
  });

  test.each([
    {
      name: 'missing export',
      source: 'export const Present = 1;',
      exports: ['Missing'],
      expectedCode: 'module_export_missing'
    },
    {
      name: 'unregistered dependency',
      source: "import value from 'react-copy'; export { value };",
      exports: ['value'],
      expectedCode: 'module_dependency_denied'
    }
  ])('fails closed for $name', async ({ source, exports, expectedCode }) => {
    const registration = await fetchedLock(source, exports);
    const registry = createNativeReactModuleRegistry({
      dependencyLock: [registration],
      hostModules,
      fetchAsset: async () => response(source)
    });
    await expect(
      registry.load(registration.module_source)
    ).rejects.toMatchObject({ code: expectedCode });
  });

  test('rejects invalid Host/fetched bindings and cross-origin assets', async () => {
    const registration = await fetchedLock('export const Surface = 1;', [
      'Surface'
    ]);
    expect(() =>
      createNativeReactModuleRegistry({
        dependencyLock: [
          {
            ...registration,
            assets: registration.assets.map((asset) => ({
              ...asset,
              url: `https://plugins.example/${asset.sha256}`
            }))
          }
        ],
        hostModules
      })
    ).toThrowError(
      expect.objectContaining({ code: 'invalid_dependency_lock' })
    );
    expect(() =>
      createNativeReactModuleRegistry({
        dependencyLock: [
          {
            ...registration,
            module_source: 'react',
            binding: 'fetched'
          }
        ],
        hostModules
      })
    ).toThrowError(
      expect.objectContaining({ code: 'invalid_dependency_lock' })
    );
  });

  test('keeps digest mismatch and a late failed fetch stable', async () => {
    const source = 'export const Surface = 1;';
    const registration = await fetchedLock(source, ['Surface']);
    const browser = registration.assets[0]!;
    const mismatch = createNativeReactModuleRegistry({
      dependencyLock: [
        {
          ...registration,
          assets: [
            {
              ...browser,
              sha256: '0'.repeat(64),
              url: assetUrl('0'.repeat(64))
            }
          ]
        }
      ],
      hostModules,
      fetchAsset: async () => response(source)
    });
    await expect(
      mismatch.load(registration.module_source)
    ).rejects.toMatchObject({ code: 'module_digest_mismatch' });

    const mediaTypeMismatch = createNativeReactModuleRegistry({
      dependencyLock: [registration],
      hostModules,
      fetchAsset: async () => response(source, 'text/css')
    });
    await expect(
      mediaTypeMismatch.load(registration.module_source)
    ).rejects.toMatchObject({ code: 'module_media_type_mismatch' });

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

function hostLock(
  moduleSource: 'react' | 'react/jsx-runtime' | 'antd',
  exports: string[]
): NativeReactCatalogModuleLock {
  return {
    module_source: moduleSource,
    module_version: '1.0.0',
    binding: 'host',
    assets: [],
    exports
  };
}

async function fetchedLock(
  source: string,
  exports: string[],
  moduleSource = '@1flowbase/native-components',
  extraAssets: NativeReactModuleAssetLock[] = []
): Promise<NativeReactCatalogModuleLock> {
  return {
    module_source: moduleSource,
    module_version: '1.0.0',
    binding: 'fetched',
    assets: [
      await assetLock(
        'browser_module',
        source,
        'text/javascript; charset=utf-8'
      ),
      ...extraAssets
    ],
    exports
  };
}

async function assetLock(
  role: NativeReactModuleAssetLock['role'],
  source: string,
  mediaType: string
): Promise<NativeReactModuleAssetLock> {
  const bytes = new TextEncoder().encode(source);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  const sha256 = [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  return { role, media_type: mediaType, sha256, url: assetUrl(sha256) };
}

function assetUrl(sha256: string) {
  return `/api/console/frontstage/workspace-1/component-module-assets/${sha256}`;
}

function response(
  source: string,
  mediaType = 'text/javascript; charset=utf-8'
): Response {
  return new Response(source, {
    status: 200,
    headers: { 'content-type': mediaType }
  });
}
