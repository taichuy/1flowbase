import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  createFrontstageBlockConfigurationModel,
  FRONTSTAGE_BLOCK_CONFIGURATION_SECTION_IDS
} from '../../lib/block-configuration';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { RestrictedBlockLoaderLimits } from '../../lib/restricted-block-loader';

function createBlock(
  overrides: Partial<FrontstageBlockInstance> = {}
): FrontstageBlockInstance {
  return {
    id: 'hero-block',
    sourceId: 'source-hero-block',
    codeRef: 'hero-code',
    sourceCodeRef: 'source-hero-code',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'hero.banner'
    },
    props: {
      title: 'Configured title',
      description: 'Configured description',
      templateId: 'data-table',
      data: {
        model: 'orders',
        fields: ['id', { name: 'title', label: 'Title' }],
        operations: {
          query: true,
          create: false,
          update: true,
          delete: false
        },
        filter: { status: 'open' },
        sort: [{ field: 'created_at', direction: 'desc' }],
        pagination: { pageSize: 20 },
        customDataKey: { enabled: true }
      },
      customUnknownProp: { keep: ['as-is'] }
    },
    layout: {
      order: 7,
      width: 6,
      height: 4,
      region: 'main'
    },
    order: 7,
    runtime: {
      kind: 'iframe',
      entry: 'blocks/hero/index.js',
      hint: 'iframe'
    },
    ...overrides
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
      inputSchema: {
        type: 'object',
        properties: {
          recordId: { type: 'string' }
        }
      }
    },
    uiCapabilities: ['responsive', 'configurable', 'data_binding'],
    raw: {
      installation_id: 'installation-1',
      provider_code: 'official',
      plugin_id: 'official.blocks',
      plugin_version: '1.0.0',
      contribution_code: 'hero.banner',
      title: 'Hero Banner',
      runtime: 'iframe',
      entry: 'blocks/hero/index.js',
      context_contract: {
        primitives: ['text', 'button', 'data_record'],
        input_schema: {
          type: 'object'
        }
      },
      permissions: {
        network: 'none',
        storage: 'none',
        secrets: 'none'
      },
      ui_capabilities: ['responsive', 'configurable', 'data_binding']
    },
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
    maxEventChainDepth: 4,
    allowedActions: ['orders.refresh'],
    allowedEvents: ['orders.loaded'],
    allowedQueries: ['orders'],
    allowedDataOperations: ['query', 'update'],
    ...overrides
  };
}

function section(
  model: ReturnType<typeof createFrontstageBlockConfigurationModel>,
  id: (typeof FRONTSTAGE_BLOCK_CONFIGURATION_SECTION_IDS)[number]
) {
  const result = model.sections.find((item) => item.id === id);
  expect(result).toMatchObject({ id });
  return result;
}

describe('frontstage block configuration model', () => {
  test('builds sections in the stable panel order', () => {
    const model = createFrontstageBlockConfigurationModel({
      block: createBlock(),
      catalogEntry: createCatalogEntry(),
      limits: createLimits()
    });

    expect(FRONTSTAGE_BLOCK_CONFIGURATION_SECTION_IDS).toEqual([
      'basic',
      'data',
      'code',
      'context',
      'limits'
    ]);
    expect(model.sections.map((item) => item.id)).toEqual([
      'basic',
      'data',
      'code',
      'context',
      'limits'
    ]);
  });

  test('creates a complete model from block, catalog entry, and restricted limits', () => {
    const model = createFrontstageBlockConfigurationModel({
      block: createBlock(),
      catalogEntry: createCatalogEntry(),
      limits: createLimits()
    });

    expect(section(model, 'basic')?.model).toEqual({
      blockId: 'hero-block',
      sourceId: 'source-hero-block',
      title: {
        value: 'Configured title',
        placeholder: 'Hero Banner'
      },
      description: {
        value: 'Configured description',
        placeholder: 'Describe what this block renders.'
      },
      width: 6,
      height: 4,
      order: 7,
      codeRef: 'hero-code',
      sourceCodeRef: 'source-hero-code',
      rawProps: expect.objectContaining({
        customUnknownProp: { keep: ['as-is'] }
      }),
      rawLayout: expect.objectContaining({
        region: 'main',
        width: 6,
        height: 4,
        order: 7
      })
    });

    expect(section(model, 'data')?.model).toEqual({
      model: 'orders',
      fields: ['id', { name: 'title', label: 'Title' }],
      operations: {
        query: { enabled: true },
        create: { enabled: false },
        update: { enabled: true },
        delete: { enabled: false }
      },
      filter: { status: 'open' },
      sort: [{ field: 'created_at', direction: 'desc' }],
      pagination: { pageSize: 20 },
      rawConfig: expect.objectContaining({
        customDataKey: { enabled: true }
      })
    });

    expect(section(model, 'code')?.model).toEqual({
      codeRef: 'hero-code',
      sourceCodeRef: 'source-hero-code',
      runtime: {
        kind: 'iframe',
        entry: 'blocks/hero/index.js',
        hint: 'iframe'
      },
      contribution: {
        pluginId: 'official.blocks',
        pluginVersion: '1.0.0',
        code: 'hero.banner',
        catalogId: 'official:hero.banner',
        catalogTitle: 'Hero Banner',
        providerCode: 'official',
        installationId: 'installation-1'
      },
      template: {
        id: 'data-table',
        raw: 'data-table'
      }
    });

    expect(section(model, 'context')?.model).toEqual({
      catalog: {
        available: true,
        primitives: ['text', 'button', 'data_record'],
        inputSchema: {
          type: 'object',
          properties: {
            recordId: { type: 'string' }
          }
        }
      },
      ctx: {
        currentUser: { path: 'ctx.currentUser', available: true },
        page: { path: 'ctx.page', available: true },
        params: { path: 'ctx.params', available: true },
        props: { path: 'ctx.props', available: true },
        state: { path: 'ctx.state', available: true },
        data: {
          path: 'ctx.data',
          available: true,
          models: ['orders'],
          operations: ['query', 'update']
        },
        actions: {
          path: 'ctx.actions',
          available: true,
          allowed: ['orders.refresh']
        },
        events: {
          path: 'ctx.events',
          available: true,
          allowed: ['orders.loaded']
        }
      }
    });

    expect(section(model, 'limits')?.model).toEqual({
      timeoutMs: 1000,
      maxRenderDepth: 8,
      maxRenderNodes: 250,
      maxEventChainDepth: 4,
      allowedActions: ['orders.refresh'],
      allowedEvents: ['orders.loaded'],
      allowedQueries: ['orders'],
      allowedDataOperations: ['query', 'update']
    });
  });

  test('provides data defaults and stable catalog fallback without throwing on unknown props', () => {
    const model = createFrontstageBlockConfigurationModel({
      block: createBlock({
        props: {
          data: 'unexpected-data-shape',
          customUnknownProp: { nested: ['kept'] }
        }
      }),
      catalogEntry: null,
      limits: { timeoutMs: 500 }
    });

    expect(section(model, 'data')?.model).toEqual({
      model: null,
      fields: [],
      operations: {
        query: { enabled: false },
        create: { enabled: false },
        update: { enabled: false },
        delete: { enabled: false }
      },
      filter: null,
      sort: null,
      pagination: {
        current: null,
        pageSize: null
      },
      rawConfig: 'unexpected-data-shape'
    });
    expect(section(model, 'context')?.model).toMatchObject({
      catalog: {
        available: false,
        primitives: [],
        inputSchema: {}
      }
    });
    expect(section(model, 'basic')?.model).toMatchObject({
      title: {
        value: null,
        placeholder: 'Untitled block'
      }
    });
  });

  test('does not mutate or reuse caller input objects and arrays', () => {
    const block = createBlock();
    const catalogEntry = createCatalogEntry();
    const limits = createLimits();
    const originalBlock = structuredClone(block);
    const originalCatalogEntry = structuredClone(catalogEntry);
    const originalLimits = structuredClone(limits);

    const model = createFrontstageBlockConfigurationModel({
      block,
      catalogEntry,
      limits
    });

    expect(block).toEqual(originalBlock);
    expect(catalogEntry).toEqual(originalCatalogEntry);
    expect(limits).toEqual(originalLimits);

    const dataModel = section(model, 'data')?.model as {
      fields: unknown[];
      sort: unknown[];
      rawConfig: { fields: unknown[] };
    };
    const contextModel = section(model, 'context')?.model as {
      catalog: {
        primitives: unknown[];
        inputSchema: Record<string, unknown>;
      };
      ctx: {
        data: {
          models: unknown[];
          operations: unknown[];
        };
      };
    };
    const limitsModel = section(model, 'limits')?.model as {
      allowedActions: unknown[];
      allowedEvents: unknown[];
      allowedQueries: unknown[];
      allowedDataOperations: unknown[];
    };

    expect(dataModel.fields).not.toBe(
      (block.props.data as { fields: unknown[] }).fields
    );
    expect(dataModel.sort).not.toBe(
      (block.props.data as { sort: unknown[] }).sort
    );
    expect(dataModel.rawConfig.fields).not.toBe(
      (block.props.data as { fields: unknown[] }).fields
    );
    expect(contextModel.catalog.primitives).not.toBe(
      catalogEntry.contextContract.primitives
    );
    expect(contextModel.catalog.inputSchema).not.toBe(
      catalogEntry.contextContract.inputSchema
    );
    expect(contextModel.ctx.data.models).not.toBe(limits.allowedQueries);
    expect(contextModel.ctx.data.operations).not.toBe(
      limits.allowedDataOperations
    );
    expect(limitsModel.allowedActions).not.toBe(limits.allowedActions);
    expect(limitsModel.allowedEvents).not.toBe(limits.allowedEvents);
    expect(limitsModel.allowedQueries).not.toBe(limits.allowedQueries);
    expect(limitsModel.allowedDataOperations).not.toBe(
      limits.allowedDataOperations
    );
  });
});
