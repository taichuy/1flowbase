import type {
  EventRuntimeQueueSnapshot,
  EventWorkflowStatus
} from '@1flowbase/api-client';

/**
 * A workflow subscription is identified by subscription_id, NOT by the
 * number of consumers. A workflow status may exist even when activation failed
 * before any runtime queue could be created.

 */
export interface ObservedSubscription {
  id: string;
  queue?: EventRuntimeQueueSnapshot;
  workflow?: EventWorkflowStatus;
  contractId: string;
}

export interface SubscribedEvent {
  contractId: string;
  subscriptions: ObservedSubscription[];
  /** Null means that at least one subscribed runtime queue wasn't observed. */
  buffered: number | null;
  needsAttention: number;
}

/** Groups the visible subscriptions by event, keeping each subscription independent. */
export function buildSubscribedEvents({
  queues,
  workflows
}: {
  queues: EventRuntimeQueueSnapshot[];
  workflows: EventWorkflowStatus[];
}): {
  events: SubscribedEvent[];
  unclassified: EventWorkflowStatus[];
} {
  const byId = new Map<string, ObservedSubscription>();
  for (const queue of queues) {
    byId.set(queue.subscription_id, {
      id: queue.subscription_id,
      queue,
      contractId: queue.contract_id
    });
  }
  const unclassified: EventWorkflowStatus[] = [];
  for (const workflow of workflows) {
    const existing = byId.get(workflow.subscription_id);
    const contractId = existing?.contractId ?? workflow.contract_id;
    if (!contractId) {
      // Older hosts do not include the configured event in activation status.
      // Do not guess which event it belongs to.
      unclassified.push(workflow);
      continue;
    }
    byId.set(workflow.subscription_id, {
      id: workflow.subscription_id,
      contractId,
      queue: existing?.queue,
      workflow
    });
  }
  const groups = new Map<string, SubscribedEvent>();
  function group(contractId: string) {
    let value = groups.get(contractId);
    if (!value) {
      value = {
        contractId,
        subscriptions: [],
        buffered: 0,
        needsAttention: 0
      };
      groups.set(contractId, value);
    }
    return value;
  }
  for (const subscription of byId.values()) {
    const item = group(subscription.contractId);
    item.subscriptions.push(subscription);
    if (subscription.queue) {
      if (item.buffered !== null) item.buffered += subscription.queue.buffered;
    } else {
      // A subscription without a runtime queue does not prove zero backlog.
      item.buffered = null;
    }
    if (
      (subscription.workflow &&
        subscription.workflow.activation !== 'active') ||
      subscription.queue?.closed ||
      (subscription.queue && subscription.queue.consumer_ids.length === 0)
    )
      item.needsAttention += 1;
  }
  return {
    events: [...groups.values()].sort((a, b) => {
      if (a.needsAttention !== b.needsAttention)
        return b.needsAttention - a.needsAttention;
      return a.contractId.localeCompare(b.contractId);
    }),
    unclassified
  };
}
