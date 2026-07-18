import { describe, expect, test } from 'vitest';

import {
  createFrontstageBlockBindingRuntimeLimits,
  readFrontstageBlockDataBindings,
  writeFrontstageBlockDataBindings
} from '../../lib/jsx-studio/block-data-binding';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { RestrictedBlockLoaderLimits } from '../../lib/restricted-block-loader';

function createBlock(): FrontstageBlockInstance {
  return {
    id: 'orders-block',
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
      title: 'Orders',
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
        },
        {
          key: '',
          id: 'frontstage.data_model.record.delete',
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

const baseLimits: RestrictedBlockLoaderLimits = {
  timeoutMs: 1000,
  maxRenderDepth: 8,
  maxRenderNodes: 250,
  maxEventChainDepth: 4,
  allowedQueries: ['stale.query'],
  allowedActions: ['stale.action'],
  allowedEvents: [],
  allowedDataOperations: []
};

describe('frontstage JSX Studio block data binding', () => {
  test('uses persisted bindings as the runtime query/action allowlist', () => {
    const block = createBlock();

    expect(readFrontstageBlockDataBindings(block.props)).toEqual([
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
    ]);

    expect(
      createFrontstageBlockBindingRuntimeLimits(block, baseLimits)
    ).toMatchObject({
      timeoutMs: 1000,
      allowedQueries: ['frontstage.data_model.record.list'],
      allowedActions: ['frontstage.data_model.record.create']
    });
  });

  test('updates dataBinding without replacing unrelated block props', () => {
    const block = createBlock();
    const nextBlock = writeFrontstageBlockDataBindings(block, [
      {
        key: 'orderDetail',
        id: 'frontstage.data_model.record.get',
        kind: 'query',
        params: { model: 'orders' }
      }
    ]);

    expect(nextBlock).not.toBe(block);
    expect(nextBlock.props).toMatchObject({
      title: 'Orders',
      dataBinding: [
        {
          key: 'orderDetail',
          id: 'frontstage.data_model.record.get',
          kind: 'query',
          params: { model: 'orders' }
        }
      ]
    });
  });
});
