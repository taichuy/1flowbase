import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { useAuthStore } from '../state/auth-store';
import { i18nText } from '../shared/i18n/text';
import {
  modelProviderCatalogContract,
  modelProviderOptionsContract,
  primaryContractProviderEnabledModelIds
} from '../test/model-provider-contract-fixtures';

const styleBoundaryProviderInstances = [
  {
    id: 'provider-openai-prod',
    installation_id: 'installation-openai-compatible',
    provider_code: 'openai_compatible',
    protocol: 'openai_responses',
    display_name: 'OpenAI Production',
    status: 'ready',
    config_json: {
      base_url: 'https://api.openai.com/v1',
      organization: 'workspace-prod'
    },
    enabled_model_ids: primaryContractProviderEnabledModelIds,
    catalog_refresh_status: 'succeeded',
    catalog_last_error_message: null,
    catalog_refreshed_at: '2026-04-18T16:01:00Z',
    model_count: primaryContractProviderEnabledModelIds.length
  }
];

export const styleBoundaryNodeContributions = [
  {
    installation_id: 'installation-1',
    provider_code: 'prompt_pack',
    plugin_id: 'prompt_pack@0.1.0',
    plugin_version: '0.1.0',
    contribution_code: 'openai_prompt',
    node_shell: 'action',
    plugin_unique_identifier: 'prompt_pack',
    package_id: 'prompt_pack@0.1.0',
    contribution_checksum: 'sha256:contribution',
    compiled_contribution_hash: 'sha256:compiled',
    category: 'generation',
    title: 'OpenAI Prompt',
    description: 'Generate prompt output',
    dependency_status: 'ready',
    schema_version: '1flowbase.node-contribution/v2',
    output_schema_snapshot: {
      outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
    },
    experimental: false,
    icon: 'sparkles',
    schema_ui: {},
    output_schema: {
      outputs: [{ key: 'answer', title: 'Answer', valueType: 'string' }]
    },
    side_effect_policy: 'external_read',
    infra_contracts: [],
    required_auth: [],
    visibility: 'public',
    dependency_installation_kind: 'model_provider',
    dependency_plugin_version_range: '^0.1.0'
  }
];

const styleBoundaryApplicationRunRecord = {
  id: 'run-1',
  application_id: 'app-1',
  scope_id: 'workspace-1',
  run_mode: 'debug_flow_run',
  status: 'succeeded',
  target_node_id: null,
  title: 'Boundary run',
  expand_id: 'boundary-expand',
  external_user: null,
  authorized_account: 'root',
  api_key_id: null,
  api_key_name_snapshot: null,
  publication_version_id: null,
  external_conversation_id: null,
  external_trace_id: null,
  compatibility_mode: null,
  idempotency_key: null,
  total_tokens: 128,
  unique_node_count: 3,
  tool_callback_count: 0,
  started_at: '2026-05-10T09:00:00Z',
  finished_at: '2026-05-10T09:00:03Z',
  created_at: '2026-05-10T09:00:00Z',
  updated_at: '2026-05-10T09:00:03Z'
};

function expandDottedBundle(bundle: Record<string, string>) {
  const expanded: Record<string, unknown> = {};

  for (const [dottedKey, value] of Object.entries(bundle)) {
    const segments = dottedKey.split('.');
    let current = expanded;

    for (const segment of segments.slice(0, -1)) {
      const next = current[segment];
      if (typeof next === 'object' && next !== null) {
        current = next as Record<string, unknown>;
        continue;
      }

      const created: Record<string, unknown> = {};
      current[segment] = created;
      current = created;
    }

    current[segments[segments.length - 1]!] = value;
  }

  return expanded;
}

const styleBoundaryPluginI18nCatalog = Object.fromEntries(
  Object.entries(modelProviderCatalogContract.i18n_catalog).map(
    ([namespace, locales]) => [
      namespace,
      Object.fromEntries(
        Object.entries(locales as Record<string, Record<string, string>>).map(
          ([locale, bundle]) => [locale, expandDottedBundle(bundle)]
        )
      )
    ]
  )
);

const styleBoundaryPluginFamiliesCatalog = {
  locale_meta: modelProviderCatalogContract.locale_meta,
  i18n_catalog: styleBoundaryPluginI18nCatalog,
  entries: modelProviderCatalogContract.entries.map((entry) => ({
    provider_code: entry.provider_code,
    plugin_type: 'model_provider',
    namespace: entry.namespace,
    label_key: entry.label_key,
    description_key: entry.description_key,
    provider_label_key: entry.label_key,
    protocol: entry.protocol,
    help_url: entry.help_url,
    default_base_url: entry.default_base_url,
    model_discovery_mode: entry.model_discovery_mode,
    current_installation_id: entry.installation_id,
    current_version: entry.plugin_version,
    latest_version: entry.plugin_version,
    has_update: false,
    installed_versions: [
      {
        installation_id: entry.installation_id,
        plugin_version: entry.plugin_version,
        source_kind: 'official_registry',
        trust_level: 'verified_official',
        created_at: '2026-04-20T10:00:00Z',
        is_current: true
      }
    ]
  }))
};

const styleBoundaryOfficialPluginCatalog = {
  source_kind: 'official_registry',
  source_label: i18nText('appShell', 'auto.official_source'),
  registry_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/latest/download/official-registry.json',
  locale_meta: modelProviderCatalogContract.locale_meta,
  page: {
    limit: 20,
    next_cursor: null
  },
  entries: [
    {
      plugin_id: '1flowbase.openai_compatible',
      provider_code: 'openai_compatible',
      plugin_type: 'model_provider',
      display_name: 'OpenAI Compatible',
      description: 'Provider plugin for OpenAI-compatible APIs.',
      protocol: 'openai_responses',
      latest_version: '0.1.0',
      selected_artifact: {
        os: 'linux',
        arch: 'x64',
        libc: 'gnu',
        rust_target: 'x86_64-unknown-linux-gnu',
        download_url: 'https://example.com/openai-compatible.tar.gz',
        checksum: 'openai-compatible-checksum',
        signature_algorithm: null,
        signing_key_id: null
      },
      help_url:
        'https://github.com/taichuy/1flowbase-official-plugins/tree/main/models/openai_compatible',
      model_discovery_mode: 'hybrid',
      install_status: 'assigned'
    }
  ]
};

export function seedStyleBoundaryAuth() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'style-boundary-csrf',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'manager',
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
        'route_page.view.all',
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
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-15T09:00:00Z',
      document: createStyleBoundaryAgentFlowDocument()
    },
    versions: [],
    autosave_interval_seconds: 30
  };
}

export function seedStyleBoundarySettingsFetch() {
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
      requestUrl.pathname === '/api/console/model-providers/catalog'
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
      url.endsWith('/api/console/model-providers')
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
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '2026-04-15T09:10:00Z',
              document: currentDraftDocument
            },
            versions: [],
            autosave_interval_seconds: 30
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
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '2026-04-15T09:00:00Z',
              document: currentDraftDocument
            },
            versions: [],
            autosave_interval_seconds: 30
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
                id: 'application-native-run-operation',
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
        '/api/runtime/models/application_run_log_summaries/records'
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
            types: [{ value: 'agent_flow', label: 'AgentFlow' }],
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

export function createStyleBoundaryFrontstagePageContent() {
  return {
    page: {
      id: 'page-1',
      title: 'Landing',
      kind: 'page' as const,
      parentId: null,
      rank: '001000',
      schemaRootUid: 'root-1'
    },
    schema: {
      rootUid: 'root-1',
      payload: { blocks: [] }
    },
    root: {
      uid: 'root-1',
      payload: { blocks: [] }
    }
  };
}
