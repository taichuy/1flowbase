import { describe, expect, test } from 'vitest';

import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../../lib/page-canvas/signal-runtime';

const producer = {
  id: 'producer',
  ports: {
    inputs: [],
    outputs: [{ name: 'total', schema: { type: 'integer' } }]
  }
} as unknown as FrontstageBlockInstance;
const consumer = {
  id: 'consumer',
  ports: {
    inputs: [
      {
        name: 'total',
        schema: { type: 'integer' },
        source: { block_id: 'producer', output: 'total', scope: 'tab' }
      }
    ],
    outputs: []
  }
} as unknown as FrontstageBlockInstance;

describe('Frontstage signal runtime coordinator', () => {
  test('AC-006 injects committed outputs and isolates stale runs', () => {
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, consumer],
      'tab-1'
    );
    expect(coordinator.canRun('consumer')).toBe(false);
    coordinator.beginInstance('producer', 'run-new');
    expect(
      coordinator.commit('producer', 'run-old', { total: 1 })
    ).toMatchObject({
      ok: false,
      stale: true
    });
    expect(coordinator.commit('producer', 'run-new', { total: 2 }).ok).toBe(
      true
    );
    expect(coordinator.canRun('consumer')).toBe(true);
    expect(coordinator.inputsFor('consumer')).toEqual({ total: 2 });
  });

  test('D3-AC-005 accepts multiple publishes from one epoch and rejects old or unmounted epochs', () => {
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, consumer],
      'tab-1'
    );
    const revisions: number[] = [];
    const firstEpoch = coordinator.beginInstance('producer');
    const firstOutputs = coordinator.outputsFor(
      'producer',
      firstEpoch,
      (revision) => revisions.push(revision)
    );
    expect(firstOutputs.publish({ total: 1 })).toMatchObject({
      ok: true,
      stale: false
    });
    expect(firstOutputs.publish({ total: 2 })).toMatchObject({
      ok: true,
      stale: false
    });
    expect(coordinator.inputsFor('consumer')).toEqual({ total: 2 });
    expect(revisions).toEqual([1, 2]);

    coordinator.endInstance('producer', firstEpoch);
    const secondEpoch = coordinator.beginInstance('producer');
    expect(secondEpoch).not.toBe(firstEpoch);
    expect(firstOutputs.publish({ total: 3 })).toEqual({
      ok: false,
      stale: true
    });
    coordinator.endInstance('producer', secondEpoch);
    expect(
      coordinator.outputsFor('producer', secondEpoch).publish({ total: 4 })
    ).toEqual({ ok: false, stale: true });
  });

  test('D3-AC-005 rejects schema-invalid publish without advancing revision', () => {
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, consumer],
      'tab-1'
    );
    const epoch = coordinator.beginInstance('producer');
    expect(
      coordinator.outputsFor('producer', epoch).publish({ total: 'invalid' })
    ).toMatchObject({
      ok: false,
      stale: false,
      error: 'Output does not match its schema: total.'
    });
    expect(coordinator.revision).toBe(0);
    expect(coordinator.canRun('consumer')).toBe(false);
  });

  test('AC-006 retains page-scoped values while switching tabs in one page session', () => {
    const session = createFrontstagePageSignalSession();
    const pageConsumer = {
      ...consumer,
      ports: {
        inputs: [
          {
            ...consumer.ports!.inputs[0],
            source: {
              block_id: 'producer',
              output: 'total',
              scope: 'page' as const,
              tab_id: 'tab-1'
            }
          }
        ],
        outputs: []
      }
    } as unknown as FrontstageBlockInstance;
    const firstTab = new FrontstageSignalRuntimeCoordinator(
      [producer, pageConsumer],
      'tab-1',
      session
    );
    firstTab.beginInstance('producer', 'run-1');
    expect(firstTab.commit('producer', 'run-1', { total: 3 }).ok).toBe(true);
    const secondTab = new FrontstageSignalRuntimeCoordinator(
      [producer, pageConsumer],
      'tab-2',
      session
    );
    expect(secondTab.inputsFor('consumer')).toEqual({ total: 3 });
  });
});
