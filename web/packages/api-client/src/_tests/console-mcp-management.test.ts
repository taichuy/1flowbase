import { describe, expect, expectTypeOf, test, vi } from 'vitest';
import * as transport from '../transport';
import type {
  ConsoleMcpTool,
  ConsoleMcpToolAvailabilityStatus
} from '../console-mcp-management';

import {
  createConsoleMcpInstance,
  copyConsoleMcpInstance,
  createConsoleMcpTool,
  createConsoleMcpToolBinding,
  deleteConsoleMcpGroup,
  deleteConsoleMcpInstance,
  deleteConsoleMcpTool,
  deleteConsoleMcpToolBinding,
  deleteConsoleMcpTemplateLibraryRelease,
  executeConsoleMcpToolDebug,
  exportConsoleMcpBundle,
  exportConsoleMcpInstanceBundle,
  exportConsoleMcpCatalog,
  fetchConsoleMcpBundleExportDefaults,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpInstanceDiscoveryPolicy,
  fetchConsoleMcpInterfaceCapabilities,
  fetchConsoleMcpListItems,
  fetchConsoleMcpTemplateLibrary,
  fetchConsoleOfficialMcpBundles,
  fetchConsoleMcpTool,
  moveConsoleMcpGroup,
  importConsoleMcpBundle,
  importConsoleMcpTemplateLibraryBundle,
  importConsoleOfficialMcpBundle,
  previewConsoleMcpBundle,
  previewConsoleMcpTemplateLibraryBundle,
  previewConsoleOfficialMcpBundle,
  refreshConsoleMcpToolDescription,
  repairConsoleMcpTemplateLibraryRelease,
  setConsoleMcpTemplateLibraryCurrentVersion,
  syncConsoleMcpTemplateLibraryBundle,
  updateConsoleMcpInstance,
  updateConsoleMcpInstanceDiscoveryPolicy,
  updateConsoleMcpTool,
  updateConsoleMcpToolBinding,
  upsertConsoleMcpGroup
} from '../console-mcp-management';

describe('console-mcp-management client', () => {
  test('AC-016 keeps availability_status as the exact public DTO union', () => {
    expectTypeOf<
      ConsoleMcpTool['availability_status']
    >().toEqualTypeOf<ConsoleMcpToolAvailabilityStatus>();
  });

  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchBlob').mockImplementation(
    async (input) => input as never
  );

  test.each([
    {
      name: 'catalog',
      request: () => fetchConsoleMcpCatalog(),
      expected: { path: '/api/console/mcp/catalog' }
    },
    {
      name: 'interface capabilities with bindable filter',
      request: () =>
        fetchConsoleMcpInterfaceCapabilities({ bindable_only: true }),
      expected: {
        path: '/api/console/mcp/interface-capabilities?bindable_only=true'
      }
    },
    {
      name: 'mcp list items',
      request: () =>
        fetchConsoleMcpListItems({
          instance_id: 'workspace_ops',
          path: '/ops',
          path_regex: '^/ops',
          limit: 25
        }),
      expected: {
        path: '/api/console/mcp/list?instance_id=workspace_ops&path=%2Fops&path_regex=%5E%2Fops&limit=25'
      }
    },
    {
      name: 'export package',
      request: () => exportConsoleMcpCatalog(),
      expected: { path: '/api/console/mcp/export' }
    },
    {
      name: 'official MCP bundles',
      request: () => fetchConsoleOfficialMcpBundles(),
      expected: { path: '/api/console/mcp/bundles/official' }
    },
    {
      name: 'MCP template library',
      request: () => fetchConsoleMcpTemplateLibrary(),
      expected: { path: '/api/console/mcp/bundles/library' }
    },
    {
      name: 'MCP bundle export defaults',
      request: () => fetchConsoleMcpBundleExportDefaults(),
      expected: { path: '/api/console/mcp/bundles/export-defaults' }
    },
    {
      name: 'single tool',
      request: () => fetchConsoleMcpTool('runtime/get'),
      expected: { path: '/api/console/mcp/tools/runtime%2Fget' }
    },
    {
      name: 'instance discovery policy',
      request: () => fetchConsoleMcpInstanceDiscoveryPolicy('workspace/ops'),
      expected: {
        path: '/api/console/mcp/instances/workspace%2Fops/discovery-policy'
      }
    }
  ])('reads the $name route', async ({ request, expected }) => {
    await expect(request()).resolves.toMatchObject(expected);
  });

  test('exports a complete MCP bundle as a blob', async () => {
    await expect(
      exportConsoleMcpBundle(
        {
          organization: 'taichuy',
          bundle_id: '1flowbase_zh_hans',
          bundle_version: '1.0.0',
          locale: 'zh_Hans'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/bundles/export',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test('exports one MCP instance bundle as a blob', async () => {
    await expect(
      exportConsoleMcpInstanceBundle(
        'workspace/ops',
        {
          organization: 'taichuy',
          bundle_id: 'workspace_ops',
          bundle_version: '1.0.0',
          locale: 'zh_Hans'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/mcp/instances/workspace%2Fops/bundles/export',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });

  test.each([
    {
      name: 'preview',
      request: (file: File) => previewConsoleMcpBundle(file, 'csrf-123'),
      path: '/api/console/mcp/bundles/preview-upload'
    },
    {
      name: 'import',
      request: (file: File) => importConsoleMcpBundle(file, 'csrf-123'),
      path: '/api/console/mcp/bundles/import-upload'
    }
  ])('uploads a local MCP bundle for $name', async ({ request, path }) => {
    const file = new File(['bundle'], 'bundle.zip', {
      type: 'application/zip'
    });
    const result = (await request(file)) as unknown as {
      path: string;
      rawBody: FormData;
      contentType: null;
    };
    expect(result).toMatchObject({ path, method: 'POST', contentType: null });
    expect(result.rawBody.get('file')).toBe(file);
  });

  test.each([
    {
      name: 'preview',
      request: () =>
        previewConsoleOfficialMcpBundle(
          { organization: 'taichuy', bundle_id: '1flowbase_zh_hans' },
          'csrf-123'
        ),
      path: '/api/console/mcp/bundles/preview-official'
    },
    {
      name: 'import',
      request: () =>
        importConsoleOfficialMcpBundle(
          { organization: 'taichuy', bundle_id: '1flowbase_zh_hans' },
          'csrf-123'
        ),
      path: '/api/console/mcp/bundles/import-official'
    }
  ])(
    'uses the backend for official MCP bundle $name',
    async ({ request, path }) => {
      await expect(request()).resolves.toMatchObject({
        path,
        method: 'POST',
        body: { organization: 'taichuy', bundle_id: '1flowbase_zh_hans' },
        csrfToken: 'csrf-123'
      });
    }
  );

  test.each([
    {
      name: 'sync',
      request: () =>
        syncConsoleMcpTemplateLibraryBundle(
          '@taichuy',
          '1flowbase/zh',
          { bundle_version: '1.1.1' },
          'csrf-123'
        ),
      suffix: 'sync',
      method: 'POST'
    },
    {
      name: 'preview',
      request: () =>
        previewConsoleMcpTemplateLibraryBundle(
          '@taichuy',
          '1flowbase/zh',
          { bundle_version: '1.1.1' },
          'csrf-123'
        ),
      suffix: 'preview',
      method: 'POST'
    },
    {
      name: 'import',
      request: () =>
        importConsoleMcpTemplateLibraryBundle(
          '@taichuy',
          '1flowbase/zh',
          { bundle_version: '1.1.1' },
          'csrf-123'
        ),
      suffix: 'import',
      method: 'POST'
    }
  ])(
    'uses the local MCP library for $name',
    async ({ request, suffix, method }) => {
      await expect(request()).resolves.toMatchObject({
        path: `/api/console/mcp/bundles/library/%40taichuy/1flowbase%2Fzh/${suffix}`,
        method,
        body: { bundle_version: '1.1.1' },
        csrfToken: 'csrf-123'
      });
    }
  );

  test.each([
    {
      name: 'set current',
      request: () =>
        setConsoleMcpTemplateLibraryCurrentVersion(
          'taichuy',
          'bundle',
          '1.1.1',
          'csrf-123'
        ),
      suffix: 'current/1.1.1',
      method: 'POST'
    },
    {
      name: 'delete',
      request: () =>
        deleteConsoleMcpTemplateLibraryRelease(
          'taichuy',
          'bundle',
          '1.1.1',
          'csrf-123'
        ),
      suffix: 'releases/1.1.1',
      method: 'DELETE'
    },
    {
      name: 'repair',
      request: () =>
        repairConsoleMcpTemplateLibraryRelease(
          'taichuy',
          'bundle',
          '1.1.1',
          'csrf-123'
        ),
      suffix: 'releases/1.1.1/repair',
      method: 'POST'
    }
  ])(
    'mutates an MCP library release for $name',
    async ({ request, suffix, method }) => {
      await expect(request()).resolves.toMatchObject({
        path: `/api/console/mcp/bundles/library/taichuy/bundle/${suffix}`,
        method,
        csrfToken: 'csrf-123'
      });
    }
  );

  test.each([
    {
      name: 'instance creation',
      request: () =>
        createConsoleMcpInstance(
          {
            instance_id: 'workspace_ops',
            name: 'Workspace Ops',
            description_short: null,
            status: 'enabled',
            default_entry_path: '/'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'instance copy',
      request: () =>
        copyConsoleMcpInstance(
          'source/slash',
          { instance_id: 'copied_ops', name: 'Copied Ops' },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/source%2Fslash/copy',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'instance update',
      request: () =>
        updateConsoleMcpInstance(
          'instance/slash',
          {
            instance_id: 'instance/slash',
            name: 'Slash Instance',
            description_short: null,
            status: 'enabled',
            default_entry_path: '/'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/instance%2Fslash',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'group upsert',
      request: () =>
        upsertConsoleMcpGroup(
          'workspace_ops',
          {
            path: '/ops',
            display_name: 'Operations',
            description_short: null,
            enabled: true,
            sort_order: 0
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops/groups',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'group move',
      request: () =>
        moveConsoleMcpGroup(
          'workspace_ops',
          {
            source_path: '/system_data',
            target_parent_path: '/system_mcp',
            sort_order: 30
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops/groups/move',
        method: 'POST',
        body: {
          source_path: '/system_data',
          target_parent_path: '/system_mcp',
          sort_order: 30
        },
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tool creation',
      request: () =>
        createConsoleMcpTool(
          {
            tool_id: 'get_runtime',
            des_id: 'des12345',
            name: 'Get Runtime',
            short_description: 'Runtime profile',
            full_description: 'Read runtime profile',
            execution_target: {
              kind: 'interface_wrapper',
              interface_id: 'get_runtime_profile'
            },
            parameter_schema: {},
            result_schema: {},
            input_mapping: {},
            output_mapping: {},
            permission_code: 'system_runtime.view.all',
            risk_level: 'high',
            status: 'draft'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/tools',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tool update',
      request: () =>
        updateConsoleMcpTool(
          'runtime.get',
          {
            name: 'Get Runtime',
            des_id: 'des12345',
            short_description: 'Runtime profile',
            full_description: 'Read runtime profile',
            execution_target: {
              kind: 'interface_wrapper',
              interface_id: 'get_runtime_profile'
            },
            parameter_schema: {},
            result_schema: {},
            input_mapping: {},
            output_mapping: {},
            permission_code: 'system_runtime.view.all',
            risk_level: 'high',
            status: 'enabled'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/tools/runtime.get',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'description refresh',
      request: () =>
        refreshConsoleMcpToolDescription('runtime.get', 'csrf-123'),
      expected: {
        path: '/api/console/mcp/tools/runtime.get/description/refresh',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'debug execute',
      request: () =>
        executeConsoleMcpToolDebug(
          {
            interface_id: 'data_model__orders__create_record',
            mcp_arguments: { title: 'Debug order' },
            input_mapping: {
              mappings: [
                {
                  interface_param: 'order_title',
                  mcp_param: 'title',
                  required: true
                }
              ]
            },
            output_mapping: {
              type: 'object',
              properties: {
                order_title: { type: 'string' }
              }
            }
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/debug/execute',
        method: 'POST',
        csrfToken: 'csrf-123',
        body: {
          interface_id: 'data_model__orders__create_record',
          mcp_arguments: { title: 'Debug order' },
          input_mapping: {
            mappings: [
              {
                interface_param: 'order_title',
                mcp_param: 'title',
                required: true
              }
            ]
          },
          output_mapping: {
            type: 'object',
            properties: {
              order_title: { type: 'string' }
            }
          }
        }
      }
    },
    {
      name: 'debug details execute',
      request: () =>
        executeConsoleMcpToolDebug(
          {
            interface_id: 'data_model__orders__create_record',
            debug_response_mode: 'debug_details',
            mcp_arguments: { title: 'Debug order' },
            input_mapping: { mappings: [] },
            output_mapping: { type: 'object' }
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/debug/execute',
        method: 'POST',
        csrfToken: 'csrf-123',
        body: {
          interface_id: 'data_model__orders__create_record',
          debug_response_mode: 'debug_details',
          mcp_arguments: { title: 'Debug order' },
          input_mapping: { mappings: [] },
          output_mapping: { type: 'object' }
        }
      }
    },
    {
      name: 'tool binding creation',
      request: () =>
        createConsoleMcpToolBinding(
          'workspace_ops',
          {
            group_path: '/ops',
            tool_id: 'runtime.get',
            display_alias: null,
            visible: true,
            sort_order: 0
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops/tool-bindings',
        method: 'POST',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tool binding update',
      request: () =>
        updateConsoleMcpToolBinding(
          'binding-1',
          {
            group_path: '/admin',
            tool_id: 'runtime.get',
            display_alias: 'Runtime Admin',
            visible: true,
            sort_order: 1
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/tool-bindings/binding-1',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'instance discovery policy update',
      request: () =>
        updateConsoleMcpInstanceDiscoveryPolicy(
          'workspace_ops',
          {
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 128,
            list_return_fields: ['path', 'name']
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops/discovery-policy',
        method: 'PUT',
        csrfToken: 'csrf-123'
      }
    }
  ])(
    'writes $name through the console mcp route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );

  test.each([
    {
      name: 'instance deletion',
      request: () => deleteConsoleMcpInstance('workspace_ops', 'csrf-123'),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tool deletion',
      request: () => deleteConsoleMcpTool('runtime.get', 'csrf-123'),
      expected: {
        path: '/api/console/mcp/tools/runtime.get',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'group deletion',
      request: () => deleteConsoleMcpGroup('workspace_ops', '/ops', 'csrf-123'),
      expected: {
        path: '/api/console/mcp/instances/workspace_ops/groups?path=%2Fops',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    },
    {
      name: 'tool binding deletion',
      request: () => deleteConsoleMcpToolBinding('binding-1', 'csrf-123'),
      expected: {
        path: '/api/console/mcp/tool-bindings/binding-1',
        method: 'DELETE',
        csrfToken: 'csrf-123'
      }
    }
  ])(
    'deletes $name through the console mcp route',
    async ({ request, expected }) => {
      await expect(request()).resolves.toMatchObject(expected);
    }
  );
});
