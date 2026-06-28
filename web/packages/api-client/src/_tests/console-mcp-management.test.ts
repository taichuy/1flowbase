import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleMcpInstance,
  createConsoleMcpTool,
  createConsoleMcpToolBinding,
  deleteConsoleMcpGroup,
  deleteConsoleMcpInstance,
  deleteConsoleMcpTool,
  deleteConsoleMcpToolBinding,
  exportConsoleMcpCatalog,
  exportConsoleMcpInstanceDirectory,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpInterfaceCapabilities,
  fetchConsoleMcpListItems,
  fetchConsoleMcpTool,
  refreshConsoleMcpToolDescription,
  updateConsoleMcpInstance,
  updateConsoleMcpMetaToolConfig,
  updateConsoleMcpTool,
  updateConsoleMcpToolBinding,
  upsertConsoleMcpGroup
} from '../console-mcp-management';

describe('console-mcp-management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchVoid').mockImplementation(
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
      name: 'instance directory export package',
      request: () => exportConsoleMcpInstanceDirectory(),
      expected: { path: '/api/console/mcp/instances/export' }
    },
    {
      name: 'single tool',
      request: () => fetchConsoleMcpTool('runtime/get'),
      expected: { path: '/api/console/mcp/tools/runtime%2Fget' }
    }
  ])('reads the $name route', async ({ request, expected }) => {
    await expect(request()).resolves.toMatchObject(expected);
  });

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
      name: 'tool creation',
      request: () =>
        createConsoleMcpTool(
          {
            tool_id: 'get_runtime',
            des_id: 'des12345',
            name: 'Get Runtime',
            short_description: 'Runtime profile',
            usage_description: null,
            full_description: 'Read runtime profile',
            interface_id: 'get_runtime_profile',
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
            usage_description: null,
            full_description: 'Read runtime profile',
            interface_id: 'get_runtime_profile',
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
      name: 'meta tool config update',
      request: () =>
        updateConsoleMcpMetaToolConfig(
          {
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 128,
            list_return_fields: ['path', 'name'],
            get_include_mapping_summary: true,
            get_include_interface_summary: true,
            call_default_des_id_policy: 'required',
            call_high_risk_requires_des_id: true,
            call_validation_error_format: 'field_errors'
          },
          'csrf-123'
        ),
      expected: {
        path: '/api/console/mcp/meta-tool-config',
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
