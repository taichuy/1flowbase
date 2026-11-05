import type {
  ConsoleNavigation,
  EventCatalogEntry
} from '@1flowbase/api-client';

import { useAuthStore } from '../../../state/auth-store';
import {
  createStyleBoundaryJsonResponse,
  getStyleBoundaryMethod,
  getStyleBoundaryRequestUrl,
  parseStyleBoundaryRequestUrl
} from '../common-fetch';

const events: EventCatalogEntry[] = [
  {
    id: null,
    enabled: true,
    owner_id: '1flowbase.boot-core',
    contract_id: '1flowbase.communication.command.requested',
    contract_version: '1',
    name: '1flowbase.communication.command.requested',
    description: 'Communication command requested.',
    payload_schema: null,
    origin: 'core',
    supported_phases: []
  },
  {
    id: '019e3b42-0000-7000-8000-000000000001',
    enabled: true,
    owner_id: 'workspace-1',
    contract_id: 'custom.order.fulfillment.completed',
    contract_version: '1.0.0',
    name: 'Order fulfillment completed',
    description: 'Published after an order is fulfilled.',
    payload_schema: {
      type: 'object',
      properties: { order_id: { type: 'string' } },
      required: ['order_id'],
      additionalProperties: false
    },
    origin: 'custom',
    supported_phases: ['published', 'consumed']
  }
];

/** Apply after renderRouterScene, which seeds common fetch and auth again. */
export function seedStyleBoundaryEventsFetch() {
  const me = useAuthStore.getState().me;
  if (me) {
    useAuthStore.setState({
      me: {
        ...me,
        permissions: [
          ...me.permissions,
          'events.catalog.list',
          'events.definitions.create',
          'events.definitions.update',
          'events.runtime.get'
        ]
      }
    });
  }

  const fallbackFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = parseStyleBoundaryRequestUrl(getStyleBoundaryRequestUrl(input));
    if (
      url.origin === window.location.origin &&
      getStyleBoundaryMethod(input, init).toUpperCase() === 'GET'
    ) {
      if (url.pathname === '/api/console/events') {
        return createStyleBoundaryJsonResponse({ data: events, meta: null });
      }
      if (url.pathname === '/api/console/navigation') {
        const response = await fallbackFetch(input, init);
        const envelope = (await response.json()) as {
          data: ConsoleNavigation;
          meta: unknown;
        };
        envelope.data.route_definitions.push({
          route_id: 'settings.events',
          surface_key: 'events',
          path: '/settings/events',
          surface_kind: 'system'
        });
        envelope.data.navigation_items.push({
          item_id: 'settings.events',
          route_id: 'settings.events',
          parent_item_id: 'settings',
          label_key: 'events.title',
          navigation_slot: 'settings',
          order: 30
        });
        return createStyleBoundaryJsonResponse(envelope);
      }
    }
    return fallbackFetch(input, init);
  };
}
