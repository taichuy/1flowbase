import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import {
  getNotificationUnreadCount,
  subscribeNotificationChanges
} from '@1flowbase/api-client';
import { TestEventSource } from '../../test/event-source';
import { seedStyleBoundaryCommonFetch } from '../scene-fixtures/common-fetch';
import { NotificationSceneEventSource } from '../scene-fixtures/notifications/data';

let originalFetch: typeof fetch;
let originalEventSource: typeof EventSource;

beforeEach(() => {
  originalFetch = globalThis.fetch;
  originalEventSource = globalThis.EventSource;
});

afterEach(() => {
  globalThis.fetch = originalFetch;
  globalThis.EventSource = originalEventSource;
  vi.restoreAllMocks();
});

test('common shell fixture serves the real unread consumer without an API server', async () => {
  const fallback = vi.fn(
    async () => new Response('unhandled', { status: 503 })
  );
  globalThis.fetch = fallback;
  seedStyleBoundaryCommonFetch();

  expect(await getNotificationUnreadCount()).toEqual({ unread_count: 1 });
  expect(fallback).not.toHaveBeenCalled();
});

test('common shell fixture retains real subscription open, refresh and close behavior', async () => {
  const refresh = vi.fn();
  const connection = vi.fn();
  const close = vi.spyOn(NotificationSceneEventSource.prototype, 'close');
  seedStyleBoundaryCommonFetch();
  const seeded = globalThis.EventSource;
  seedStyleBoundaryCommonFetch();
  expect(globalThis.EventSource).toBe(seeded);

  const unsubscribe = subscribeNotificationChanges(refresh, connection);
  try {
    await Promise.resolve();
    expect(connection).toHaveBeenCalledWith(true);
    expect(refresh).toHaveBeenCalledTimes(1);
  } finally {
    unsubscribe();
  }
  expect(close).toHaveBeenCalledTimes(1);
});

test('closing the fixture stream before open prevents a late refresh', async () => {
  const refresh = vi.fn();
  const connection = vi.fn();
  seedStyleBoundaryCommonFetch();
  const unsubscribe = subscribeNotificationChanges(refresh, connection);
  unsubscribe();
  await Promise.resolve();
  expect(refresh).not.toHaveBeenCalled();
  expect(connection).not.toHaveBeenCalled();
});

test('unrelated HTTP URLs and methods keep the original transport and errors', async () => {
  const fallback = vi.fn(
    async () => new Response('unhandled', { status: 503 })
  );
  globalThis.fetch = fallback;
  seedStyleBoundaryCommonFetch();
  const requests: Array<[string, RequestInit?]> = [
    ['/api/console/other'],
    ['https://other.example/api/console/notifications/messages/unread-count'],
    ['/api/console/notifications/messages/unread-count', { method: 'POST' }]
  ];
  for (const [url, init] of requests) {
    expect((await fetch(url, init)).status).toBe(503);
    expect(fallback).toHaveBeenLastCalledWith(url, init);
  }
});

test('unrelated EventSource URLs retain the original constructor and credentials', () => {
  const fallback = vi.fn(function (url: string | URL, init?: EventSourceInit) {
    return new TestEventSource(url, init);
  });
  Object.assign(fallback, { CONNECTING: 0, OPEN: 1, CLOSED: 2 });
  globalThis.EventSource = fallback as unknown as typeof EventSource;
  seedStyleBoundaryCommonFetch();
  const urls = [
    '/api/console/other/events',
    'https://other.example/api/console/notifications/events',
    '/api/console/notifications/events?other=1'
  ];
  for (const url of urls) {
    const stream = new EventSource(url, { withCredentials: true });
    try {
      expect(stream).toBeInstanceOf(TestEventSource);
      expect(fallback).toHaveBeenLastCalledWith(url, { withCredentials: true });
    } finally {
      stream.close();
    }
  }
});
