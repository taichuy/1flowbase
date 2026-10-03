import { describe, expect, test, vi } from 'vitest';

import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstageRootNodeBlocks } from '../../lib/page-canvas/runtime-assembly';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../../lib/page-canvas/signal-runtime';

const schema = {
  type: 'object',
  required: ['days'],
  properties: { days: { type: 'integer' } }
};
const blocks: FrontstageBlockInstance[] = ['overview', 'trend', 'users'].map(
  (id, order) => ({
    id,
    rendererVersion: null,
    sourceId: id,
    codeRef: `${id}-code`,
    sourceCodeRef: id,
    catalog: { providerCode: null, installationId: null },
    contribution: { pluginId: null, pluginVersion: null, code: id },
    props: {},
    presentation: { heightMode: 'auto', height: null },
    layout: { order },
    order,
    runtime: { kind: 'native_react', entry: null, hint: 'native_react' },
    input_mapping: { timeRange: 'usage.timeRange' },
    output_mapping: { timeRange: 'usage.timeRange' },
    ports: {
      inputs: [{ name: 'timeRange', schema }],
      outputs: [{ name: 'timeRange', schema }]
    }
  })
);

function setup() {
  const session = createFrontstagePageSignalSession();
  const coordinator = new FrontstageSignalRuntimeCoordinator(
    blocks,
    'tab-1',
    session
  );
  const listeners = blocks.map(({ id }) => {
    coordinator.beginInstance(id, id);
    const listener = vi.fn();
    coordinator.subscribeBlock(id, listener);
    return listener;
  });
  return { coordinator, session, listeners };
}

describe('shared page variable mappings', () => {
  test('every bound block can update the same filter including its own input without DAG cycles', () => {
    const { coordinator, listeners } = setup();
    expect(coordinator.graph.diagnostics).toEqual([]);
    for (const [index, block] of blocks.entries()) {
      expect(coordinator.canRun(block.id)).toBe(true);
      expect(
        coordinator.commit(block.id, block.id, {
          timeRange: { days: index + 1 }
        })
      ).toEqual({ ok: true, stale: false });
      for (const consumer of blocks) {
        expect(coordinator.inputsFor(consumer.id)).toEqual({
          timeRange: { days: index + 1 }
        });
        expect(
          Object.isFrozen(coordinator.inputsFor(consumer.id).timeRange)
        ).toBe(true);
      }
    }
    for (const listener of listeners) expect(listener).toHaveBeenCalledTimes(3);
  });

  test('equivalent object values retain snapshots and do not notify subscribers', () => {
    const { coordinator, listeners } = setup();
    coordinator.commit('overview', 'overview', {
      timeRange: { days: 7, bucket: 'day' }
    });
    const snapshots = blocks.map(({ id }) => coordinator.getBlockSnapshot(id));
    coordinator.commit('trend', 'trend', {
      timeRange: { bucket: 'day', days: 7 }
    });
    blocks.forEach(({ id }, index) =>
      expect(coordinator.getBlockSnapshot(id)).toBe(snapshots[index])
    );
    for (const listener of listeners) expect(listener).toHaveBeenCalledTimes(1);
  });

  test('stale epochs and invalid outputs cannot overwrite the shared filter or notify', () => {
    const { coordinator, listeners } = setup();
    coordinator.commit('overview', 'overview', { timeRange: { days: 7 } });
    const revision = coordinator.revision;
    expect(
      coordinator.commit('trend', 'old', { timeRange: { days: 1 } }).stale
    ).toBe(true);
    expect(
      coordinator.commit('users', 'users', { timeRange: { days: 'invalid' } })
        .ok
    ).toBe(false);
    expect(coordinator.commit('users', 'users', {}).ok).toBe(false);
    expect(coordinator.revision).toBe(revision);
    expect(coordinator.inputsFor('trend')).toEqual({ timeRange: { days: 7 } });
    for (const listener of listeners) expect(listener).toHaveBeenCalledTimes(1);
  });

  test('retains values across tabs in one page session and isolates another page', () => {
    const { coordinator, session } = setup();
    coordinator.commit('overview', 'overview', { timeRange: { days: 30 } });
    coordinator.dispose();
    const nextTab = new FrontstageSignalRuntimeCoordinator(
      blocks,
      'tab-2',
      session
    );
    expect(nextTab.inputsFor('trend')).toEqual({ timeRange: { days: 30 } });
    const otherPage = new FrontstageSignalRuntimeCoordinator(blocks, 'tab-1');
    expect(otherPage.inputsFor('trend')).toEqual({});
    nextTab.beginInstance('users', 'new-tab');
    nextTab.commit('users', 'new-tab', { timeRange: { days: 1 } });
    expect(coordinator.inputsFor('overview')).toEqual({
      timeRange: { days: 1 }
    });
    expect(otherPage.inputsFor('overview')).toEqual({});
  });

  test('preserves persisted mappings when assembling block nodes', () => {
    const assembled = createFrontstageRootNodeBlocks([
      {
        block_id: 'overview',
        runtime_descriptor: { ports: blocks[0]!.ports },
        input_mapping: blocks[0]!.input_mapping,
        output_mapping: blocks[0]!.output_mapping
      } as Parameters<typeof createFrontstageRootNodeBlocks>[0][number]
    ]);
    expect(assembled[0]!.input_mapping).toEqual({
      timeRange: 'usage.timeRange'
    });
    expect(assembled[0]!.output_mapping).toEqual({
      timeRange: 'usage.timeRange'
    });
  });
});
