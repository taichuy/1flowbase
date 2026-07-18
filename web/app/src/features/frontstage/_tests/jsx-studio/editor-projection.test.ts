import type { ConsoleFrontstageDataCapabilities } from '@1flowbase/api-client';
import { describe, expect, test } from 'vitest';

import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import {
  createFrontstageJsxEditorProjection,
  createFrontstageJsxBindingSnippet
} from '../../lib/jsx-studio/editor-projection';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const dataCapabilities: ConsoleFrontstageDataCapabilities = {
  queries: [
    {
      id: 'frontstage.data_model.record.list',
      kind: 'query',
      params_schema: {
        type: 'object',
        required: ['model'],
        properties: {
          model: { type: 'string' },
          page: { type: 'integer' }
        }
      },
      result_schema: {
        type: 'object',
        properties: {
          items: { type: 'array', items: { type: 'object' } },
          total: { type: 'integer' }
        }
      }
    },
    {
      id: 'frontstage.data_model.record.get',
      kind: 'query',
      params_schema: { type: 'object' },
      result_schema: { type: 'object' }
    }
  ],
  actions: [
    {
      id: 'frontstage.data_model.record.create',
      kind: 'action',
      params_schema: {
        type: 'object',
        required: ['model', 'values'],
        properties: {
          model: { type: 'string' },
          values: { type: 'object' }
        }
      },
      result_schema: {
        type: 'object',
        properties: { record: { type: 'object' } }
      }
    }
  ],
  models: [
    {
      code: 'orders',
      scope_kind: 'workspace',
      fields: [
        {
          code: 'id',
          title: 'ID',
          field_kind: 'string',
          is_required: true,
          is_writable: false
        },
        {
          code: 'total',
          title: 'Total',
          field_kind: 'number',
          is_required: false,
          is_writable: true
        }
      ]
    }
  ]
};

function createBlock(): FrontstageBlockInstance {
  return {
    id: 'orders-block',
    rendererVersion: 'v1',
    sourceId: 'orders-block',
    codeRef: 'orders-code',
    sourceCodeRef: 'orders-code',
    catalog: {
      providerCode: '1flowbase',
      installationId: 'builtin-installation'
    },
    contribution: {
      pluginId: 'builtin-frontstage',
      pluginVersion: '1.0.0',
      code: 'frontstage.js-ui-block'
    },
    props: {
      dataBinding: [
        {
          key: 'ordersList',
          id: 'frontstage.data_model.record.list',
          kind: 'query',
          params: { model: 'orders' }
        },
        {
          key: 'createOrder',
          id: 'frontstage.data_model.record.create',
          kind: 'action',
          params: { model: 'orders' }
        }
      ]
    },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: 'iframe',
      entry: 'index.js',
      hint: 'iframe'
    }
  };
}

function createCatalogEntry(): NormalizedFrontstageBlockCatalogEntry {
  return {
    id: '1flowbase:frontstage.js-ui-block',
    runtimeKind: 'iframe',
    installationId: 'builtin-installation',
    providerCode: '1flowbase',
    pluginId: 'builtin-frontstage',
    pluginVersion: '1.0.0',
    contributionCode: 'frontstage.js-ui-block',
    title: 'JSX 区块',
    entry: 'index.js',
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    contextContract: { primitives: [], inputSchema: {} },
    uiCapabilities: ['configurable', 'data_binding'],
    codeCapabilities: {
      template: null,
      allowedImports: ['@1flowbase/block-renderer/antd-facade'],
      monacoExtraLibs: [
        {
          filePath: 'file:///node_modules/antd-facade/index.d.ts',
          content: [
            "declare module '@1flowbase/block-renderer/antd-facade' {",
            '  export const Stack: unknown;',
            '  export const Table: unknown;',
            '}'
          ].join('\n')
        }
      ],
      workerModuleSources: ['@1flowbase/block-renderer/antd-facade']
    },
    raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
  };
}

describe('frontstage JSX Studio editor projection', () => {
  test('projects only bound capabilities and catalog components into visible Monaco context', () => {
    const projection = createFrontstageJsxEditorProjection({
      block: createBlock(),
      catalogEntry: createCatalogEntry(),
      dataCapabilities
    });

    expect(projection.components).toEqual(['Stack', 'Table']);
    expect(projection.prelude).toContain('ordersList');
    expect(projection.prelude).toContain('createOrder');
    expect(projection.prelude).toContain('ctx.data.query');
    expect(projection.prelude).toContain('ctx.actions.invoke');
    expect(projection.prelude).not.toContain(
      'frontstage.data_model.record.get'
    );

    const generatedTypes = projection.monacoExtraLibs
      .map((extraLib) => extraLib.content)
      .join('\n');
    expect(generatedTypes).toContain('interface OrdersRecord');
    expect(generatedTypes).toContain('id: string');
    expect(generatedTypes).toContain('total?: number');
    expect(generatedTypes).toContain(
      "queryId: 'frontstage.data_model.record.list'"
    );
    expect(generatedTypes).toContain(
      "actionId: 'frontstage.data_model.record.create'"
    );
  });

  test('creates executable snippets with the bound model fixed in params', () => {
    const [queryBinding, actionBinding] =
      createFrontstageJsxEditorProjection({
        block: createBlock(),
        catalogEntry: createCatalogEntry(),
        dataCapabilities
      }).bindings;

    expect(createFrontstageJsxBindingSnippet(queryBinding)).toContain(
      "await ctx.data.query('frontstage.data_model.record.list', { model: 'orders'"
    );
    expect(createFrontstageJsxBindingSnippet(actionBinding)).toContain(
      "await ctx.actions.invoke('frontstage.data_model.record.create', { model: 'orders'"
    );
  });
});
