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
    const firstEpoch = coordinator.beginInstance('producer');
    const firstOutputs = coordinator.outputsFor('producer', firstEpoch);
    expect(firstOutputs.publish({ total: 1 })).toMatchObject({
      ok: true,
      stale: false
    });
    expect(firstOutputs.publish({ total: 2 })).toMatchObject({
      ok: true,
      stale: false
    });
    expect(coordinator.inputsFor('consumer')).toEqual({ total: 2 });
    expect(coordinator.revision).toBe(2);

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

  test('D3R-AC-002 exposes immutable, referentially stable block snapshots and only notifies direct DAG dependents', () => {
    const transitiveConsumer = {
      id: 'transitive-consumer',
      ports: {
        inputs: [
          {
            name: 'derived',
            schema: { type: 'integer' },
            source: {
              block_id: 'consumer',
              output: 'derived',
              scope: 'tab'
            }
          }
        ],
        outputs: []
      }
    } as unknown as FrontstageBlockInstance;
    const producingConsumer = {
      ...consumer,
      ports: {
        ...consumer.ports,
        outputs: [{ name: 'derived', schema: { type: 'integer' } }]
      }
    } as unknown as FrontstageBlockInstance;
    const unrelated = {
      id: 'unrelated',
      ports: { inputs: [], outputs: [] }
    } as unknown as FrontstageBlockInstance;
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, producingConsumer, transitiveConsumer, unrelated],
      'tab-1'
    );
    const consumerNotifications: number[] = [];
    const transitiveNotifications: number[] = [];
    const unrelatedNotifications: number[] = [];
    coordinator.subscribeBlock('consumer', () =>
      consumerNotifications.push(
        coordinator.getBlockSnapshot('consumer').revision
      )
    );
    coordinator.subscribeBlock('transitive-consumer', () =>
      transitiveNotifications.push(1)
    );
    coordinator.subscribeBlock('unrelated', () =>
      unrelatedNotifications.push(1)
    );

    const initial = coordinator.getBlockSnapshot('consumer');
    expect(coordinator.getBlockSnapshot('consumer')).toBe(initial);
    expect(Object.isFrozen(initial)).toBe(true);
    expect(Object.isFrozen(initial.inputs)).toBe(true);

    const epoch = coordinator.beginInstance('producer');
    expect(coordinator.commit('producer', epoch, { total: 2 }).ok).toBe(true);
    const changed = coordinator.getBlockSnapshot('consumer');
    expect(changed).not.toBe(initial);
    expect(changed).toEqual({ revision: 1, inputs: { total: 2 } });
    expect(coordinator.getBlockSnapshot('consumer')).toBe(changed);
    expect(consumerNotifications).toEqual([1]);
    expect(transitiveNotifications).toEqual([]);
    expect(unrelatedNotifications).toEqual([]);
  });

  test('D3R-AC-003 rejects stale and schema-invalid publishes without commit, snapshot replacement, or notification', () => {
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, consumer],
      'tab-1'
    );
    const notifications: number[] = [];
    coordinator.subscribeBlock('consumer', () => notifications.push(1));
    const initial = coordinator.getBlockSnapshot('consumer');
    const epoch = coordinator.beginInstance('producer', 'latest');

    expect(coordinator.commit('producer', 'stale', { total: 1 })).toEqual({
      ok: false,
      stale: true
    });
    expect(
      coordinator.commit('producer', epoch, { total: 'invalid' })
    ).toMatchObject({
      ok: false,
      stale: false
    });
    expect(coordinator.revision).toBe(0);
    expect(coordinator.getBlockSnapshot('consumer')).toBe(initial);
    expect(notifications).toEqual([]);
  });

  test('D3R-AC-004 keeps multi-publish deterministic and clears subscriptions, epochs, and store state', () => {
    const coordinator = new FrontstageSignalRuntimeCoordinator(
      [producer, consumer],
      'tab-1'
    );
    const revisions: number[] = [];
    const unsubscribe = coordinator.subscribeBlock('consumer', () =>
      revisions.push(coordinator.getBlockSnapshot('consumer').revision)
    );
    const epoch = coordinator.beginInstance('producer');
    expect(coordinator.commit('producer', epoch, { total: 1 }).ok).toBe(true);
    expect(coordinator.commit('producer', epoch, { total: 2 }).ok).toBe(true);
    expect(revisions).toEqual([1, 2]);

    unsubscribe();
    expect(coordinator.commit('producer', epoch, { total: 3 }).ok).toBe(true);
    expect(revisions).toEqual([1, 2]);

    coordinator.clear();
    expect(coordinator.revision).toBe(0);
    expect(coordinator.instanceEpochFor('producer')).toBeNull();
    expect(coordinator.inputsFor('consumer')).toEqual({});
    expect(coordinator.commit('producer', epoch, { total: 4 })).toEqual(
      {
        ok: false,
        stale: true
      }
    );

    const disposedNotifications: number[] = [];
    const disposedEpoch = coordinator.beginInstance('producer');
    coordinator.subscribeBlock('consumer', () =>
      disposedNotifications.push(1)
    );
    coordinator.dispose();
    expect(
      coordinator.commit('producer', disposedEpoch, { total: 5 })
    ).toEqual({
      ok: false,
      stale: true
    });
    expect(disposedNotifications).toEqual([]);
  });
});
