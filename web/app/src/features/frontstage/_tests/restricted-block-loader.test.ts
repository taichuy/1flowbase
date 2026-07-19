import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageBlockInstance } from '../lib/page-document';
import {
  createRestrictedBlockRunPlan,
  type RestrictedBlockLoaderLimits
} from '../lib/restricted-block-loader';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero-block',
    sourceId: 'hero-block',
    codeRef: 'hero-code',
    sourceCodeRef: 'hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'hero.banner'
    },
    props: { title: 'Hello' },
    layout: { order: 10 },
    order: 10,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides,
    rendererVersion: overrides.rendererVersion ?? 'v1'
  };
}

function createCatalogEntry(
  overrides: Partial<NormalizedFrontstageBlockCatalogEntry> = {}
): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: 'official:hero.banner',
    runtimeKind: 'iframe',
    installationId: 'installation-1',
    providerCode: 'official',
    pluginId: 'official.blocks',
    pluginVersion: '1.0.0',
    contributionCode: 'hero.banner',
    title: 'Hero Banner',
    entry: 'blocks/hero/index.js',
    permissions: {
      network: 'none',
      storage: 'none',
      secrets: 'none'
    },
    contextContract: {
      primitives: ['text', 'button', 'data_record'],
      inputSchema: { type: 'object' }
    },
    uiCapabilities: ['responsive', 'data_binding'],
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw'],
    ...overrides
  };
}

function createLimits(
  overrides: Partial<RestrictedBlockLoaderLimits> = {}
): RestrictedBlockLoaderLimits {
  return {
    timeoutMs: 1000,
    maxRenderDepth: 8,
    maxRenderNodes: 250,
    allowedActions: ['record.save'],
    allowedEvents: ['record.saved'],
    allowedQueries: ['records'],
    allowedDataOperations: ['query'],
    maxEventChainDepth: 4,
    ...overrides
  };
}

describe('restricted block loader core', () => {
  test('builds a stable run request with schema validation options and mediator policy', () => {
    const result = createRestrictedBlockRunPlan({
      block: createBlock(),
      catalogEntry: createCatalogEntry({
        codeCapabilities: {
          template: null,
          allowedImports: [
            '@1flowbase/block-sdk',
            '@1flowbase/block-renderer/antd-facade'
          ],
          monacoExtraLibs: [],
          workerModuleSources: []
        }
      }),
      code: 'export default { render() {} }',
      contextSnapshot: {
        workspaceId: 'workspace-1',
        pageId: 'page-1'
      },
      props: { title: 'Override' },
      state: { selected: true },
      limits: createLimits()
    });

    expect(result).toEqual({
      ok: true,
      request: {
        requestId: 'restricted-block:hero-block:hero-code',
        blockId: 'hero-block',
        source: 'export default { render() {} }',
        props: { title: 'Override' },
        state: { selected: true },
        contextSnapshot: {
          workspaceId: 'workspace-1',
          pageId: 'page-1'
        },
        limits: {
          timeoutMs: 1000,
          maxRenderDepth: 8,
          maxRenderNodes: 250
        },
        allowedImports: [
          '@1flowbase/block-sdk',
          '@1flowbase/block-renderer/antd-facade'
        ]
      },
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],
        allowedActions: ['record.save'],
        allowedEvents: []
      },
      mediatorPolicy: {
        allowedEvents: [],
        allowedActions: ['record.save'],
        allowedQueries: ['records'],
        allowedDataOperations: ['query'],
        maxEventChainDepth: 4
      }
    });
  });

  test('falls back to block props and empty state without mutating caller objects', () => {
    const block = createBlock({ props: { title: 'From block' } });
    const contextSnapshot = { pageId: 'page-1' };
    const result = createRestrictedBlockRunPlan({
      block,
      catalogEntry: createCatalogEntry(),
      code: 'export default {}',
      contextSnapshot,
      limits: createLimits({ allowedActions: [], allowedEvents: [] })
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }
    expect(result.request.props).toEqual({ title: 'From block' });
    expect(result.request.props).not.toBe(block.props);
    expect(result.request.state).toEqual({});
    expect(result.request.contextSnapshot).toEqual({ pageId: 'page-1' });
    expect(result.request.contextSnapshot).not.toBe(contextSnapshot);
  });

  test.each([
    [
      'catalog_mismatch',
      {
        block: createBlock({
          catalog: { providerCode: 'other', installationId: 'installation-1' }
        }),
        catalogEntry: createCatalogEntry()
      },
      'block.catalog.providerCode'
    ],
    [
      'unsupported_runtime',
      {
        block: createBlock({
          runtime: { kind: 'worker', entry: null, hint: 'worker' }
        }),
        catalogEntry: createCatalogEntry()
      },
      'block.runtime.kind'
    ],
    [
      'missing_code_ref',
      {
        block: createBlock({ codeRef: '' }),
        catalogEntry: createCatalogEntry()
      },
      'block.codeRef'
    ],
    [
      'missing_code_ref',
      {
        block: createBlock({
          codeRef: 'fallback-code-ref',
          sourceCodeRef: null
        }),
        catalogEntry: createCatalogEntry()
      },
      'block.codeRef'
    ],
    [
      'missing_code',
      {
        block: createBlock(),
        catalogEntry: createCatalogEntry(),
        code: '   '
      },
      'code'
    ],
    [
      'missing_limits',
      {
        block: createBlock(),
        catalogEntry: createCatalogEntry(),
        limits: undefined
      },
      'limits'
    ]
  ] as const)(
    'returns a structured %s rejection',
    (code, overrides, expectedPath) => {
      const baseInput = {
        block: createBlock(),
        catalogEntry: createCatalogEntry(),
        code: 'export default {}',
        contextSnapshot: {},
        limits: createLimits()
      };
      const result = createRestrictedBlockRunPlan({
        ...baseInput,
        ...overrides
      });

      expect(result).toMatchObject({
        ok: false,
        code,
        path: expectedPath,
        blockId: 'hero-block'
      });
    }
  );
});
