import type { EventRuntimeQueueSnapshot } from '@1flowbase/api-client';

export function queueStatistics(queues: EventRuntimeQueueSnapshot[]) {
  return {
    queue_count: queues.length,
    open: queues.filter((queue) => !queue.closed).length,
    closed: queues.filter((queue) => queue.closed).length,
    buffered: queues.reduce((sum, queue) => sum + queue.buffered, 0),
    in_flight: queues.reduce((sum, queue) => sum + queue.in_flight, 0),
    accepted: queues.reduce((sum, queue) => sum + queue.accepted, 0),
    consumed: queues.reduce((sum, queue) => sum + queue.consumed, 0),
    admission_rejected: queues.reduce(
      (sum, queue) => sum + queue.admission_rejected,
      0
    ),
    // Consumer identities belong to their queue, so count queue-consumer pairs.
    consumer_count: queues.reduce(
      (sum, queue) => sum + queue.consumer_ids.length,
      0
    )
  };
}

export interface QueueSample {
  captured_at: number;
  buffered: number;
}

export function appendQueueSample(samples: QueueSample[], sample: QueueSample) {
  if (samples.at(-1)?.captured_at === sample.captured_at) return samples;
  return [
    ...samples.filter(
      (point) => point.captured_at >= sample.captured_at - 30 * 60 * 1000
    ),
    sample
  ].slice(-361);
}
