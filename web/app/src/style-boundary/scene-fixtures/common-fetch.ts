import { getStyleBoundaryRuntimeResponse } from './runtime-snapshots';
import {
  seedStyleBoundaryNotificationEvents,
  getStyleBoundaryNotificationResponse
} from './notifications/data';
import { settingsI18nCatalogTestNavigation } from '../../features/settings/pages/i18n-catalog/_tests/i18n-catalog-test-fixture';

export function createStyleBoundaryJsonResponse(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json' }
  });
}

export function getStyleBoundaryRequestUrl(input: RequestInfo | URL) {
  return typeof input === 'string'
    ? input
    : input instanceof Request
      ? input.url
      : String(input);
}

export function getStyleBoundaryMethod(
  input: RequestInfo | URL,
  init?: RequestInit
) {
  return init?.method ?? (input instanceof Request ? input.method : 'GET');
}

export function parseStyleBoundaryRequestUrl(url: string) {
  const baseUrl = globalThis.document?.baseURI ?? globalThis.location?.href;

  return baseUrl ? new URL(url, baseUrl) : new URL(url);
}

export function getStyleBoundaryCommonResponse(
  requestUrl: URL,
  method: string
): Response | null {
  if (
    method.toUpperCase() === 'GET' &&
    ['/api/public/system-metadata', '/api/console/settings/system'].includes(
      requestUrl.pathname
    )
  ) {
    return createStyleBoundaryJsonResponse({
      data: {
        logo_url: '/icon.svg',
        system_name: '1flowbase',
        login_title: '1flowbase',
        system_description: '对话即是壁垒，AI应用原生底座',
        filing_html:
          '<a href="https://beian.miit.gov.cn/">京ICP备12345678号-1</a>'
      },
      meta: null
    });
  }
  const runtimeResponse = getStyleBoundaryRuntimeResponse(requestUrl, method);
  if (runtimeResponse) return runtimeResponse;

  const notificationResponse = getStyleBoundaryNotificationResponse(
    requestUrl,
    method
  );
  if (notificationResponse) return notificationResponse;

  if (
    method.toUpperCase() === 'GET' &&
    requestUrl.pathname === '/api/console/assistant/settings'
  ) {
    return createStyleBoundaryJsonResponse({
      data: {
        preference: {
          application_id: 'flow-1',
          mcp_instance_ids: ['catalog']
        },
        published_agent_flows: [
          { application_id: 'flow-1', name: 'Support Agent' }
        ],
        enabled_mcp_instances: [{ instance_id: 'catalog', name: 'Catalog' }],
        page_reference_max_bytes: 65536,
        page_reference_max_count: 5,
        page_reference_max_total_bytes: 65536,
        run_capabilities: {
          model_selection_enabled: false,
          reasoning_effort_enabled: false,
          models: []
        }
      },
      meta: null
    });
  }

  if (
    method.toUpperCase() === 'GET' &&
    requestUrl.pathname === '/api/console/navigation'
  ) {
    return createStyleBoundaryJsonResponse({
      data: {
        route_definitions: [
          {
            route_id: 'settings.system',
            surface_key: 'system',
            path: '/settings/system',
            surface_kind: 'system'
          },
          {
            route_id: 'embedded-apps',
            surface_key: 'embedded-apps',
            path: '/embedded-apps',
            surface_kind: 'system'
          },
          {
            route_id: 'settings.model-providers',
            surface_key: 'model-providers',
            path: '/settings/model-providers/providers',
            surface_kind: 'system'
          },
          {
            route_id: 'settings.applications',
            surface_key: 'applications',
            path: '/settings/applications',
            surface_kind: 'system'
          },
          {
            route_id: 'settings.backups',
            surface_key: 'backups',
            path: '/settings/backups',
            surface_kind: 'system'
          },
          {
            route_id: 'settings.docs',
            surface_key: 'docs',
            path: '/settings/docs',
            surface_kind: 'system'
          },
          {
            route_id: 'settings.ui-management',
            surface_key: 'ui-management',
            path: '/settings/ui-management/code-templates',
            surface_kind: 'system'
          },
          ...settingsI18nCatalogTestNavigation.route_definitions
        ],
        navigation_items: [
          {
            item_id: 'settings.system',
            route_id: 'settings.system',
            parent_item_id: 'settings',
            label_key: 'system_settings.title',
            navigation_slot: 'settings',
            order: 1500
          },
          {
            item_id: 'embedded-apps',
            route_id: 'embedded-apps',
            parent_item_id: null,
            label_key: 'embeddedApps:auto.subsystem',
            navigation_slot: 'primary',
            order: 3
          },
          {
            item_id: 'model-providers',
            route_id: 'settings.model-providers',
            parent_item_id: 'settings',
            label_key: 'auto.model_providers',
            navigation_slot: 'settings',
            order: 1
          },
          {
            item_id: 'applications',
            route_id: 'settings.applications',
            parent_item_id: 'settings',
            label_key: 'auto.application_management',
            navigation_slot: 'settings',
            order: 2
          },
          {
            item_id: 'backups',
            route_id: 'settings.backups',
            parent_item_id: 'settings',
            label_key: 'auto.backups',
            navigation_slot: 'settings',
            order: 3
          },
          {
            item_id: 'docs',
            route_id: 'settings.docs',
            parent_item_id: 'settings',
            label_key: 'auto.api_documentation',
            navigation_slot: 'settings',
            order: 3
          },
          {
            item_id: 'ui-management',
            route_id: 'settings.ui-management',
            parent_item_id: 'settings',
            label_key: 'auto.ui_management',
            navigation_slot: 'settings',
            order: 4
          },
          ...settingsI18nCatalogTestNavigation.navigation_items
        ],
        permission_bindings: []
      },
      meta: null
    });
  }

  if (
    method.toUpperCase() === 'GET' &&
    requestUrl.pathname === '/api/console/frontstage/pages'
  ) {
    return createStyleBoundaryJsonResponse({ data: [], meta: null });
  }

  if (
    method.toUpperCase() === 'GET' &&
    requestUrl.pathname === '/api/console/system/release-status'
  ) {
    return createStyleBoundaryJsonResponse({
      data: {
        current_version: '0.1.0',
        latest_version: '0.1.0',
        has_update: false,
        release_info: null,
        contributors_url: '/contributors',
        upgrade_commands: {
          shell: '',
          powershell: ''
        },
        cached: true,
        warning: null
      },
      meta: null
    });
  }

  return null;
}

export function seedStyleBoundaryCommonFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  seedStyleBoundaryNotificationEvents();
  const fallbackFetch = globalThis.fetch.bind(globalThis);

  globalThis.fetch = async (input, init) => {
    const url = getStyleBoundaryRequestUrl(input);
    const method = getStyleBoundaryMethod(input, init);
    const requestUrl = parseStyleBoundaryRequestUrl(url);
    const commonResponse = getStyleBoundaryCommonResponse(requestUrl, method);

    if (commonResponse) {
      return commonResponse;
    }

    return fallbackFetch(input as RequestInfo, init);
  };
}
