import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../../../lib/block-catalog';
import {
  DataBlockHostContext,
  type DataBlockHost
} from '../../../../lib/data-blocks/host-context';
import {
  createFormBlockConfig,
  FORM_BLOCK_CONTRIBUTION,
  type FormBlockConfig
} from '../../../../lib/data-blocks/types';
import { FormBlock } from '../FormBlock';

const mocks = vi.hoisted(() => ({
  metadata: vi.fn(),
  record: vi.fn(),
  writes: vi.fn(),
  catalog: vi.fn(),
  models: vi.fn(),
  sources: vi.fn()
}));
vi.mock('../../../../hooks/use-frontstage-block-catalog', () => ({
  useFrontstageBlockCatalog: mocks.catalog
}));
vi.mock('../../../../api/data-blocks', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../api/data-blocks')>()),
  useDataBlockMetadata: mocks.metadata,
  useDataBlockRecord: mocks.record,
  useDataBlockWrites: mocks.writes
}));
vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  fetchConsoleDataModels: mocks.models,
  fetchConsoleDataSources: mocks.sources
}));

const binding = {
  model_id: 'orders',
  model_code: 'orders',
  data_source_id: null
};
const otherBinding = {
  model_id: 'customers',
  model_code: 'customers',
  data_source_id: null
};
const formEntry: NormalizedFrontstageBlockCatalogEntry = {
  id: 'form',
  contributionCode: FORM_BLOCK_CONTRIBUTION,
  title: 'Form',
  runtimeKind: 'native_react',
  installationId: 'builtin',
  providerCode: '1flowbase',
  pluginId: 'frontstage',
  pluginVersion: '1',
  entry: 'index.js',
  permissions: { network: 'none', storage: 'none', secrets: 'none' },
  contextContract: { primitives: [], inputSchema: {} },
  uiCapabilities: [],
  codeCapabilities: {
    template: {
      source: 'export default function Form() {return null}',
      version: '1',
      language: 'tsx'
    }
  },
  raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
};
const metadata = {
  model_id: 'orders',
  model_code: 'orders',
  record_id_field: 'id',
  fields: [],
  record_capabilities: {
    can_list: true,
    can_get: true,
    can_create: true,
    can_update: true,
    can_delete: false
  }
};
function setup(
  mode: FormBlockConfig['mode'] = 'create',
  design = true,
  override: Partial<FormBlockConfig> = {}
) {
  const config: FormBlockConfig = {
    ...createFormBlockConfig(binding, mode),
    initialized: false,
    source_block_id: 'source-table',
    source_model_code: 'orders',
    surface_size: 'large',
    title: 'Custom form title',
    relation: {
      field_code: 'customer',
      parent_model_code: 'customers',
      parent_record_input: 'customer_id'
    },
    ...override
  };
  const saveConfig = vi.fn().mockResolvedValue(undefined);
  const host: DataBlockHost = {
    block_id: 'child',
    design_mode: design,
    saving: false,
    targets: [],
    saveConfig,
    prepareAction: vi.fn(),
    openAction: vi.fn(),
    close: vi.fn()
  };
  const ctx = {
    workspace: { id: 'workspace' },
    currentUser: { id: 'user', permissions: [] },
    props: { data_block: config },
    inputs: { record_id: 'A' },
    outputs: { publish: vi.fn() }
  } as unknown as BlockContext;
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } }
  });
  render(
    <QueryClientProvider client={client}>
      <DataBlockHostContext.Provider value={host}>
        <FormBlock ctx={ctx} />
      </DataBlockHostContext.Provider>
    </QueryClientProvider>
  );
  return { config, saveConfig };
}
async function openFormMenu() {
  fireEvent.click(screen.getByRole('button', { name: '创建区块' }));
  fireEvent.mouseEnter(
    await screen.findByRole('menuitem', { name: /^表单(?: right)?$/ })
  );
  await screen.findByRole('menuitem', { name: '当前数据表' });
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.metadata.mockReturnValue({
    data: metadata,
    isLoading: false,
    isError: false
  });
  mocks.record.mockReturnValue({
    data: { id: 'A' },
    isLoading: false,
    isError: false
  });
  mocks.writes.mockReturnValue({
    create: { mutateAsync: vi.fn() },
    update: { mutateAsync: vi.fn() }
  });
  mocks.catalog.mockReturnValue({
    items: [
      formEntry,
      { ...formEntry, id: 'table', contributionCode: 'frontstage.table-block' }
    ],
    loading: false,
    error: null,
    refetch: vi.fn()
  });
  mocks.models.mockResolvedValue([
    {
      id: 'customers',
      code: 'customers',
      title: '其他客户',
      data_source_id: null,
      status: 'published',
      capabilities: { record: { can_list: true } }
    }
  ]);
  mocks.sources.mockResolvedValue([{ id: 'main', display_name: 'Main' }]);
});

describe('action child form initialization', () => {
  it('waits for a model choice without initializing or reading a record', async () => {
    const { saveConfig } = setup();
    await openFormMenu();
    expect(
      screen.queryByRole('menuitem', { name: /^表格(?: right)?$/ })
    ).not.toBeInTheDocument();
    expect(saveConfig).not.toHaveBeenCalled();
    expect(mocks.metadata).not.toHaveBeenCalled();
    expect(mocks.record).not.toHaveBeenCalled();
    expect(mocks.writes).not.toHaveBeenCalled();
  });
  it('initializes the current binding while retaining the source and inherited relation', async () => {
    const { config, saveConfig } = setup();
    await openFormMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: '当前数据表' }));
    await waitFor(() =>
      expect(saveConfig).toHaveBeenCalledExactlyOnceWith({
        ...config,
        initialized: true
      })
    );
    expect(await screen.findByText('请配置表单显示的字段')).toBeInTheDocument();
  });
  it('allows create to choose another model without retaining incompatible inherited fields or relation', async () => {
    const { config, saveConfig } = setup();
    await openFormMenu();
    fireEvent.mouseEnter(
      screen.getByRole('menuitem', { name: /^其他数据表(?: right)?$/ })
    );
    fireEvent.click(await screen.findByRole('menuitem', { name: '其他客户' }));
    await waitFor(() =>
      expect(saveConfig).toHaveBeenCalledExactlyOnceWith({
        ...config,
        initialized: true,
        binding: otherBinding,
        relation: undefined,
        items: [],
        rules: []
      })
    );
    await waitFor(() => expect(mocks.record).toHaveBeenCalled());
    expect(mocks.record.mock.calls.at(-1)?.[2]).toBeNull();
  });
  it.each(['edit', 'view'] as const)(
    'restricts %s initialization to the current record model',
    async (mode) => {
      const { config, saveConfig } = setup(mode);
      await openFormMenu();
      expect(
        screen.queryByRole('menuitem', { name: /^其他数据表(?: right)?$/ })
      ).not.toBeInTheDocument();
      expect(mocks.models).not.toHaveBeenCalled();
      expect(mocks.sources).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole('menuitem', { name: '当前数据表' }));
      await waitFor(() =>
        expect(saveConfig).toHaveBeenCalledExactlyOnceWith({
          ...config,
          initialized: true
        })
      );
    }
  );
  it('keeps a failed initialization unconfigured with a retryable menu', async () => {
    const { saveConfig } = setup();
    saveConfig.mockRejectedValue(new Error('Initialization rejected'));
    await openFormMenu();
    fireEvent.click(screen.getByRole('menuitem', { name: '当前数据表' }));
    expect(
      await screen.findByText('Initialization rejected')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建区块' })).toBeEnabled();
    expect(mocks.record).not.toHaveBeenCalled();
  });
  it('shows a formal runtime state and makes no record reads for an unconfigured edit form', () => {
    setup('edit', false);
    expect(screen.getByText('此表单尚未配置。')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
    expect(mocks.catalog).toHaveBeenCalledWith({ workspaceId: null });
    expect(mocks.metadata).not.toHaveBeenCalled();
    expect(mocks.record).not.toHaveBeenCalled();
  });
  it('keeps standalone defaults and older configurations initialized', async () => {
    expect(createFormBlockConfig(binding).initialized).toBe(true);
    setup('create', true, { initialized: undefined });
    expect(await screen.findByText('请配置表单显示的字段')).toBeInTheDocument();
    expect(mocks.metadata).toHaveBeenCalled();
    expect(
      screen.queryByRole('button', { name: '创建区块' })
    ).not.toBeInTheDocument();
  });
});
