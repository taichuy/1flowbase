import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../../lib/page-document';
import { resolveFrontstageBlockNativeDependencyLock } from '../block-catalog-helpers';

function catalogEntry(
  contributionCode: string,
  moduleSource: string
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: contributionCode,
    providerCode: 'provider',
    installationId: 'installation',
    pluginId: 'plugin',
    pluginVersion: '1.0.0',
    contributionCode,
    title: contributionCode,
    runtimeKind: 'native_react',
    entry: 'index.js',
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    contextContract: { primitives: [], inputSchema: {} },
    uiCapabilities: [],
    codeCapabilities: undefined,
    codeModules: [
      {
        source: moduleSource,
        version: '1.0.0',
        binding: 'fetched',
        assets: [
          {
            role: 'browser_module',
            media_type: 'text/javascript; charset=utf-8',
            sha256:
              'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            url: '/fixture-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
          }
        ],
        exports: ['default'],
        type_declarations: ''
      }
    ],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
  };
}

function block(id: string, contributionCode: string): FrontstageBlockInstance {
  return {
    id,
    rendererVersion: 'v1',
    sourceId: null,
    codeRef: `${id}-code`,
    sourceCodeRef: null,
    catalog: {
      providerCode: 'provider',
      installationId: 'installation'
    },
    contribution: {
      pluginId: 'plugin',
      pluginVersion: '1.0.0',
      code: contributionCode
    },
    props: {},
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0, region: 'main' },
    order: 0,
    runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
  };
}

describe('resolveFrontstageBlockNativeDependencyLock', () => {
  test('resolves dependencies from the active child instead of the initial root', () => {
    const rootEntry = catalogEntry('root', '@example/root-module');
    const childEntry = catalogEntry('child', '@example/child-module');

    const resolution = resolveFrontstageBlockNativeDependencyLock(
      block('child-block', 'child'),
      [rootEntry, childEntry],
      'workspace-1'
    );

    expect(resolution.error).toBeNull();
    expect(resolution.dependencyLock).toEqual([
      expect.objectContaining({ module_source: '@example/child-module' })
    ]);
  });
});
