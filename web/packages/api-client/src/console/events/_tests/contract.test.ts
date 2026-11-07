import type { EventCatalogEntry, EventRuntimeSnapshot } from '../types';
import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  createEventDefinition,
  eventCatalogIdentity,
  getEventRuntime,
  listEventCatalog,
  updateEventDefinition
} from '../index';

afterEach(() => vi.unstubAllGlobals());
describe('event center wire contract', () => {
  test('unwraps the actual success envelope without field aliases', async () => {
    const data: EventCatalogEntry[] = [
      {
        id: null,
        enabled: null,
        scope: { kind: 'system' },
        owner_id: 'core',
        name: 'Core ready',
        description: '',
        payload_schema: null,
        contract_id: 'core.ready',
        supported_phases: [],
        group_path: ['billing', 'reservations'],
        source: {
          selection: {
            source_id: 'credits',
            scope: { kind: 'system' },
            filters: { kind: 'reserved' }
          },
          point_id: 'credits.fact'
        },
        origin: 'core'
      }
    ];
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ data, meta: null }), { status: 200 })
      );
    vi.stubGlobal('fetch', fetch);
    await expect(listEventCatalog('https://example.test')).resolves.toEqual(
      data
    );
    expect(fetch.mock.calls[0][0]).toBe(
      'https://example.test/api/console/events'
    );
  });
  test('posts a persistent definition and sends only mutable fields to its update route', async () => {
    const fetch = vi
      .fn()
      .mockImplementation(
        async () =>
          new Response(
            JSON.stringify({ data: { id: 'definition' }, meta: null }),
            { status: 200 }
          )
      );
    vi.stubGlobal('fetch', fetch);
    const input = {
      name: 'Order',
      description: '',
      enabled: false,
      interface_event: {
        interface_id: 'orders.create',
        phase: 'completion' as const,
        results: ['failed' as const, 'rejected' as const, 'cancelled' as const]
      }
    };
    await createEventDefinition(input, 'csrf', 'https://example.test');
    const mutable = input;
    await updateEventDefinition(
      'definition/a',
      mutable,
      'csrf',
      'https://example.test'
    );
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(input);
    expect(fetch.mock.calls[0][1].method).toBe('POST');
    expect(fetch.mock.calls[1][0]).toBe(
      'https://example.test/api/console/events/definition%2Fa'
    );
    expect(fetch.mock.calls[1][1].method).toBe('PUT');
    expect(JSON.parse(fetch.mock.calls[1][1].body)).toEqual(mutable);
    expect(fetch.mock.calls[1][1].headers['x-csrf-token']).toBe('csrf');
  });
  test('runtime observation uses a read-only request', async () => {
    const data: EventRuntimeSnapshot = {
      instance_id: 'instance',
      workspace_id: 'workspace',
      admission_rejected: 0,
      source_views: [],
      source_metadata_bytes: 0,
      source_queue_refresh_error: null,
      observation_bytes: 0,
      observation_max_bytes: 0,
      observation_ttl_ms: 0,
      observation_loss: {
        accounting_available: false,
        window_started_at_unix_ms: null,
        window_duration_ms: 0,
        expired: 0,
        evicted: 0,
        oversize_dropped: 0
      },
      workflow_subscriptions: [],
      workflow_refresh_error: null,
      queues: [],
      activity: [],
      configured: false
    };
    const fetch = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ data, meta: null }), { status: 200 })
      );
    vi.stubGlobal('fetch', fetch);
    await expect(getEventRuntime('https://example.test')).resolves.toEqual(
      data
    );
    expect(fetch.mock.calls[0][0]).toBe(
      'https://example.test/api/console/events/runtime'
    );
    expect(fetch.mock.calls[0][1].method).toBe('GET');
  });
});

test('catalog identity distinguishes typed filters and user rule IDs regardless of filter key order', () => {
  const leaf = {
    contract_id: 'business.fact',
    source: {
      selection: {
        source_id: 'facts',
        scope: { kind: 'system' as const },
        filters: { kind: 'reserved', version: 'v1' }
      }
    }
  };
  expect(eventCatalogIdentity(leaf)).toBe(
    eventCatalogIdentity({
      ...leaf,
      source: {
        selection: {
          ...leaf.source.selection,
          filters: { version: 'v1', kind: 'reserved' }
        }
      }
    })
  );
  expect(eventCatalogIdentity(leaf)).not.toBe(
    eventCatalogIdentity({
      ...leaf,
      source: {
        selection: {
          ...leaf.source.selection,
          filters: { kind: 'settled', version: 'v1' }
        }
      }
    })
  );
  expect(eventCatalogIdentity({ ...leaf, id: 'rule-a' })).not.toBe(
    eventCatalogIdentity({ ...leaf, id: 'rule-b' })
  );
});
