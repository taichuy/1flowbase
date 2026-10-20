import { getStyleBoundaryOriginalFetch } from './scene-fixtures/original-fetch';

import {
  createStyleBoundaryJsonResponse,
  getStyleBoundaryRequestUrl,
  getStyleBoundaryMethod,
  parseStyleBoundaryRequestUrl,
  getStyleBoundaryCommonResponse
} from './scene-fixtures/common-fetch';
export { seedStyleBoundaryCommonFetch } from './scene-fixtures/common-fetch';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { useAuthStore } from '../state/auth-store';
import { modelProviderOptionsContract } from '../test/model-provider-contract-fixtures';

import {
  styleBoundaryApplicationRunRecord,
  styleBoundaryMcpCatalog,
  styleBoundaryMcpInterfaceCapabilities,
  styleBoundaryNodeContributions
} from './scene-fixtures/settings/catalogs';

export { styleBoundaryMcpCatalog, styleBoundaryMcpInterfaceCapabilities };

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

function createStyleBoundaryAgentFlowDocument() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
  const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

  if (llmNode) {
    llmNode.config = {
      ...llmNode.config,
      provider_instance_id: 'provider-openai-prod',
      model: 'gpt-4o-mini',
      temperature: 0.7,
      mcp_instance_ids: ['workspace_ops', 'workspace_ops']
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

  const originalFetch = getStyleBoundaryOriginalFetch();

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

export function seedStyleBoundaryApplicationFetch() {
  if (typeof globalThis.fetch !== 'function') {
    return;
  }

  const originalFetch = getStyleBoundaryOriginalFetch();
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
      requestUrl.pathname === '/api/console/mcp/catalog'
    ) {
      return createStyleBoundaryJsonResponse({
        data: styleBoundaryMcpCatalog,
        meta: null
      });
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
        '/api/runtime/models/application_run_log_tasks/list'
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

  const originalFetch = getStyleBoundaryOriginalFetch();

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

export { seedStyleBoundarySettingsFetch } from './scene-fixtures/settings-fetch';
