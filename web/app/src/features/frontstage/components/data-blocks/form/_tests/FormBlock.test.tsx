import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutEffect, type ComponentProps, type ReactNode } from 'react';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  createFormBlockConfig,
  type DataBlockField,
  type DataFormItem,
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
  designPatch: null as Partial<FormBlockConfig> | null,
  metadataReady: true,
  observeFieldCommits: false,
  fieldCommits: [] as {
    code: string;
    value: unknown;
    displayed: string | null;
    readOnly: boolean | undefined;
  }[],
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
    ] as DataBlockField[]
  }
}));
vi.mock('../../../../api/data-blocks', () => ({
  useDataBlockMetadata: () => ({
    data: state.metadataReady ? state.metadata : undefined,
    refetch: vi.fn(),
    isLoading: !state.metadataReady,
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
    onSave,
    submitAction
  }: {
    config: FormBlockConfig;
    onSave(config: FormBlockConfig): Promise<boolean>;
    submitAction?: ReactNode;
  }) => (
    <>
      {submitAction}
      <button
        onClick={() => {
          void onSave({ ...config, title: 'New design', ...state.designPatch });
        }}
      >
        save-design
      </button>
    </>
  )
}));
vi.mock('../FormFieldControl', async () => {
  const actual = await vi.importActual<typeof import('../FormFieldControl')>(
    '../FormFieldControl'
  );
  return {
    ...actual,
    FormFieldControl(props: ComponentProps<typeof actual.FormFieldControl>) {
      useLayoutEffect(() => {
        if (!state.observeFieldCommits) return;
        const control = props.id ? document.getElementById(props.id) : null;
        state.fieldCommits.push({
          code: props.field.code,
          value: props.value,
          displayed:
            control instanceof HTMLInputElement ||
            control instanceof HTMLTextAreaElement ||
            control instanceof HTMLSelectElement
              ? control.value
              : null,
          readOnly: props.readOnly
        });
      });
      return <actual.FormFieldControl {...props} />;
    }
  };
});
vi.mock('@ant-design/x-markdown', () => ({ default: () => null }));
const initialMetadata = state.metadata;

function field(
  code: string,
  title: string,
  overrides: Partial<DataBlockField> = {}
): DataBlockField {
  return {
    ...initialMetadata.fields[0],
    id: code,
    code,
    title,
    is_required: false,
    api_required: false,
    ...overrides
  };
}

function item(
  code: string,
  overrides: Partial<DataFormItem> = {}
): DataFormItem {
  return {
    id: code,
    field_code: code,
    kind: 'field',
    row: code,
    span: 24,
    ...overrides
  };
}
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
  state.designPatch = null;
  state.metadataReady = true;
  state.observeFieldCommits = false;
  state.fieldCommits = [];
  state.metadata = {
    ...initialMetadata,
    record_capabilities: { ...initialMetadata.record_capabilities },
    fields: initialMetadata.fields.map((entry) => ({ ...entry }))
  };
  state.saveConfig.mockReset().mockResolvedValue(undefined);
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

describe('FormBlock config and design save lifetimes', () => {
  it('immediately accepts a same-record config source and preserves its draft after an old save resolves', async () => {
    state.design = true;
    state.records.A = { uuid: 'A', name: 'Server value' };
    let finish!: () => void;
    state.saveConfig.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        })
    );
    const original = { ...config('edit'), title: 'Original design' };
    const view = render(
      <FormBlock ctx={context(original, { record_id: 'A' })} />
    );
    await waitFor(() =>
      expect(screen.getByLabelText('Name')).toHaveValue('Server value')
    );
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Record draft' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'save-design' }));
    expect(state.saveConfig).toHaveBeenCalledOnce();
    expect(
      screen.getByRole('heading', { name: 'New design' })
    ).toBeInTheDocument();

    const incoming = { ...original, title: 'Incoming design' };
    view.rerender(<FormBlock ctx={context(incoming, { record_id: 'A' })} />);
    expect
      .soft(screen.queryByRole('heading', { name: 'Incoming design' }))
      .toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('Record draft');
    await act(async () => finish());

    expect(
      screen.getByRole('heading', { name: 'Incoming design' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: 'New design' })
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('Record draft');
    expect(state.update).not.toHaveBeenCalled();
    expect(state.publish).not.toHaveBeenCalled();
    expect(state.close).not.toHaveBeenCalled();
  });

  it('keeps a newer same-record config and draft when an old design save rejects without showing its error', async () => {
    state.design = true;
    state.records.A = { uuid: 'A', name: 'Server value' };
    let reject!: (error: Error) => void;
    state.saveConfig.mockImplementation(
      () =>
        new Promise<void>((_resolve, fail) => {
          reject = fail;
        })
    );
    const original = { ...config('edit'), title: 'Original design' };
    const view = render(
      <FormBlock ctx={context(original, { record_id: 'A' })} />
    );
    await waitFor(() =>
      expect(screen.getByLabelText('Name')).toHaveValue('Server value')
    );
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Record draft' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'save-design' }));
    expect(state.saveConfig).toHaveBeenCalledOnce();
    expect(
      screen.getByRole('heading', { name: 'New design' })
    ).toBeInTheDocument();

    const incoming = { ...original, title: 'Incoming design' };
    view.rerender(<FormBlock ctx={context(incoming, { record_id: 'A' })} />);
    expect
      .soft(screen.queryByRole('heading', { name: 'Incoming design' }))
      .toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('Record draft');
    await act(async () => reject(new Error('Old design rejected')));

    expect
      .soft(screen.queryByText('Old design rejected'))
      .not.toBeInTheDocument();
    expect(
      screen.queryByText('data_blocks.form.design_failed')
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Incoming design' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: 'Original design' })
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('Record draft');
    expect(state.update).not.toHaveBeenCalled();
    expect(state.publish).not.toHaveBeenCalled();
    expect(state.close).not.toHaveBeenCalled();
  });
});

describe('FormBlock source snapshots and editable history', () => {
  it.each(['create', 'edit'] as const)(
    'shows the first %s source value in the first committed field control',
    (mode) => {
      const current = config(mode);
      const expected = mode === 'create' ? 'Metadata default' : 'Record seed';
      state.metadata.fields[0].default_value = 'Metadata default';
      state.records.A = { uuid: 'A', name: 'Record seed' };
      state.observeFieldCommits = true;
      render(
        <FormBlock
          ctx={context(current, mode === 'edit' ? { record_id: 'A' } : {})}
        />
      );
      expect(screen.getByLabelText('Name')).toHaveValue(expected);
      const commits = state.fieldCommits.filter(
        (entry) => entry.code === 'name' && entry.displayed !== null
      );
      expect(commits.length).toBeGreaterThan(0);
      expect(commits[0].displayed).toBe(expected);
    }
  );

  it('preserves edits while adding each new field default only once', () => {
    state.metadata.fields = [
      field('name', 'Name', { default_value: 'Backend name' }),
      field('tag', 'Tag', { default_value: 'Backend tag' })
    ];
    const original = {
      ...config(),
      items: [item('name', { default_value: 'Configured name' })]
    };
    const view = render(<FormBlock ctx={context(original)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('Configured name');
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Typed name' }
    });
    state.metadata = {
      ...state.metadata,
      fields: [
        field('name', 'Name', { default_value: 'Refetched name' }),
        field('tag', 'Tag', { default_value: 'Refetched tag' })
      ]
    };
    const expanded = {
      ...original,
      items: [
        item('name', { default_value: 'Changed name default' }),
        item('tag', { default_value: 'Configured tag' })
      ]
    };
    view.rerender(<FormBlock ctx={context(expanded)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('Typed name');
    expect(screen.getByLabelText('Tag')).toHaveValue('Configured tag');
    fireEvent.change(screen.getByLabelText('Tag'), {
      target: { value: '' }
    });
    state.metadata = { ...state.metadata };
    view.rerender(
      <FormBlock
        ctx={context({
          ...expanded,
          items: [expanded.items[0], item('tag', { default_value: 'Later tag' })]
        })}
      />
    );
    expect(screen.getByLabelText('Name')).toHaveValue('Typed name');
    expect(screen.getByLabelText('Tag')).toHaveValue('');
  });

  it('does not revisit a first-seen missing default after refetch or field reattachment', () => {
    const original = config();
    const view = render(<FormBlock ctx={context(original)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('');
    state.metadata = {
      ...state.metadata,
      fields: [field('name', 'Name', { default_value: 'Late backend default' })]
    };
    const changed = {
      ...original,
      items: [item('name', { default_value: 'Late configured default' })]
    };
    view.rerender(<FormBlock ctx={context(changed)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('');
    view.rerender(<FormBlock ctx={context({ ...changed, items: [] })} />);
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument();
    view.rerender(<FormBlock ctx={context(changed)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('');
    expect(state.create).not.toHaveBeenCalled();
  });

  it('waits for metadata while preserving explicit null, false and zero defaults', () => {
    state.metadataReady = false;
    state.metadata.fields = [
      field('name', 'Name', { default_value: 'First backend default' }),
      field('empty', 'Empty', { default_value: 'Must not replace null' }),
      field('enabled', 'Enabled', {
        field_kind: 'boolean',
        display_interface: 'switch',
        default_value: true
      }),
      field('count', 'Count', {
        field_kind: 'number',
        display_interface: 'input_number',
        default_value: 9
      })
    ];
    const current = {
      ...config(),
      items: [
        item('name'),
        item('empty', { default_value: null }),
        item('enabled', { default_value: false }),
        item('count', { default_value: 0 })
      ]
    };
    const view = render(<FormBlock ctx={context(current)} />);
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument();
    state.metadataReady = true;
    view.rerender(<FormBlock ctx={context(current)} />);
    expect(screen.getByLabelText('Name')).toHaveValue('First backend default');
    expect(screen.getByLabelText('Empty')).toHaveValue('');
    expect(screen.getByRole('switch', { name: 'Enabled' })).not.toBeChecked();
    expect(screen.getByRole('spinbutton', { name: 'Count' })).toHaveDisplayValue(
      '0'
    );
  });

  it('replaces pre-seed design edits once and preserves later edits across same-record refetches', () => {
    state.design = true;
    const current = config('edit');
    const view = render(
      <FormBlock ctx={context(current, { record_id: 'A' })} />
    );
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Before first record' }
    });
    expect(screen.getByLabelText('Name')).toHaveValue('Before first record');
    state.records.A = { uuid: 'A', name: 'First record' };
    view.rerender(<FormBlock ctx={context(current, { record_id: 'A' })} />);
    expect(screen.getByLabelText('Name')).toHaveValue('First record');
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'After first record' }
    });
    delete state.records.A;
    view.rerender(<FormBlock ctx={context(current, { record_id: 'A' })} />);
    expect(screen.getByLabelText('Name')).toHaveValue('After first record');
    state.records.A = { uuid: 'A', name: 'Refetched record' };
    view.rerender(<FormBlock ctx={context(current, { record_id: 'A' })} />);
    expect(screen.getByLabelText('Name')).toHaveValue('After first record');
    expect(state.update).not.toHaveBeenCalled();
  });

  it('retains former relation values across same-owner design changes and submits the current readonly parent', async () => {
    state.design = true;
    state.observeFieldCommits = true;
    state.metadata.fields = [
      field('name', 'Name'),
      ...[
        ['customer', 'Customer'],
        ['account', 'Account']
      ].map(([code, title]) =>
        field(code, title, {
          field_kind: 'many_to_one',
          relation_target_model_id: 'customers',
          relation_target_model_code: 'customers'
        })
      )
    ];
    const relation = {
      field_code: 'customer',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    const current = {
      ...config(),
      relation,
      close_after_submit: false,
      items: [
        item('name'),
        item('customer', { default_value: 'Conflicting default' }),
        item('account')
      ]
    };
    render(<FormBlock ctx={context(current, { parent_record_id: 'P1' })} />);
    expect(screen.getByText('P1')).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: 'Customer' })
    ).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Name'), {
      target: { value: 'Typed name' }
    });
    state.designPatch = { relation: { ...relation, field_code: 'account' } };
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'save-design' }));
    });
    expect(screen.getByRole('combobox', { name: 'Customer' })).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: 'Account' })
    ).not.toBeInTheDocument();
    expect(screen.getAllByText('P1')).toHaveLength(2);
    expect(screen.getByLabelText('Name')).toHaveValue('Typed name');
    const account = state.fieldCommits.filter((entry) => entry.code === 'account');
    expect(account.at(-1)).toMatchObject({ value: 'P1', readOnly: true });
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.submit' })
    );
    await waitFor(() =>
      expect(state.create).toHaveBeenCalledWith({
        name: 'Typed name',
        customer: 'P1',
        account: 'P1'
      })
    );
    state.designPatch = { relation: undefined };
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'save-design' }));
    });
    expect(screen.getByRole('combobox', { name: 'Customer' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Account' })).toBeInTheDocument();
    expect(screen.getAllByText('P1')).toHaveLength(2);
    expect(screen.getByLabelText('Name')).toHaveValue('Typed name');
    expect(state.update).not.toHaveBeenCalled();
  });
});
