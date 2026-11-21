import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  DataBlockHostContext,
  type DataBlockHost
} from '../../../../lib/data-blocks/host-context';
import {
  createTableBlockConfig,
  type TableBlockConfig
} from '../../../../lib/data-blocks/types';
import { TableBlock } from '../TableBlock';
import { BlockSettingsProvider } from '../../../../lib/block-settings/registry';
import { useBlockSettings } from '../../../../lib/block-settings/registry-store';

function BlockSettingsMenu() {
  const items = useBlockSettings();
  return (
    <div role="toolbar" aria-label="Block settings">
      {items.map((item) => (
        <button key={item.id} disabled={item.disabled} onClick={item.onSelect}>
          {item.label}
        </button>
      ))}
    </div>
  );
}

const mocks = vi.hoisted(() => ({
  metadata: vi.fn(),
  records: vi.fn(),
  remove: vi.fn(),
  refetch: vi.fn()
}));
vi.mock('../../../../api/data-blocks', async (importOriginal) => ({
  ...await importOriginal<typeof import('../../../../api/data-blocks')>(),
  useDataBlockMetadata: mocks.metadata,
  useDataBlockRecords: mocks.records,
  useDataBlockWrites: () => ({ remove: { mutateAsync: mocks.remove } })
}));
vi.mock('../../../../../../shared/i18n/text', () => ({
  i18nText: (_: string, key: string, options?: Record<string, unknown>) =>
    `${key}${options?.deleted !== undefined ? ` ${options.deleted}/${options.failed}` : ''}`
}));
vi.mock('../../shared/DataBlockFilterEditor', () => ({
  DataBlockFilterEditor: () => null
}));
vi.mock('../../shared/DataRelationEditor', () => ({
  DataRelationEditor: () => null
}));
vi.mock('../../shared/DataFieldCreateModal', () => ({
  DataFieldCreateModal: () => null
}));

const baseMetadata = {
  model_id: 'orders-model',
  model_code: 'orders',
  record_id_field: 'order_no',
  fields: [
    {
      id: 'name',
      code: 'name',
      title: 'Name',
      description: '',
      field_kind: 'string',
      display_options: null
    }
  ],
  record_capabilities: {
    can_list: true,
    can_get: true,
    can_create: true,
    can_update: true,
    can_delete: true
  }
};
function setup(
  configure?: (config: TableBlockConfig) => void,
  ownerOverrides: Partial<DataBlockHost> = {},
  contextOverrides: Partial<BlockContext> = {}
) {
  const config = createTableBlockConfig({
    model_id: 'orders-model',
    model_code: 'orders',
    data_source_id: null
  });
  config.columns = [{ field_code: 'name' }];
  config.actions = [
    { id: 'delete', kind: 'delete', enabled: true },
    { id: 'refresh', kind: 'refresh', enabled: true }
  ];
  config.action_column = { hidden: false, position: 0 };
  config.row_actions = [
    { id: 'view', kind: 'view', enabled: true, target_block_id: 'detail-form' }
  ];
  configure?.(config);
  const publish = vi.fn();
  const ctx = {
    props: { data_block: config },
    inputs: {},
    workspace: { id: 'workspace' },
    outputs: { publish },
    currentUser: { id: 'user' },
    ...contextOverrides
  } as unknown as BlockContext;
  const owner: DataBlockHost = {
    block_id: 'orders-table',
    design_mode: false,
    saving: false,
    targets: [],
    saveConfig: vi.fn(),
    prepareAction: vi.fn(),
    openAction: vi.fn(),
    close: vi.fn(),
    ...ownerOverrides
  };
  const view = render(
    <BlockSettingsProvider>
      <DataBlockHostContext.Provider value={owner}>
        <TableBlock ctx={ctx} />
        <BlockSettingsMenu />
      </DataBlockHostContext.Provider>
    </BlockSettingsProvider>
  );
  return {
    owner,
    publish,
    config,
    ctx,
    rerender(nextContext: BlockContext) {
      view.rerender(
        <BlockSettingsProvider>
          <DataBlockHostContext.Provider value={owner}>
            <TableBlock ctx={nextContext} />
            <BlockSettingsMenu />
          </DataBlockHostContext.Provider>
        </BlockSettingsProvider>
      );
    }
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.metadata.mockReturnValue({
    data: baseMetadata,
    isPending: false,
    isError: false
  });
  mocks.records.mockReturnValue({
    data: {
      items: [
        { order_no: 'A', id: 'wrong-A', name: 'Alpha' },
        { order_no: 'B', id: 'wrong-B', name: 'Beta' }
      ],
      total: 2
    },
    isFetching: false,
    isError: false,
    refetch: mocks.refetch
  });
  mocks.remove.mockResolvedValue(undefined);
});

describe('TableBlock runtime boundaries', () => {
  it('passes the actual backend primary key and persisted action target to the host', async () => {
    const { owner } = setup();
    fireEvent.click(
      screen.getAllByRole('button', { name: 'data_blocks.common.view' })[1]
    );
    await waitFor(() =>
      expect(owner.openAction).toHaveBeenCalledWith(
        expect.objectContaining({ target_block_id: 'detail-form' }),
        {
          record_id: 'B',
          source_block_id: 'orders-table',
          source_model_code: 'orders'
        }
      )
    );
    expect(owner.saveConfig).not.toHaveBeenCalled();
  });
  it('keeps only failed selected records after partial deletion and publishes real IDs', async () => {
    mocks.remove.mockImplementation(async (id: string) => {
      if (id === 'B') throw new Error('denied');
    });
    const { publish } = setup();
    expect(
      screen.getByRole('button', {
        name: /^(?:delete )?auto\.delete$/
      })
    ).toBeDisabled();
    fireEvent.click(screen.getAllByRole('checkbox')[0]);
    await waitFor(() =>
      expect(publish).toHaveBeenLastCalledWith({
        record_id: 'A',
        selected_record_ids: ['A', 'B']
      })
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: /^(?:delete )?auto\.delete$/
      })
    );
    fireEvent.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: /^(?:delete )?auto\.delete$/
      })
    );
    await waitFor(() =>
      expect(publish).toHaveBeenLastCalledWith({
        record_id: 'B',
        selected_record_ids: ['B']
      })
    );
    expect(mocks.remove.mock.calls.map(([id]) => id)).toEqual(['A', 'B']);
    expect(
      screen.getByText('data_blocks.table.delete_partial 1/1')
    ).toBeInTheDocument();
  });
  it('does not query or manually refresh related data before a parent is selected', () => {
    setup((config) => {
      config.relation = {
        field_code: 'parent_id',
        parent_model_code: 'parents',
        parent_record_input: 'parent_record_id'
      };
    });
    expect(mocks.records).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.anything(),
      expect.anything(),
      false
    );
    expect(
      screen.getByRole('button', {
        name: /^(?:reload )?auto\.refresh$/
      })
    ).toBeDisabled();
    expect(mocks.refetch).not.toHaveBeenCalled();
  });
  it('shows list data without record actions or selection when metadata has no primary key', () => {
    mocks.metadata.mockReturnValue({
      data: { ...baseMetadata, record_id_field: null },
      isPending: false,
      isError: false
    });
    setup();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'data_blocks.common.view' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', {
        name: /^(?:delete )?auto\.delete$/
      })
    ).not.toBeInTheDocument();
  });
  it('uses effective backend capabilities even when the saved configuration enables actions', () => {
    mocks.metadata.mockReturnValue({
      data: {
        ...baseMetadata,
        record_capabilities: {
          ...baseMetadata.record_capabilities,
          can_get: false,
          can_delete: false
        }
      },
      isPending: false,
      isError: false
    });
    setup();
    expect(
      screen.queryByRole('button', { name: 'data_blocks.common.view' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', {
        name: /^(?:delete )?auto\.delete$/
      })
    ).not.toBeInTheDocument();
    expect(screen.getByText('Alpha')).toBeInTheDocument();
  });
});

describe('TableBlock design persistence', () => {
  it('keeps the failed title draft open and leaves the displayed configuration unchanged', async () => {
    const { owner } = setup(
      (config) => {
        config.title = 'Original title';
      },
      {
        design_mode: true,
        saveConfig: vi.fn().mockRejectedValue(new Error('revision conflict'))
      }
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.title_description' })
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
      target: { value: 'Unsaved title' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'files.save' }));
    await waitFor(() =>
      expect(owner.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ title: 'Unsaved title' })
      )
    );
    expect(
      await screen.findByText('data_blocks.table.save_failed')
    ).toBeInTheDocument();
    expect(screen.getByText('Original title')).toBeInTheDocument();
    expect(
      within(screen.getByRole('dialog')).getAllByRole('textbox')[0]
    ).toHaveValue('Unsaved title');
  });
});

function deferredResult() {
  let resolve!: () => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<void>((fulfilled, rejected) => {
    resolve = fulfilled;
    reject = rejected;
  });
  return { promise, resolve, reject };
}

async function submitTitle(title: string) {
  fireEvent.click(
    screen.getByRole('button', { name: 'data_blocks.form.title_description' })
  );
  const dialog = await screen.findByRole('dialog');
  fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
    target: { value: title }
  });
  fireEvent.click(within(dialog).getByRole('button', { name: 'files.save' }));
}

function confirmSelectedDeletion() {
  fireEvent.click(screen.getAllByRole('checkbox')[0]);
  fireEvent.click(
    screen.getByRole('button', { name: /^(?:delete )?auto\.delete$/ })
  );
  fireEvent.click(
    within(screen.getByRole('dialog')).getByRole('button', {
      name: /^(?:delete )?auto\.delete$/
    })
  );
}

describe('TableBlock config and operation lifetimes', () => {
  it('uses incoming query defaults immediately for a new config source on the same model', () => {
    const { config, ctx, rerender } = setup();
    mocks.records.mockClear();
    rerender({
      ...ctx,
      props: {
        ...ctx.props,
        data_block: {
          ...config,
          page_size: 50,
          sort: { field: 'name', direction: 'desc' }
        }
      }
    });
    expect(mocks.records).toHaveBeenCalled();
    for (const [, , input] of mocks.records.mock.calls) {
      expect(input).toEqual(
        expect.objectContaining({
          page: 1,
          page_size: 50,
          sort: { field: 'name', direction: 'desc' }
        })
      );
    }
  });

  it('displays an acknowledged save while the host has not delivered replacement props', async () => {
    const { owner } = setup(
      (config) => {
        config.title = 'Original title';
      },
      { design_mode: true }
    );
    await submitTitle('Acknowledged title');
    await waitFor(() =>
      expect(owner.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ title: 'Acknowledged title' })
      )
    );
    expect(await screen.findByText('Acknowledged title')).toBeInTheDocument();
    expect(screen.queryByText('Original title')).not.toBeInTheDocument();
  });

  it('does not let a late acknowledged save replace a newer config source', async () => {
    const saved = deferredResult();
    const { owner, config, ctx, rerender } = setup(
      (config) => {
        config.title = 'Original title';
      },
      { design_mode: true, saveConfig: vi.fn(() => saved.promise) }
    );
    await submitTitle('Old source saved title');
    await waitFor(() => expect(owner.saveConfig).toHaveBeenCalled());
    rerender({
      ...ctx,
      props: {
        ...ctx.props,
        data_block: { ...config, title: 'New source title' }
      }
    });
    expect(screen.getByText('New source title')).toBeInTheDocument();
    await act(async () => {
      saved.resolve();
      await saved.promise;
    });
    expect(screen.getByText('New source title')).toBeInTheDocument();
    expect(
      screen.queryByText('Old source saved title')
    ).not.toBeInTheDocument();
  });

  it('never publishes failed deletion IDs into a different parent query context', async () => {
    mocks.metadata.mockReturnValue({
      data: {
        ...baseMetadata,
        fields: [
          ...baseMetadata.fields,
          {
            ...baseMetadata.fields[0],
            id: 'parent',
            code: 'parent_id',
            title: 'Parent'
          }
        ]
      },
      isPending: false,
      isError: false
    });
    const deleted = deferredResult();
    mocks.remove.mockImplementation((id: string) =>
      id === 'A' ? deleted.promise : Promise.resolve()
    );
    const { ctx, publish, rerender } = setup(
      (config) => {
        config.relation = {
          field_code: 'parent_id',
          parent_model_code: 'parents',
          parent_record_input: 'parent_record_id'
        };
      },
      {},
      { inputs: { parent_record_id: 'parent-A' } }
    );
    confirmSelectedDeletion();
    await waitFor(() => expect(mocks.remove).toHaveBeenCalledWith('A'));
    mocks.records.mockReturnValue({
      data: { items: [{ order_no: 'C', name: 'Gamma' }], total: 1 },
      isFetching: false,
      isError: false,
      refetch: mocks.refetch
    });
    rerender({ ...ctx, inputs: { parent_record_id: 'parent-B' } });
    await waitFor(() =>
      expect(publish).toHaveBeenLastCalledWith({
        record_id: '',
        selected_record_ids: []
      })
    );
    await act(async () => {
      deleted.reject(new Error('old parent deletion denied'));
      await deleted.promise.catch(() => undefined);
    });
    await waitFor(() => expect(mocks.remove).toHaveBeenCalledWith('B'));
    expect(screen.getByText('Gamma')).toBeInTheDocument();
    expect(publish).toHaveBeenLastCalledWith({
      record_id: '',
      selected_record_ids: []
    });
  });

  it('does not restore an old page when deletion completes after the page-size default changes', async () => {
    mocks.records.mockReturnValue({
      data: {
        items: [
          { order_no: 'A', name: 'Alpha' },
          { order_no: 'B', name: 'Beta' }
        ],
        total: 60
      },
      isFetching: false,
      isError: false,
      refetch: mocks.refetch
    });
    const deleted = deferredResult();
    mocks.remove.mockImplementation((id: string) =>
      id === 'A' ? deleted.promise : Promise.resolve()
    );
    const { config, ctx, rerender } = setup();
    fireEvent.click(screen.getByRole('button', { name: /^(页|Page) 3$/i }));
    await waitFor(() =>
      expect(mocks.records).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.anything(),
        expect.objectContaining({ page: 3 }),
        true
      )
    );
    confirmSelectedDeletion();
    await waitFor(() => expect(mocks.remove).toHaveBeenCalledWith('A'));
    rerender({
      ...ctx,
      props: { ...ctx.props, data_block: { ...config, page_size: 10 } }
    });
    await waitFor(() =>
      expect(mocks.records).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.anything(),
        expect.objectContaining({ page: 1, page_size: 10 }),
        true
      )
    );
    await act(async () => {
      deleted.resolve();
      await deleted.promise;
    });
    await waitFor(() => expect(mocks.remove).toHaveBeenCalledWith('B'));
    expect(mocks.records).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.anything(),
      expect.objectContaining({ page: 1, page_size: 10 }),
      true
    );
  });
});
