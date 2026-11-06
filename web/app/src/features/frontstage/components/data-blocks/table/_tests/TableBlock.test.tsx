import {
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

const mocks = vi.hoisted(() => ({
  metadata: vi.fn(),
  records: vi.fn(),
  remove: vi.fn(),
  refetch: vi.fn()
}));
vi.mock('../../../../api/data-blocks', () => ({
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
  ownerOverrides: Partial<DataBlockHost> = {}
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
  config.row_actions = [
    { id: 'view', kind: 'view', enabled: true, target_block_id: 'detail-form' }
  ];
  configure?.(config);
  const publish = vi.fn();
  const ctx = {
    props: { data_block: config },
    inputs: {},
    outputs: { publish },
    currentUser: { id: 'user' }
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
  render(
    <DataBlockHostContext.Provider value={owner}>
      <TableBlock ctx={ctx} />
    </DataBlockHostContext.Provider>
  );
  return { owner, publish };
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
      screen.getAllByRole('button', { name: 'data_blocks.table.view' })[1]
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
      screen.getByRole('button', { name: 'data_blocks.table.delete' })
    ).toBeDisabled();
    fireEvent.click(screen.getAllByRole('checkbox')[0]);
    await waitFor(() =>
      expect(publish).toHaveBeenLastCalledWith({
        record_id: 'A',
        selected_record_ids: ['A', 'B']
      })
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.table.delete' })
    );
    fireEvent.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'data_blocks.table.delete'
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
      screen.getByRole('button', { name: 'data_blocks.table.refresh' })
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
      screen.queryByRole('button', { name: 'data_blocks.table.view' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'data_blocks.table.delete' })
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
      screen.queryByRole('button', { name: 'data_blocks.table.view' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'data_blocks.table.delete' })
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
      screen.getByRole('button', { name: 'data_blocks.table.block_settings' })
    );
    fireEvent.click(
      await screen.findByText('data_blocks.table.title_description')
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
      target: { value: 'Unsaved title' }
    });
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'data_blocks.table.save' })
    );
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
