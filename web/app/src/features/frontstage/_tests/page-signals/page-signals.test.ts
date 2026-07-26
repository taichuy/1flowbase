import { describe, expect, test } from 'vitest';

import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstageSignalGraph } from '../../lib/page-signals/graph';
import {
  commitFrontstageBlockOutputs,
  createFrontstageSignalSnapshot,
  readFrontstageSignal
} from '../../lib/page-signals/store';

function block(
  id: string,
  ports: FrontstageBlockInstance['ports']
): FrontstageBlockInstance {
  return { id, ports } as FrontstageBlockInstance;
}

const totalSchema = { type: 'integer' };

describe('Frontstage page signals', () => {
  test('AC-006 builds a typed producer-to-consumer order', () => {
    const producer = block('producer', {
      inputs: [],
      outputs: [{ name: 'total', schema: totalSchema }]
    });
    const consumer = block('consumer', {
      inputs: [
        {
          name: 'conversationTotal',
          schema: totalSchema,
          source: {
            block_id: 'producer',
            output: 'total',
            scope: 'tab'
          }
        }
      ],
      outputs: []
    });
    const graph = createFrontstageSignalGraph([consumer, producer]);
    expect(graph.diagnostics).toEqual([]);
    expect(graph.order).toEqual(['producer', 'consumer']);
  });

  test('AC-007 rejects cycles instead of hiding them behind event depth', () => {
    const first = block('first', {
      inputs: [
        {
          name: 'in',
          schema: totalSchema,
          source: { block_id: 'second', output: 'out', scope: 'tab' }
        }
      ],
      outputs: [{ name: 'out', schema: totalSchema }]
    });
    const second = block('second', {
      inputs: [
        {
          name: 'in',
          schema: totalSchema,
          source: { block_id: 'first', output: 'out', scope: 'tab' }
        }
      ],
      outputs: [{ name: 'out', schema: totalSchema }]
    });
    expect(createFrontstageSignalGraph([first, second]).diagnostics).toEqual(
      expect.arrayContaining([expect.objectContaining({ code: 'cycle' })])
    );
  });

  test('AC-007 validates and commits all outputs atomically', () => {
    const producer = block('producer', {
      inputs: [],
      outputs: [{ name: 'total', schema: totalSchema }]
    });
    const initial = createFrontstageSignalSnapshot();
    const invalid = commitFrontstageBlockOutputs({
      block: producer,
      outputs: { total: 'two' },
      scopes: ['tab', 'page'],
      tabId: 'tab-1',
      snapshot: initial
    });
    expect(invalid.ok).toBe(false);
    expect(invalid.snapshot).toBe(initial);

    const committed = commitFrontstageBlockOutputs({
      block: producer,
      outputs: { total: 2 },
      scopes: ['tab', 'page'],
      tabId: 'tab-1',
      snapshot: initial
    });
    expect(committed.ok).toBe(true);
    expect(
      readFrontstageSignal(committed.snapshot, {
        scope: 'tab',
        tab_id: 'tab-1',
        block_id: 'producer',
        output: 'total'
      })
    ).toBe(2);
    expect(committed.snapshot.revision).toBe(1);
    expect(
      readFrontstageSignal(committed.snapshot, {
        scope: 'page',
        tab_id: 'another-tab',
        block_id: 'producer',
        output: 'total'
      })
    ).toBe(2);
  });

  test('rejects binary resources before they enter the JSON signal snapshot', () => {
    const producer = block('producer', {
      inputs: [],
      outputs: [{ name: 'download', schema: { type: 'object' } }]
    });
    const initial = createFrontstageSignalSnapshot();
    const rejected = commitFrontstageBlockOutputs({
      block: producer,
      outputs: {
        download: {
          bytes: new Uint8Array([1, 2, 3]),
          file_name: 'export.zip',
          content_type: 'application/zip'
        }
      },
      scopes: ['tab'],
      tabId: 'tab-1',
      snapshot: initial
    });
    expect(rejected.ok).toBe(false);
    expect(rejected.snapshot).toBe(initial);
  });
});
