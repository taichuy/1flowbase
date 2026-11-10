import type {
  EventRuntimeQueueSnapshot,
  EventWorkflowStatus
} from '@1flowbase/api-client';
import { describe, expect, test } from 'vitest';
import { buildSubscribedEvents } from '../runtime/subscription-view';

const queue = (
  overrides: Partial<EventRuntimeQueueSnapshot> = {}
): EventRuntimeQueueSnapshot => ({
  subscription_id: 'subscription-one',
  workspace_id: 'workspace-one',
  contract_id: 'communication.command.requested',
  trigger_phase: 'published',
  consumer_ids: ['app-one', 'worker-two'],
  closed: false,
  buffered: 5,
  in_flight: 2,
  accepted: 10,
  consumed: 5,
  admission_rejected: 0,
  ...overrides
});

const workflow = (
  overrides: Partial<EventWorkflowStatus> = {}
): EventWorkflowStatus => ({
  workspace_id: 'workspace-one',
  application_id: 'app-one',
  subscription_id: 'subscription-one',
  contract_id: 'communication.command.requested',
  trigger_phase: 'published',
  source: null,
  activation: 'active',
  reason: null,
  last_signal_id: null,
  last_run_id: null,
  last_delivery: null,
  last_delivery_reason: null,
  ...overrides
});

describe('event-first subscription projection', () => {
  test('joins status and runtime queue once by subscription_id, not by consumer count', () => {
    const result = buildSubscribedEvents({
      queues: [queue(), queue({ subscription_id: 'subscription-one' })],
      workflows: [workflow()]
    });
    expect(result.events).toHaveLength(1);
    expect(result.events[0].subscriptions).toHaveLength(1);
    expect(result.events[0].subscriptions[0].workflow?.application_id).toBe(
      'app-one'
    );
    expect(result.events[0].buffered).toBe(5);
  });

  test('failed activation stays under its configured event without pretending to have zero backlog', () => {
    const result = buildSubscribedEvents({
      queues: [queue()],
      workflows: [
        workflow({
          subscription_id: 'unavailable-subscription',
          application_id: 'app-two',
          activation: 'unavailable',
          reason: 'activation_failed'
        })
      ]
    });
    expect(result.events[0].subscriptions).toHaveLength(2);
    expect(result.events[0].needsAttention).toBe(1);
    expect(result.events[0].buffered).toBeNull();
  });

  test('only an actual subscription creates an event row', () => {
    expect(buildSubscribedEvents({ queues: [], workflows: [] }).events).toEqual(
      []
    );
    const result = buildSubscribedEvents({ queues: [queue()], workflows: [] });
    expect(result.events).toHaveLength(1);
    expect(result.events[0].subscriptions).toHaveLength(1);
    expect(result.events[0].buffered).toBe(5);
  });

  test('legacy workflow statuses without event identity are not falsely attributed to an event', () => {
    const status = workflow({ contract_id: undefined });
    const result = buildSubscribedEvents({
      queues: [],
      workflows: [status]
    });
    expect(result.events).toEqual([]);
    expect(result.unclassified).toEqual([status]);
  });
});
