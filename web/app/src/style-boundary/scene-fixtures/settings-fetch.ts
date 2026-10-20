import { styleBoundaryOrganizationMembers } from './settings/organization';
import { getStyleBoundaryOriginalFetch } from './original-fetch';
import {
  createStyleBoundaryJsonResponse,
  getStyleBoundaryRequestUrl,
  getStyleBoundaryMethod,
  parseStyleBoundaryRequestUrl,
  getStyleBoundaryCommonResponse
} from './common-fetch';

import {
  modelProviderCatalogContract,
  modelProviderOptionsContract
} from '../../test/model-provider-contract-fixtures';
import { createSettingsI18nCatalogTestServer } from '../../features/settings/pages/i18n-catalog/_tests/i18n-catalog-test-fixture';

import {
  styleBoundaryMcpCatalog,
  styleBoundaryMcpInterfaceCapabilities,
  styleBoundaryOfficialPluginCatalog,
  styleBoundaryPluginFamiliesCatalog,
  styleBoundaryProviderInstances
} from './settings/catalogs';

export function seedStyleBoundarySettingsFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  const originalFetch = getStyleBoundaryOriginalFetch();
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
      requestUrl.origin === window.location.origin &&
      method.toUpperCase() === 'GET'
    ) {
      if (
        requestUrl.pathname === '/api/console/settings/members' &&
        !requestUrl.search
      ) {
        return createStyleBoundaryJsonResponse({
          data: styleBoundaryOrganizationMembers,
          meta: null
        });
      }
      if (
        requestUrl.pathname === '/api/console/mcp/bundles/export-defaults' &&
        !requestUrl.search
      ) {
        return createStyleBoundaryJsonResponse({
          data: { current_system_version: '0.2.6' },
          meta: null
        });
      }
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
              service: 'runtime-extension-host',
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
              services: ['api-server', 'runtime-extension-host']
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
              target_id: 'runtime-extension-host',
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
      requestUrl.pathname === '/api/console/system/runtime-processes'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          process_total: 2,
          processes: [
            {
              pid: 1442117,
              parent_pid: 1,
              name: 'api-server',
              command: './target/debug/api-server',
              user: 'taichuy',
              status: 'sleeping',
              cpu_usage_percent: 0.85,
              cpu_usage_single_core_percent: 6.8,
              memory_bytes: 33_554_432,
              memory_usage_percent: 3.2,
              start_time_unix_seconds: 1_758_160_251,
              terminable: true,
              backend_process: true
            },
            {
              pid: 1619155,
              parent_pid: 1,
              name: 'MainThread',
              command: 'node dsh --profile web',
              user: 'taichuy',
              status: 'running',
              cpu_usage_percent: 0.57,
              cpu_usage_single_core_percent: 4.56,
              memory_bytes: 25_165_824,
              memory_usage_percent: 2.84,
              start_time_unix_seconds: 1_758_217_368,
              terminable: false,
              backend_process: false
            }
          ]
        },
        meta: null
      });
    }

    if (
      method.toUpperCase() === 'GET' &&
      requestUrl.pathname === '/api/console/settings/ui-management/components'
    ) {
      return createStyleBoundaryJsonResponse({
        data: [
          {
            id: '019c0000-0000-7000-8000-000000000001',
            scope_id: '00000000-0000-0000-0000-000000000000',
            component_code: 'official.data-table',
            name: 'DataTable',
            description: 'Data table component',
            import_code:
              "import { DataTable } from '@1flowbase/native-components';",
            source_code: '<DataTable />',
            origin: 'official',
            source: 'official',
            group: 'data',
            upstream: {
              identity: '@1flowbase/native-components',
              version: '1.0.0'
            },
            version: '1.0.0',
            keywords: ['table'],
            catalog_updated_at: null,
            source_locator: null,
            source_checksum: null,
            created_at: '2026-08-23T00:00:00Z',
            updated_at: '2026-08-23T00:00:00Z'
          }
        ],
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
      requestUrl.pathname === '/api/console/settings/system-backups'
    ) {
      return createStyleBoundaryJsonResponse({
        data: {
          items: [
            {
              backup_set_id: '0198f8e1-21e0-7000-8000-000000000001',
              exact_backup_name: '0198f8e1-21e0-7000-8000-000000000001',
              created_at: '2026-08-12T08:00:00Z',
              availability: 'ready',
              total_size_bytes: 2254857830,
              envelope_digest: 'a'.repeat(64)
            }
          ]
        },
        meta: null
      });
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
