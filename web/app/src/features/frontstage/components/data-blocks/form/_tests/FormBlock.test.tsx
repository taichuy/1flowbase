import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  createFormBlockConfig,
  type FormBlockConfig
} from '../../../../lib/data-blocks/types';
import { FormBlock } from '../FormBlock';

const state = vi.hoisted(() => ({
  records: {} as Record<string, Record<string, unknown> | undefined>,
  create: vi.fn(),
  update: vi.fn(),
  publish: vi.fn(),
  close: vi.fn(),
  saveConfig: vi.fn(),
  design: false,
  metadata: {
    model_id: 'm',
    model_code: 'users',
    record_id_field: 'uuid',
    record_capabilities: {
      can_list: true,
      can_get: true,
      can_create: true,
      can_update: true,
      can_delete: false
    },
    fields: [
      {
        id: 'f',
        code: 'name',
        title: 'Name',
        field_kind: 'string',
        is_writable: true,
        is_required: true,
        api_required: true,
        default_value: null,
        display_interface: 'input',
        display_options: {},
        relation_target_model_id: null,
        relation_target_model_code: null,
        relation_options: {}
      }
    ]
  }
}));
vi.mock('../../../../api/data-blocks', () => ({
  useDataBlockMetadata: () => ({
    data: state.metadata,
    refetch: vi.fn(),
    isLoading: false,
    isError: false
  }),
  useDataBlockRecord: (_ctx: unknown, _binding: unknown, id: string) => ({
    data: state.records[id],
    isLoading: Boolean(id && !state.records[id]),
    isError: false
  }),
  useDataBlockRecords: () => ({ data: { items: [], total: 0 } }),
  useDataBlockWrites: () => ({
    create: { mutateAsync: state.create },
    update: { mutateAsync: state.update }
  })
}));
vi.mock('../../../../lib/data-blocks/host-context', () => ({
  useDataBlockHost: () => ({
    block_id: 'block',
    design_mode: state.design,
    saving: false,
    close: state.close,
    saveConfig: state.saveConfig
  })
}));
vi.mock('../../../../../../shared/i18n/text', () => ({
  i18nText: (_namespace: string, key: string) => key
}));
vi.mock('../FormDesignControls', () => ({
  FormDesignControls: ({
    config,
    onSave
  }: {
    config: FormBlockConfig;
    onSave(config: FormBlockConfig): Promise<boolean>;
  }) => (
    <button
      onClick={() => {
        void onSave({ ...config, title: 'New design' });
      }}
    >
      save-design
    </button>
  ),
  newFormItem: () => ({})
}));
vi.mock('@ant-design/x-markdown', () => ({ default: () => null }));
const config = (mode: FormBlockConfig['mode'] = 'create'): FormBlockConfig => ({
  ...createFormBlockConfig(
    { model_id: 'm', model_code: 'users', data_source_id: null },
    mode
  ),
  items: [
    { id: 'name', field_code: 'name', kind: 'field', row: 'name', span: 24 }
  ],
  submit_enabled: true
});
const context = (data_block: FormBlockConfig, inputs = {}): BlockContext =>
  ({
    workspace: { id: 'w' },
    props: { data_block },
    inputs,
    outputs: { publish: state.publish }
  }) as unknown as BlockContext;

beforeEach(() => {
  state.records = {};
  state.design = false;
  state.saveConfig.mockReset().mockResolvedValue(undefined);
  state.metadata.fields[0].field_kind = 'string';
  state.create.mockReset().mockResolvedValue({ uuid: 'new', name: 'Saved' });
  state.update.mockReset().mockResolvedValue({ uuid: 'B', name: 'Saved' });
  state.publish.mockReset().mockResolvedValue({ ok: true, stale: false });
  state.close.mockReset();
});

describe('FormBlock record lifecycle', () => {
  it('keeps input on write failure and never publishes or closes', async () => {
    state.create.mockRejectedValue(new Error('Write rejected'));
    render(<FormBlock ctx={context(config())} />);
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Keep my draft' }
    });
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    );
    await screen.findByText('Write rejected');
    expect(screen.getByLabelText('Name')).toHaveValue('Keep my draft');
    expect(state.publish).not.toHaveBeenCalled();
    expect(state.close).not.toHaveBeenCalled();
  });
  it('publishes the backend primary key after create and then closes', async () => {
    render(<FormBlock ctx={context(config())} />);
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Saved' }
    });
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    );
    await waitFor(() =>
      expect(state.publish).toHaveBeenCalledWith({
        saved_record_id: 'new',
        saved_record: { uuid: 'new', name: 'Saved' },
        save_revision: 1
      })
    );
    expect(state.close).toHaveBeenCalledOnce();
  });
  it('requires an explicit ID for edit instead of picking any record', () => {
    state.records.A = { uuid: 'A', name: 'A' };
    render(<FormBlock ctx={context(config('edit'))} />);
    expect(
      screen.getByText('data_blocks.form.missing_record')
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument();
    expect(state.update).not.toHaveBeenCalled();
  });
  it('switches A to B without allowing late A to overwrite B', async () => {
    const edit = config('edit');
    const view = render(<FormBlock ctx={context(edit, { record_id: 'A' })} />);
    state.records.B = { uuid: 'B', name: 'Bee' };
    view.rerender(<FormBlock ctx={context(edit, { record_id: 'B' })} />);
    await waitFor(() =>
      expect(screen.getByLabelText('Name')).toHaveValue('Bee')
    );
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'B draft' }
    });
    state.records.A = { uuid: 'A', name: 'Late A' };
    view.rerender(<FormBlock ctx={context(edit, { record_id: 'B' })} />);
    expect(screen.getByLabelText('Name')).toHaveValue('B draft');
  });
  it('prevents completion of a previous record from closing the next record', async () => {
    let finish!: (value: Record<string, unknown>) => void;
    state.records.A = { uuid: 'A', name: 'A' };
    state.records.B = { uuid: 'B', name: 'B' };
    state.update.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        })
    );
    const edit = config('edit');
    const view = render(<FormBlock ctx={context(edit, { record_id: 'A' })} />);
    await waitFor(() => expect(screen.getByLabelText('Name')).toHaveValue('A'));
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    );
    view.rerender(<FormBlock ctx={context(edit, { record_id: 'B' })} />);
    await act(async () => finish({ uuid: 'A', name: 'Saved A' }));
    expect(state.close).not.toHaveBeenCalled();
    expect(state.publish).not.toHaveBeenCalled();
    expect(screen.getByLabelText('Name')).toHaveValue('B');
  });
  it('keeps the record draft and rolls back a failed design save', async () => {
    state.design = true;
    state.saveConfig.mockRejectedValue(new Error('Design rejected'));
    render(<FormBlock ctx={context({ ...config(), title: 'Old design' })} />);
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Record draft' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'save-design' }));
    await screen.findByText('Design rejected');
    expect(screen.getByText('Old design')).toBeInTheDocument();
    expect(screen.queryByText('New design')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('Record draft');
    expect(state.create).not.toHaveBeenCalled();
    expect(state.publish).not.toHaveBeenCalled();
  });

  it('does not write a required relationship when no record is selected', async () => {
    state.metadata.fields[0].field_kind = 'many_to_one';
    render(<FormBlock ctx={context(config())} />);
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    );
    await screen.findByText('data_blocks.form.required_error');
    expect(state.create).not.toHaveBeenCalled();
  });

  it('blocks related creates when the parent input is missing', () => {
    const related = {
      ...config(),
      relation: {
        field_code: 'customer',
        parent_model_code: 'customers',
        parent_record_input: 'parent_record_id'
      }
    };
    render(<FormBlock ctx={context(related)} />);
    expect(
      screen.getByText('data_blocks.form.missing_parent')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    ).toBeDisabled();
  });
});
