import type { ComponentProps } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ConsoleDataModel,
  ConsoleDataSource
} from '@1flowbase/api-client';
import { resetAuthStore } from '../../../../../state/auth-store';
import { normalizeFrontstageBlockCatalog } from '../../../lib/block-catalog';
import {
  FORM_BLOCK_CONTRIBUTION,
  TABLE_BLOCK_CONTRIBUTION
} from '../../../lib/data-blocks/types';
import { CreateDataBlockMenu } from '../CreateDataBlockMenu';

const api = vi.hoisted(() => ({
  fetchConsoleDataModels: vi.fn(),
  fetchConsoleDataSources: vi.fn()
}));
vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  ...api
}));

const entries = normalizeFrontstageBlockCatalog(
  [
    ['frontstage.js-ui-block', 'JS'],
    [TABLE_BLOCK_CONTRIBUTION, 'Table'],
    [FORM_BLOCK_CONTRIBUTION, 'Form']
  ].map(([contribution_code, title]) => ({
    installation_id: 'builtin-installation',
    provider_code: '1flowbase',
    plugin_id: 'builtin-frontstage',
    plugin_version: '1.0.0',
    contribution_code,
    title,
    runtime: 'native_react',
    entry: 'index.js',
    code_modules: [],
    code_template: 'export default function Block() { return null; }',
    code_template_version: '1',
    code_template_language: 'tsx' as const,
    context_contract: { primitives: [], input_schema: {} },
    permissions: { network: 'none', storage: 'none', secrets: 'none' },
    ui_capabilities: [],
    frontend_contribution_id: contribution_code,
    frontend_block_id: contribution_code,
    frontend_block_version: '1.0.0',
    runtime_kind: 'trusted_native' as const,
    execution_kind: 'ui_mount' as const,
    isolation_requirement: 'trusted_host_realm' as const,
    requested_permissions: [],
    granted_permissions: [],
    workspace_id: 'workspace-1',
    lifecycle_kind: 'workspace_assignment' as const,
    graph_fingerprint: 'catalog-fixture',
    provenance: {
      module_id: 'builtin-frontstage',
      module_version: '1.0.0',
      module_kind: 'trusted_host' as const
    },
    disable_reason: null
  }))
).items;

function source(id: string, display_name: string): ConsoleDataSource {
  const common = {
    display_name,
    status: 'available',
    enabled: true,
    default_data_model_status: 'published' as const,
    capabilities: {
      can_update_defaults: false,
      can_create_data_model: true,
      can_validate: false,
      can_discover_resources: false,
      can_preview_resources: false,
      can_map_resources: false
    }
  };
  return id === 'main'
    ? {
        ...common,
        id: 'main',
        fixed: true,
        backend: { kind: 'core', durable_backend: 'postgres' }
      }
    : {
        ...common,
        id,
        fixed: false,
        backend: {
          kind: 'runtime_extension',
          installation_id: 'external-installation',
          source_code: 'crm',
          config_json: {},
          secret_ref: null,
          secret_version: null,
          catalog_refresh_status: null,
          catalog_last_error_message: null,
          catalog_refreshed_at: null
        }
      };
}

function model(
  id: string,
  code: string,
  title: string,
  data_source_id: string | null,
  overrides: Partial<ConsoleDataModel> = {}
): ConsoleDataModel {
  return {
    id,
    code,
    title,
    data_source_id,
    scope_kind: 'workspace',
    scope_id: 'workspace-1',
    description: null,
    status: 'published',
    runtime_availability: 'available',
    source_kind: data_source_id ? 'external_source' : 'main_source',
    external_resource_key: null,
    external_table_id: null,
    template_provider: '',
    template_code: '',
    template_version: '',
    template_summary: null,
    physical_table_name: code,
    acl_namespace: 'model',
    audit_namespace: 'model',
    builtin_kind: null,
    capabilities: {
      can_delete: false,
      can_add_user_field: false,
      can_update_lifecycle_status: false,
      record: {
        can_list: true,
        can_get: true,
        can_create: true,
        can_update: true,
        can_delete: false
      }
    },
    fields: [],
    ...overrides
  };
}

function setup(
  props: Partial<ComponentProps<typeof CreateDataBlockMenu>> = {}
) {
  const onCreate = vi.fn();
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } }
  });
  render(
    <QueryClientProvider client={client}>
      <CreateDataBlockMenu
        workspaceId="workspace-1"
        entries={entries}
        disabled={false}
        loading={false}
        onCreate={onCreate}
        {...props}
      />
    </QueryClientProvider>
  );
  return { onCreate };
}

async function openType(title: string) {
  fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
  const item = await screen.findByRole('menuitem', { name: title });
  fireEvent.mouseEnter(item);
}

beforeEach(() => {
  resetAuthStore();
  api.fetchConsoleDataModels
    .mockReset()
    .mockResolvedValue([
      model('main-customers', 'local_customers', '客户', null),
      model('crm-customers', 'crm_customers', '客户', 'crm-source')
    ]);
  api.fetchConsoleDataSources
    .mockReset()
    .mockResolvedValue([
      source('main', '主数据源'),
      source('crm-source', 'CRM 数据源')
    ]);
});

describe('CreateDataBlockMenu catalog and model selection', () => {
  it('offers JS, table, and form entries without creating a block on open', async () => {
    const { onCreate } = setup();
    expect(api.fetchConsoleDataModels).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
    expect(
      await screen.findByRole('menuitem', { name: 'JS 区块' })
    ).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: '表格' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: '表单' })).toBeInTheDocument();
    expect(onCreate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('menuitem', { name: 'JS 区块' }));
    expect(onCreate).toHaveBeenCalledExactlyOnceWith(entries[0], undefined);
  });

  it.each([
    { title: '表格', contribution: TABLE_BLOCK_CONTRIBUTION },
    { title: '表单', contribution: FORM_BLOCK_CONTRIBUTION }
  ])(
    'binds $title to the explicitly selected source even when model titles match',
    async ({ title, contribution }) => {
      const { onCreate } = setup();
      await openType(title);
      const sourceItem = await screen.findByRole('menuitem', {
        name: 'CRM 数据源'
      });
      expect(
        screen.getByRole('menuitem', { name: '主数据源' })
      ).toBeInTheDocument();
      expect(onCreate).not.toHaveBeenCalled();
      fireEvent.mouseEnter(sourceItem);
      fireEvent.click(await screen.findByRole('menuitem', { name: '客户' }));
      expect(onCreate).toHaveBeenCalledExactlyOnceWith(
        entries.find((entry) => entry.contributionCode === contribution),
        {
          model_id: 'crm-customers',
          model_code: 'crm_customers',
          data_source_id: 'crm-source'
        }
      );
      expect(api.fetchConsoleDataModels).toHaveBeenCalledWith(
        {},
        expect.any(String)
      );
      expect(api.fetchConsoleDataSources).toHaveBeenCalledWith(
        expect.any(String)
      );
    }
  );

  it('keeps the backend null data source when selecting a model from the sole source', async () => {
    api.fetchConsoleDataModels.mockResolvedValue([
      model('orders-id', 'orders', '订单', null)
    ]);
    api.fetchConsoleDataSources.mockResolvedValue([source('main', '主数据源')]);
    const { onCreate } = setup();
    await openType('表格');
    fireEvent.click(await screen.findByRole('menuitem', { name: '订单' }));
    expect(onCreate).toHaveBeenCalledExactlyOnceWith(entries[1], {
      model_id: 'orders-id',
      model_code: 'orders',
      data_source_id: null
    });
  });

  it('filters unpublished and unreadable models using backend capabilities', async () => {
    const unreadable = model('unreadable', 'private', '无读取权限', null);
    unreadable.capabilities.record.can_list = false;
    api.fetchConsoleDataModels.mockResolvedValue([
      model('published', 'orders', '可用订单', null),
      model('draft', 'draft', '未发布数据表', null, { status: 'draft' }),
      unreadable
    ]);
    api.fetchConsoleDataSources.mockResolvedValue([source('main', '主数据源')]);
    const { onCreate } = setup();
    await openType('表单');
    expect(
      await screen.findByRole('menuitem', { name: '可用订单' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: '未发布数据表' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: '无读取权限' })
    ).not.toBeInTheDocument();
    expect(onCreate).not.toHaveBeenCalled();
  });

  it('searches backend model codes without selecting or creating a model', async () => {
    api.fetchConsoleDataModels.mockResolvedValue([
      model('orders', 'orders', '订单', null),
      model('customers', 'customers', '客户', null)
    ]);
    api.fetchConsoleDataSources.mockResolvedValue([source('main', '主数据源')]);
    const { onCreate } = setup();
    await openType('表格');
    await screen.findByRole('menuitem', { name: '订单' });
    fireEvent.change(screen.getByRole('textbox', { name: '搜索数据表' }), {
      target: { value: 'CUSTOM' }
    });
    await waitFor(() =>
      expect(
        screen.queryByRole('menuitem', { name: '订单' })
      ).not.toBeInTheDocument()
    );
    expect(screen.getByRole('menuitem', { name: '客户' })).toBeInTheDocument();
    expect(onCreate).not.toHaveBeenCalled();
  });

  it('does not open or query models while creation is disabled', () => {
    const { onCreate } = setup({ disabled: true });
    fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
    expect(
      screen.queryByRole('menuitem', { name: '表格' })
    ).not.toBeInTheDocument();
    expect(api.fetchConsoleDataModels).not.toHaveBeenCalled();
    expect(api.fetchConsoleDataSources).not.toHaveBeenCalled();
    expect(onCreate).not.toHaveBeenCalled();
  });
});
