import { describe, expect, test } from 'vitest';

import {
  readFrontstageBlockDataBindings,
  writeFrontstageBlockDataBindings
} from '../../lib/jsx-studio/block-data-binding';
import type { FrontstageBlockInstance } from '../../lib/page-document';

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
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: 'native_react',
      entry: 'index.js',
      hint: 'native_react'
    }
  };
}

describe('frontstage JSX Studio block data binding', () => {
  test('reads persisted query and action bindings', () => {
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
