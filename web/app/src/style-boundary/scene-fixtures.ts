import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { useAuthStore } from '../state/auth-store';
import {
  modelProviderCatalogContract,
  modelProviderOptionsContract
} from '../test/model-provider-contract-fixtures';
import {
  createSettingsI18nCatalogTestServer,
  settingsI18nCatalogTestNavigation
} from '../features/settings/pages/i18n-catalog/_tests/i18n-catalog-test-fixture';

import {
  styleBoundaryApplicationRunRecord,
  styleBoundaryMcpCatalog,
  styleBoundaryMcpInterfaceCapabilities,
  styleBoundaryNodeContributions,
  styleBoundaryOfficialPluginCatalog,
  styleBoundaryPluginFamiliesCatalog,
  styleBoundaryProviderInstances
} from './scene-fixtures/settings/catalogs';

export {
  styleBoundaryMcpCatalog,
  styleBoundaryMcpInterfaceCapabilities,
  styleBoundaryNodeContributions
};

export function seedStyleBoundaryAuth() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'style-boundary-csrf',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'member',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Captain Root',
      name: 'Root',
      avatar_url: null,
      introduction: 'Boundary user',
      effective_display_role: 'root',
      permissions: [
        'application.view.all',
        'application.edit.own',
        'application.create.all',
        'embedded_app.view.all',
        'api_reference.view.all',
        'system_runtime.view.all',
        'state_model.view.all',
        'state_model.manage.all',
        'file_table.view.all',
        'file_object.view.all',
        'file_storage.view.all',
        'frontstage.page.design',
        'mcp_management.view.all',
        'mcp_management.manage.all',
        'user.view.all',
        'user.manage.all',
        'role_permission.view.all',
        'role_permission.manage.all'
      ]
    }
  });
}

let styleBoundaryOriginalFetch: typeof globalThis.fetch | null = null;

function createStyleBoundaryJsonResponse(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json' }
  });
}

function getStyleBoundaryRequestUrl(input: RequestInfo | URL) {
  return typeof input === 'string'
    ? input
    : input instanceof Request
      ? input.url
      : String(input);
}

function getStyleBoundaryMethod(input: RequestInfo | URL, init?: RequestInit) {
  return init?.method ?? (input instanceof Request ? input.method : 'GET');
}

function parseStyleBoundaryRequestUrl(url: string) {
  const baseUrl = globalThis.document?.baseURI ?? globalThis.location?.href;

  return baseUrl ? new URL(url, baseUrl) : new URL(url);
}

function getStyleBoundaryCommonResponse(
  requestUrl: URL,
  method: string
): Response | null {
  if (
    method.toUpperCase() === 'GET' &&
    requestUrl.pathname === '/api/console/navigation'
  ) {
    return createStyleBoundaryJsonResponse({
      data: {
        route_definitions: [
          {
            route_id: 'home',
            surface_key: 'home',
            path: '/',
            surface_kind: 'system'
          },
          {
            route_id: 'embedded-apps',
            surface_key: 'embedded-apps',
            path: '/embedded-apps',
            surface_kind: 'system'
          },
          {
            route_id: 'templates',
            surface_key: 'templates',
            path: '/templates',
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
            route_id: 'settings.docs',
            surface_key: 'docs',
            path: '/settings/docs',
            surface_kind: 'system'
          },
          ...settingsI18nCatalogTestNavigation.route_definitions
        ],
        navigation_items: [
          {
            item_id: 'home',
            route_id: 'home',
            parent_item_id: null,
            label_key: 'auto.workbench',
            navigation_slot: 'primary',
            order: 1
          },
          {
            item_id: 'embedded-apps',
            route_id: 'embedded-apps',
            parent_item_id: null,
            label_key: 'auto.subsystem',
            navigation_slot: 'primary',
            order: 3
          },
          {
            item_id: 'templates',
            route_id: 'templates',
            parent_item_id: null,
            label_key: 'auto.templates',
            navigation_slot: 'primary',
            order: 4
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
            item_id: 'docs',
            route_id: 'settings.docs',
            parent_item_id: 'settings',
            label_key: 'auto.api_documentation',
            navigation_slot: 'settings',
            order: 3
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

function createStyleBoundaryAgentFlowDocument() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
  const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

  if (llmNode) {
    llmNode.config = {
      ...llmNode.config,
      provider_instance_id: 'provider-openai-prod',
      model: 'gpt-4o-mini',
      temperature: 0.7
    };
  }

  return document;
}

export function createStyleBoundaryOrchestrationState() {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-15T09:00:00Z',
      document: createStyleBoundaryAgentFlowDocument()
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10
  };
}

export function seedStyleBoundaryTemplateFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  styleBoundaryOriginalFetch ??= globalThis.fetch.bind(globalThis);
  const originalFetch = styleBoundaryOriginalFetch;

  globalThis.fetch = async (input, init) => {
    const url = getStyleBoundaryRequestUrl(input);
    const method = getStyleBoundaryMethod(input, init);
    const requestUrl = parseStyleBoundaryRequestUrl(url);
    const commonResponse = getStyleBoundaryCommonResponse(requestUrl, method);

    if (commonResponse) {
      return commonResponse;
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/frontstage/workspace-1/pages'
    ) {
      return createStyleBoundaryJsonResponse({ data: [], meta: null });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname ===
        '/api/console/settings/extension-center/installed' &&
      requestUrl.searchParams.get('category') === 'agent-flow'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          limit: 50,
          total_entries: 1,
          next_cursor: null,
          entries: [
            {
              id: 'boundary-template-v1',
              category: 'agent-flow',
              catalog_id: 'agent-flow:openai/boundary-template',
              organization: 'openai',
              artifact_id: 'boundary-template',
              version: '1.0.0',
              node_id: 'node-1',
              source: 'official',
              trust: 'official',
              warnings: [],
              local_path: '/extensions/boundary-template/1.0.0',
              checksum: 'sha256:boundary-template',
              signature_status: 'valid',
              signature_algorithm: 'ed25519',
              signing_key_id: 'official-key',
              status: 'installed',
              is_current: true,
              application_action: 'import_agent_flow',
              application_status: 'not_applied',
              installed_by: 'user-1',
              created_at: '2026-06-16T00:00:00.000Z',
              updated_at: '2026-06-16T00:00:00.000Z',
              installed_versions: [
                {
                  id: 'boundary-template-v1',
                  version: '1.0.0',
                  source: 'official',
                  trust: 'official',
                  warnings: [],
                  local_path: '/extensions/boundary-template/1.0.0',
                  checksum: 'sha256:boundary-template',
                  signature_status: 'valid',
                  signature_algorithm: 'ed25519',
                  signing_key_id: 'official-key',
                  status: 'installed',
                  is_current: true,
                  installed_by: 'user-1',
                  created_at: '2026-06-16T00:00:00.000Z',
                  updated_at: '2026-06-16T00:00:00.000Z'
                }
              ]
            }
          ]
        },
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname ===
        '/api/console/settings/extension-center/catalog/agent-flow'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          category: 'agent-flow',
          catalog_page: 'boundary-agent-flow',
          catalog_page_number: 1,
          catalog_page_checksum: 'sha256:boundary-agent-flow',
          catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
          limit: 20,
          next_cursor: null,
          total_entries: 0,
          entries: []
        },
        meta: null
      });
    }

    return originalFetch(input as RequestInfo, init);
  };
}

function createStyleBoundaryRuntimeMetrics(
  cpuUsagePercent: number,
  processBytes: number
) {
  return {
    captured_at_unix_milliseconds: 1_752_745_600_000,
    sample_interval_milliseconds: 2000,
    cpu: {
      availability: 'available',
      scope_kind: 'cgroup',
      usage_percent: cpuUsagePercent,
      logical_count: 8,
      limit_cores: 2
    },
    memory: {
      availability: 'available',
      scope_kind: 'cgroup',
      total_bytes: 4_294_967_296,
      available_bytes: 3_221_225_472,
      used_bytes: 1_073_741_824,
      process_bytes: processBytes,
      related_process_bytes: processBytes,
      related_process_count: 1,
      cgroup_composition: {
        anonymous_bytes: 536_870_912,
        file_bytes: 268_435_456,
        kernel_bytes: 67_108_864,
        shared_memory_bytes: 16_777_216
      }
    },
    storage: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      mount_point: '/',
      file_system: 'overlay',
      total_bytes: 68_719_476_736,
      available_bytes: 51_539_607_552,
      used_bytes: 17_179_869_184
    },
    network: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      received_bytes_per_second: 2048,
      transmitted_bytes_per_second: 1024
    },
    disk_io: {
      availability: 'available',
      scope_kind: 'runtime_visible',
      read_bytes_per_second: 4096,
      written_bytes_per_second: 8192
    }
  };
}

export function seedStyleBoundarySettingsFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  styleBoundaryOriginalFetch ??= globalThis.fetch.bind(globalThis);
  const originalFetch = styleBoundaryOriginalFetch;
  const i18nCatalogServer = createSettingsI18nCatalogTestServer();
  window.__STYLE_BOUNDARY_I18N_CATALOG_REQUESTS__ = [];

  globalThis.fetch = async (input, init) => {
    const url = getStyleBoundaryRequestUrl(input);
    const method = getStyleBoundaryMethod(input, init);
    const requestUrl = parseStyleBoundaryRequestUrl(url);
    const commonResponse = getStyleBoundaryCommonResponse(requestUrl, method);

    if (commonResponse) {
      return commonResponse;
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/system/runtime-profile'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          provider_install_root: '/opt/1flowbase/plugins',
          host_extension_dropin_root:
            '/opt/1flowbase/plugins/host-extension/dropins',
          locale_meta: {
            requested_locale: null,
            resolved_locale: 'zh_Hans',
            source: 'fallback',
            fallback_locale: 'en_US',
            supported_locales: ['zh_Hans', 'en_US']
          },
          topology: { relationship: 'same_host' },
          related_process_memory_complete: true,
          services: {
            api_server: {
              reachable: true,
              service: 'api-server',
              status: 'ok',
              version: '0.2.6',
              host_fingerprint: 'host-boundary'
            },
            plugin_runner: {
              reachable: true,
              service: 'plugin-runner',
              status: 'ok',
              version: '0.2.6',
              host_fingerprint: 'host-boundary'
            }
          },
          hosts: [
            {
              host_fingerprint: 'host-boundary',
              platform: {
                os: 'linux',
                arch: 'amd64',
                libc: 'musl',
                rust_target_triple: 'x86_64-unknown-linux-musl'
              },
              cpu: { logical_count: 8 },
              related_process_bytes: 402_653_184,
              related_process_count: 2,
              memory: {
                total_bytes: 4_294_967_296,
                total_gb: 4,
                available_bytes: 3_221_225_472,
                available_gb: 3,
                process_bytes: 268_435_456,
                process_gb: 0.25
              },
              services: ['api-server', 'plugin-runner']
            }
          ],
          runtime_targets: [
            {
              target_id: 'api-server',
              reachable: true,
              host_fingerprint: 'host-boundary',
              metrics: createStyleBoundaryRuntimeMetrics(12.5, 268_435_456)
            },
            {
              target_id: 'plugin-runner',
              reachable: true,
              host_fingerprint: 'host-boundary',
              metrics: createStyleBoundaryRuntimeMetrics(8.5, 134_217_728)
            }
          ]
        },
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/i18n/entries'
    ) {
      window.__STYLE_BOUNDARY_I18N_CATALOG_REQUESTS__?.push(
        Object.fromEntries(requestUrl.searchParams)
      );
      return createStyleBoundaryJsonResponse({
        data: await i18nCatalogServer.listEntriesFromSearchParams(
          requestUrl.searchParams
        ),
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/i18n/entries/detail'
    ) {
      return createStyleBoundaryJsonResponse({
        data: await i18nCatalogServer.getEntry({
          key: requestUrl.searchParams.get('key') ?? '',
          locale: requestUrl.searchParams.get('locale') ?? ''
        }),
        meta: null
      });
    }

    if (
      ['PUT', 'DELETE'].includes(method.toUpperCase()) &&
      requestUrl.pathname === '/api/console/settings/i18n/overrides'
    ) {
      const request = JSON.parse(String(init?.body ?? '{}'));
      const data =
        method.toUpperCase() === 'PUT'
          ? await i18nCatalogServer.saveOverride(request)
          : await i18nCatalogServer.restoreOverride(request);
      return createStyleBoundaryJsonResponse({ data, meta: null });
    }

    if (
      method.toUpperCase() === 'PUT' &&
      requestUrl.pathname === '/api/console/settings/i18n/custom-translations'
    ) {
      const request = JSON.parse(String(init?.body ?? '{}'));
      return createStyleBoundaryJsonResponse({
        data: await i18nCatalogServer.saveCustomTranslation(request),
        meta: null
      });
    }

    if (url.includes('/api/console/docs/catalog')) {
      return new Response(
        JSON.stringify({
          data: {
            title: '1flowbase API',
            version: '0.1.0',
            categories: [
              {
                id: 'console',
                label: 'console',
                operation_count: 2
              }
            ]
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/applications'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          items: [
            {
              id: 'boundary-workflow',
              application_type: 'workflow',
              workflow_trigger_type: 'schedule',
              name: 'Boundary Workflow',
              description: 'Style boundary application',
              icon: null,
              icon_type: null,
              icon_background: null,
              created_by: 'user-1',
              created_by_display_name: 'Root',
              created_at: '2026-07-15T00:00:00Z',
              updated_at: '2026-07-15T00:00:00Z',
              tags: [{ id: 'tag-boundary', name: 'Boundary' }],
              publication_status: 'published'
            }
          ],
          total: 1,
          page: 1,
          page_size: 20
        },
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/catalog'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          types: [
            {
              value: 'agent_flow',
              label: 'AgentFlow'
            },
            {
              value: 'workflow',
              label: 'Workflow'
            }
          ],
          workflow_triggers: [
            {
              value: 'extension',
              label: 'Extension'
            },
            {
              value: 'schedule',
              label: 'Schedule'
            }
          ],
          tags: [
            {
              id: 'tag-boundary',
              name: 'Boundary',
              application_count: 1
            }
          ]
        },
        meta: null
      });
    }

    if (url.includes('/api/console/docs/categories/console/operations')) {
      return new Response(
        JSON.stringify({
          data: {
            id: 'console',
            label: 'console',
            operations: [
              {
                id: 'patch_me',
                method: 'PATCH',
                path: '/api/console/me',
                summary: 'Update current profile',
                description: 'Update current profile',
                tags: ['console'],
                group: 'console',
                deprecated: false
              },
              {
                id: 'list_members',
                method: 'GET',
                path: '/api/console/members',
                summary: 'List members',
                description: 'List members',
                tags: ['console'],
                group: 'console',
                deprecated: false
              }
            ]
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      url.includes('/api/console/docs/operations/list_members/openapi.json')
    ) {
      return new Response(
        JSON.stringify({
          openapi: '3.1.0',
          info: { title: '1flowbase API', version: '0.1.0' },
          paths: {
            '/api/console/members': {
              get: {
                operationId: 'list_members',
                responses: {
                  '200': { description: 'ok' }
                }
              }
            }
          },
          components: {}
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/model-providers/options'
    ) {
      return new Response(
        JSON.stringify({
          data: modelProviderOptionsContract,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/model-providers/catalog'
    ) {
      return new Response(
        JSON.stringify({
          data: modelProviderCatalogContract,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/mcp/catalog'
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryMcpCatalog,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/mcp/bundles/library'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          remote_available: true,
          bundles: [
            {
              organization: 'taichuy',
              bundle_id: '1flowbase_zh_hans',
              current_bundle_version: '1.1.1',
              remote_versions: [],
              local_versions: [
                {
                  bundle_version: '1.1.1',
                  locale: 'zh_Hans',
                  minimum_host_version: '0.3.2',
                  exported_from_system_version: '0.3.2',
                  checksum: 'style-boundary-mcp-bundle',
                  signature_status: 'verified',
                  downloaded_at: '2026-08-02T10:00:00Z'
                }
              ]
            }
          ]
        },
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/mcp/interface-capabilities'
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryMcpInterfaceCapabilities,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      url.includes('/api/console/plugins/families')
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryPluginFamiliesCatalog,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      url.includes('/api/console/plugins/official-catalog')
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryOfficialPluginCatalog,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      url.endsWith('/api/console/settings/model-providers/instances')
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryProviderInstances,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    return originalFetch(input as RequestInfo, init);
  };
}

export function seedStyleBoundaryApplicationFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  styleBoundaryOriginalFetch ??= globalThis.fetch.bind(globalThis);
  const originalFetch = styleBoundaryOriginalFetch;
  let currentDraftDocument = createStyleBoundaryAgentFlowDocument();

  globalThis.fetch = async (input, init) => {
    const url = getStyleBoundaryRequestUrl(input);
    const method = getStyleBoundaryMethod(input, init);
    const requestUrl = parseStyleBoundaryRequestUrl(url);
    const commonResponse = getStyleBoundaryCommonResponse(requestUrl, method);

    if (commonResponse) {
      return commonResponse;
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/frontstage/workspace-1/pages'
    ) {
      return createStyleBoundaryJsonResponse({ data: [], meta: null });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/model-providers/options'
    ) {
      return new Response(
        JSON.stringify({
          data: modelProviderOptionsContract,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/node-contributions' &&
      requestUrl.searchParams.get('application_id') === 'app-1'
    ) {
      return new Response(
        JSON.stringify({
          data: styleBoundaryNodeContributions,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname.includes(
        '/api/console/applications/app-1/orchestration/nodes/'
      ) &&
      requestUrl.pathname.endsWith('/last-run')
    ) {
      return new Response(
        JSON.stringify({
          data: null,
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname ===
        '/api/console/applications/app-1/orchestration/debug-variable-snapshot'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            variable_cache: {}
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'PUT' &&
      url.includes('/api/console/applications/app-1/orchestration/draft')
    ) {
      const requestBody =
        typeof init?.body === 'string'
          ? JSON.parse(init.body)
          : init?.body && typeof init.body === 'object'
            ? init.body
            : null;

      if (
        requestBody &&
        'document' in requestBody &&
        requestBody.document &&
        typeof requestBody.document === 'object'
      ) {
        currentDraftDocument = requestBody.document as ReturnType<
          typeof createDefaultAgentFlowDocument
        >;
      }

      return new Response(
        JSON.stringify({
          data: {
            flow_id: 'flow-1',
            messages: [],
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '2026-04-15T09:10:00Z',
              document: currentDraftDocument
            },
            versions: [],
            autosave_interval_seconds: 30,
            user_protection_limit: 10
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (url.endsWith('/api/console/applications/app-1/orchestration')) {
      return new Response(
        JSON.stringify({
          data: {
            flow_id: 'flow-1',
            messages: [],
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '2026-04-15T09:00:00Z',
              document: currentDraftDocument
            },
            versions: [],
            autosave_interval_seconds: 30,
            user_protection_limit: 10
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      url.endsWith('/api/console/applications/app-1/environment-variables')
    ) {
      return new Response(
        JSON.stringify({
          data: [],
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/app-1/api-keys'
    ) {
      return new Response(
        JSON.stringify({
          data: [
            {
              id: 'key-1',
              name: 'Production client',
              token_prefix: 'sk-019e1a2b48',
              creator_user_id: 'user-1',
              enabled: true,
              expires_at: null,
              created_at: '2026-05-09T10:00:00Z',
              updated_at: '2026-05-09T10:00:00Z'
            }
          ],
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'POST' &&
      requestUrl.pathname === '/api/console/applications/app-1/api-keys'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            id: 'key-created',
            name: 'Production client',
            token: 'sk-019e1a463b39-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCD',
            token_prefix: 'sk-019e1a463b39',
            creator_user_id: 'user-1',
            enabled: true,
            expires_at: null,
            created_at: '2026-05-09T10:00:00Z',
            updated_at: '2026-05-09T10:00:00Z'
          },
          meta: null
        }),
        {
          status: 201,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/app-1/api-mapping'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            input: {
              query_target: 'start.query',
              model_target: null,
              inputs_target: null,
              history_target: null,
              attachments_target: null
            },
            output: {
              answer_selector: 'answer',
              usage_selector: null,
              files_selector: null,
              error_selector: null
            }
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/app-1/api-publication'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            id: 'pub-1',
            application_id: 'app-1',
            flow_id: 'flow-1',
            flow_version_id: 'version-1',
            compiled_plan_id: 'compiled-1',
            version_sequence: 3,
            active: true,
            api_enabled: true,
            public_url: '/api/agent/v1/runs',
            created_by: 'user-1',
            created_at: '2026-05-09T10:00:00Z',
            mapping_snapshot: {
              input: {
                query_target: 'start.query',
                model_target: null,
                inputs_target: null,
                history_target: null,
                attachments_target: null
              },
              output: {
                answer_selector: 'answer',
                usage_selector: null,
                files_selector: null,
                error_selector: null
              }
            }
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/app-1/api-docs/catalog'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            title: 'Support Agent API',
            version: 'v3',
            categories: [
              {
                id: 'application-native-api',
                label: 'Application Native API',
                operation_count: 1
              },
              {
                id: 'openai-compatible-api',
                label: 'OpenAI Compatible API',
                operation_count: 1
              }
            ]
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname.includes(
        '/api/console/applications/app-1/api-docs/categories/'
      ) &&
      requestUrl.pathname.endsWith('/operations')
    ) {
      return new Response(
        JSON.stringify({
          data: {
            id: 'application-native-api',
            label: 'Application Native API',
            operations: [
              {
                id: 'application-native-api-run-operation',
                method: 'POST',
                path: '/api/agent/v1/runs',
                summary: 'Run published application',
                description: 'Run published application',
                tags: ['application-public-api'],
                group: 'application-native-api',
                deprecated: false
              }
            ]
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname.includes(
        '/api/console/applications/app-1/api-docs/operations/'
      ) &&
      requestUrl.pathname.endsWith('/openapi.json')
    ) {
      return new Response(
        JSON.stringify({
          openapi: '3.1.0',
          info: { title: 'Support Agent API', version: 'v3' },
          paths: {
            '/api/agent/v1/runs': {
              post: {
                operationId: 'applicationNativeRun',
                responses: {
                  '200': { description: 'ok' }
                }
              }
            }
          },
          components: {}
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname ===
        '/api/runtime/models/application_run_log_summaries/list'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            items: [styleBoundaryApplicationRunRecord],
            total: 1,
            page: 1,
            page_size: 20
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/applications/app-1/logs/runs'
    ) {
      return new Response(
        JSON.stringify({
          data: {
            items: [styleBoundaryApplicationRunRecord],
            total: 1,
            page: 1,
            page_size: 20
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (url.endsWith('/api/console/applications/catalog')) {
      return new Response(
        JSON.stringify({
          data: {
            types: [
              {
                value: 'agent_flow',
                label: 'AgentFlow'
              }
            ],
            workflow_triggers: [
              {
                value: 'extension',
                label: 'Extension'
              },
              {
                value: 'schedule',
                label: 'Schedule'
              }
            ],
            tags: []
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (url.includes('/api/console/applications/app-1')) {
      return new Response(
        JSON.stringify({
          data: {
            id: 'app-1',
            application_type: 'agent_flow',
            name: 'Support Agent',
            description: 'customer support',
            icon: 'RobotOutlined',
            icon_type: 'iconfont',
            icon_background: '#E6F7F2',
            created_by: 'user-1',
            updated_at: '2026-04-15T09:00:00Z',
            tags: [],
            sections: {
              orchestration: {
                status: 'planned',
                subject_kind: 'agent_flow',
                subject_status: 'unconfigured',
                current_subject_id: null,
                current_draft_id: null
              },
              api: {
                status: 'planned',
                credential_kind: 'application_api_key',
                invoke_routing_mode: 'api_key_bound_application',
                invoke_path_template: null,
                api_capability_status: 'planned',
                credentials_status: 'planned'
              },
              logs: {
                status: 'planned',
                runs_capability_status: 'planned',
                run_object_kind: 'application_run',
                log_retention_status: 'planned'
              },
              monitoring: {
                status: 'planned',
                metrics_capability_status: 'planned',
                metrics_object_kind: 'application_metrics',
                tracing_config_status: 'planned'
              }
            }
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    if (url.endsWith('/api/console/applications')) {
      return new Response(
        JSON.stringify({
          data: [
            {
              id: 'app-1',
              application_type: 'agent_flow',
              name: 'Support Agent',
              description: 'customer support',
              icon: 'RobotOutlined',
              icon_type: 'iconfont',
              icon_background: '#E6F7F2',
              created_by: 'user-1',
              updated_at: '2026-04-15T09:00:00Z',
              tags: []
            }
          ],
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      );
    }

    return originalFetch(input as RequestInfo, init);
  };
}

export function seedStyleBoundaryFrontstageFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  styleBoundaryOriginalFetch ??= globalThis.fetch.bind(globalThis);
  const originalFetch = styleBoundaryOriginalFetch;

  globalThis.fetch = async (input, init) => {
    const url = getStyleBoundaryRequestUrl(input);
    const method = getStyleBoundaryMethod(input, init);
    const requestUrl = parseStyleBoundaryRequestUrl(url);
    const commonResponse = getStyleBoundaryCommonResponse(requestUrl, method);

    if (commonResponse) {
      return commonResponse;
    }

    if (requestUrl.pathname === '/api/console/frontend-blocks') {
      return new Response(JSON.stringify({ data: [], meta: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      });
    }

    return originalFetch(input as RequestInfo, init);
  };
}

export { createStyleBoundaryFrontstagePageContent } from './scene-fixtures/frontstage-content';
