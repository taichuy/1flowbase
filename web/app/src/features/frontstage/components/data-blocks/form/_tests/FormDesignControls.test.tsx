import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { ConfigProvider } from 'antd';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  createFormBlockConfig,
  type DataBlockField,
  type FormBlockConfig
} from '../../../../lib/data-blocks/types';
import { FormDesignControls, type DesignDialog } from '../FormDesignControls';

vi.mock('../../../../../../shared/i18n/text', () => ({
  i18nText: (_namespace: string, key: string) => key
}));
vi.mock('../../shared/DataFieldCreateModal', () => ({
  DataFieldCreateModal: () => null
}));

const fields: DataBlockField[] = [
  { id: 'name', code: 'name', title: 'Alpha', api_required: true },
  { id: 'other', code: 'internal_beta', title: 'Other', api_required: false }
].map((field) => ({
  description: null,
  field_kind: 'string',
  is_writable: true,
  is_required: false,
  default_value: null,
  display_interface: null,
  display_options: {},
  relation_target_model_id: null,
  relation_target_model_code: null,
  relation_options: {},
  ...field
}));
const ctx = {
  workspace: { id: 'workspace' },
  currentUser: { id: 'user', permissions: [] },
  props: {},
  inputs: {},
  outputs: { publish: vi.fn() }
} as unknown as BlockContext;

function config(): FormBlockConfig {
  return {
    ...createFormBlockConfig({
      model_id: 'model',
      model_code: 'model',
      data_source_id: null
    }),
    title: 'Original title',
    items: [
      {
        id: 'first',
        kind: 'field',
        field_code: 'name',
        title: 'First item',
        row: 'first',
        span: 24
      },
      {
        id: 'second',
        kind: 'field',
        field_code: 'name',
        title: 'Second item',
        row: 'second',
        span: 24
      }
    ]
  };
}

function Harness({
  value,
  initialDialog = null,
  onSave,
  availableFields = fields
}: {
  value: FormBlockConfig;
  initialDialog?: DesignDialog;
  onSave: (value: FormBlockConfig) => Promise<boolean>;
  availableFields?: DataBlockField[];
}) {
  const [dialog, setDialog] = useState<DesignDialog>(initialDialog);
  return (
    <ConfigProvider theme={{ token: { motion: false } }}>
      <button onClick={() => setDialog('title')}>Open title</button>
      <button onClick={() => setDialog(value.items[0])}>Open first item</button>
      <button onClick={() => setDialog(value.items[1])}>
        Open second item
      </button>
      <FormDesignControls
        ctx={ctx}
        config={value}
        fields={availableFields}
        saving={false}
        onSave={onSave}
        onFieldCreated={async () => {}}
        dialog={dialog}
        setDialog={setDialog}
      />
    </ConfigProvider>
  );
}

const save = (dialog: HTMLElement) =>
  fireEvent.click(
    within(dialog).getByRole('button', { name: 'data_blocks.form.save_design' })
  );
afterEach(() => vi.restoreAllMocks());

describe('FormDesignControls draft consumers', () => {
  it('keeps open edits on rerender and unsuccessful save, then reopens from the latest source', async () => {
    const original = config();
    const onSave = vi
      .fn()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true);
    const view = render(
      <Harness value={original} initialDialog="title" onSave={onSave} />
    );
    const dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
      target: { value: 'Draft title' }
    });
    const replacement = { ...original, title: 'Latest host title' };
    view.rerender(<Harness value={replacement} onSave={onSave} />);
    expect(within(dialog).getAllByRole('textbox')[0]).toHaveValue(
      'Draft title'
    );
    save(dialog);
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(within(dialog).getAllByRole('textbox')[0]).toHaveValue(
      'Draft title'
    );
    expect(onSave).toHaveBeenLastCalledWith({
      ...original,
      title: 'Draft title'
    });
    save(dialog);
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    fireEvent.click(screen.getByRole('button', { name: 'Open title' }));
    expect(
      within(await screen.findByRole('dialog')).getAllByRole('textbox')[0]
    ).toHaveValue('Latest host title');
  });

  it('discards cancelled edits and remounts item drafts when the dialog item ID changes', async () => {
    const value = config();
    const onSave = vi.fn().mockResolvedValue(true);
    render(
      <Harness value={value} initialDialog={value.items[0]} onSave={onSave} />
    );
    let dialog = await screen.findByRole('dialog');
    fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
      target: { value: 'Discarded' }
    });
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'auto.cancel' })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    expect(onSave).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Open first item' }));
    dialog = await screen.findByRole('dialog');
    expect(within(dialog).getAllByRole('textbox')[0]).toHaveValue('First item');
    fireEvent.change(within(dialog).getAllByRole('textbox')[0], {
      target: { value: 'Unsaved first' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'Open second item' }));
    expect(
      within(await screen.findByRole('dialog')).getAllByRole('textbox')[0]
    ).toHaveValue('Second item');
  });

  it('keeps backend constraints and saves the edited default and span to the matching item only', async () => {
    const value = config();
    const onSave = vi.fn().mockResolvedValue(true);
    render(
      <Harness value={value} initialDialog={value.items[0]} onSave={onSave} />
    );
    const dialog = await screen.findByRole('dialog');
    const required = within(dialog).getByRole('checkbox', {
      name: 'data_blocks.fields.required'
    });
    expect(required).toBeChecked();
    expect(required).toBeDisabled();
    fireEvent.click(
      within(dialog).getByRole('checkbox', {
        name: 'data_blocks.form.readonly'
      })
    );
    fireEvent.change(within(dialog).getAllByRole('textbox')[3], {
      target: { value: 'Default text' }
    });
    fireEvent.mouseDown(within(dialog).getByRole('combobox'));
    fireEvent.click(await screen.findByText('50%', { exact: true }));
    save(dialog);
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        ...value,
        items: [
          {
            ...value.items[0],
            default_value: 'Default text',
            read_only: true,
            span: 12
          },
          value.items[1]
        ]
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  it('does not make a backend readonly field editable', async () => {
    const value = config();
    render(
      <Harness
        value={value}
        initialDialog={value.items[0]}
        onSave={vi.fn()}
        availableFields={[{ ...fields[0], is_writable: false }]}
      />
    );
    const checkbox = within(await screen.findByRole('dialog')).getByRole(
      'checkbox',
      { name: 'data_blocks.form.readonly' }
    );
    expect(checkbox).toBeChecked();
    expect(checkbox).toBeDisabled();
    expect(
      within(screen.getByRole('dialog')).getAllByRole('textbox')
    ).toHaveLength(3);
  });

  it('filters by title and code without changing survivor order or the field toggle payload', async () => {
    const value = { ...config(), items: [] };
    const onSave = vi.fn().mockResolvedValue(true);
    render(<Harness value={value} onSave={onSave} />);
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.form.configure_fields' })
    );
    const search = await screen.findByPlaceholderText(
      'data_blocks.form.search_fields'
    );
    expect(
      screen
        .getAllByRole('menuitem')
        .slice(0, 2)
        .map((item) => item.textContent)
    ).toEqual(['Alpha', 'Other']);
    fireEvent.change(search, { target: { value: 'ALP' } });
    expect(screen.getByRole('menuitem', { name: 'Alpha' })).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: 'Other' })
    ).not.toBeInTheDocument();
    fireEvent.change(search, { target: { value: 'INTERNAL_B' } });
    expect(
      screen.queryByRole('menuitem', { name: 'Alpha' })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('menuitem', { name: 'Other' }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave).toHaveBeenLastCalledWith({
      ...value,
      items: [
        expect.objectContaining({
          kind: 'field',
          field_code: 'internal_beta',
          span: 24
        })
      ]
    });
  });

  it('updates layout controls in the same draft', async () => {
    const value = config();
    const onSave = vi.fn().mockResolvedValue(true);
    render(<Harness value={value} initialDialog="layout" onSave={onSave} />);
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(
      within(dialog).getByRole('radio', { name: 'data_blocks.form.horizontal' })
    );
    fireEvent.change(within(dialog).getByRole('spinbutton'), {
      target: { value: '180' }
    });
    fireEvent.click(
      within(dialog).getByRole('checkbox', { name: 'data_blocks.form.colon' })
    );
    save(dialog);
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        ...value,
        label_layout: 'horizontal',
        label_width: 180,
        colon: false
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  it('creates rule IDs only on Add and removes the selected rule from the saved draft', async () => {
    const value = config();
    const onSave = vi.fn().mockResolvedValue(true);
    const randomUUID = vi.spyOn(crypto, 'randomUUID');
    render(<Harness value={value} initialDialog="rules" onSave={onSave} />);
    const dialog = await screen.findByRole('dialog');
    expect(randomUUID).not.toHaveBeenCalled();
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'data_blocks.form.add_rule' })
    );
    expect(randomUUID).toHaveBeenCalledTimes(1);
    const ruleId = randomUUID.mock.results[0].value;
    expect(
      within(dialog).getByRole('combobox', {
        name: 'data_blocks.form.target_field'
      })
    ).toBeInTheDocument();
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'data_blocks.form.add_rule' })
    );
    const remove = within(dialog).getAllByRole('button', {
      name: 'data_blocks.form.remove_rule'
    });
    fireEvent.click(remove[1]);
    save(dialog);
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        ...value,
        rules: [
          {
            id: ruleId,
            target_field: 'name',
            effect: 'show',
            condition: { id: 'root', conjunction: 'and', conditions: [] }
          }
        ]
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });
});
