import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { ConfigProvider } from 'antd';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DataFilterVariable } from '../../../../lib/data-blocks/filter-variables';
import type {
  DataBlockField,
  DataFilterCondition,
  DataFilterGroup
} from '../../../../lib/data-blocks/types';
import { DataBlockFilterEditor } from '../DataBlockFilterEditor';

vi.mock('../../../../../../shared/i18n/text', () => ({
  i18nText: (_namespace: string, key: string) => key
}));

const fields: DataBlockField[] = [
  { id: 'name', code: 'name', title: 'Name', field_kind: 'string' },
  { id: 'amount', code: 'amount', title: 'Amount', field_kind: 'number' },
  { id: 'enabled', code: 'enabled', title: 'Enabled', field_kind: 'boolean' },
  {
    id: 'choice',
    code: 'choice',
    title: 'Choice',
    field_kind: 'enum',
    display_options: {
      options: [
        { label: 'Seven', value: 7 },
        { label: 'Nine', value: 9 }
      ]
    }
  }
].map((field) => ({
  description: null,
  is_writable: true,
  is_required: false,
  api_required: false,
  default_value: null,
  display_interface: null,
  display_options: {},
  relation_target_model_id: null,
  relation_target_model_code: null,
  relation_options: {},
  ...field
}));
const variables: DataFilterVariable[] = [
  { value: 'currentUser.id', label: 'Current user', resolved: 'user-1' }
];
const condition = (
  values: Partial<DataFilterCondition> = {}
): DataFilterCondition => ({
  id: 'condition',
  field_code: 'name',
  operator: '$eq',
  value: 'original',
  ...values
});
const group = (
  conditions: DataFilterGroup['conditions'] = []
): DataFilterGroup => ({
  id: 'root',
  conjunction: 'and',
  conditions
});

function Harness({
  initial,
  onChange,
  availableFields = fields
}: {
  initial: DataFilterGroup;
  onChange: (value: DataFilterGroup) => void;
  availableFields?: DataBlockField[];
}) {
  const [value, setValue] = useState(initial);
  return (
    <ConfigProvider theme={{ token: { motion: false } }}>
      <DataBlockFilterEditor
        value={value}
        fields={availableFields}
        variables={variables}
        onChange={(next) => {
          onChange(next);
          setValue(next);
        }}
      />
    </ConfigProvider>
  );
}

async function select(control: HTMLElement, text: string) {
  fireEvent.mouseDown(control);
  fireEvent.click(await screen.findByText(text, { exact: true }));
}
async function expectChange(
  onChange: ReturnType<typeof vi.fn>,
  value: DataFilterGroup
) {
  await waitFor(() => expect(onChange).toHaveBeenLastCalledWith(value));
}
afterEach(() => vi.restoreAllMocks());

describe('DataBlockFilterEditor controlled conditions', () => {
  it('updates and removes nested conditions by ID while preserving siblings and group conjunction', async () => {
    const first = condition({ id: 'first', value: 'First' });
    const nested = condition({ id: 'nested', value: 'Nested' });
    const last = condition({ id: 'last', field_code: 'amount', value: 3 });
    const inner: DataFilterGroup = {
      id: 'inner',
      conjunction: 'or',
      conditions: [nested]
    };
    const initial = group([first, inner, last]);
    const onChange = vi.fn();
    render(<Harness initial={initial} onChange={onChange} />);
    fireEvent.change(screen.getByDisplayValue('Nested'), {
      target: { value: 'Changed' }
    });
    const changed = { ...inner, conditions: [{ ...nested, value: 'Changed' }] };
    await expectChange(onChange, group([first, changed, last]));
    expect(onChange.mock.lastCall?.[0].conditions[0]).toBe(first);
    expect(onChange.mock.lastCall?.[0].conditions[2]).toBe(last);
    fireEvent.click(
      screen.getAllByRole('button', { name: 'data_blocks.common.remove' })[1]
    );
    await expectChange(
      onChange,
      group([first, { ...inner, conditions: [] }, last])
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.filter.remove_group' })
    );
    await expectChange(onChange, group([first, last]));
    await select(
      screen.getByRole('combobox', { name: 'data_blocks.filter.conjunction' }),
      'data_blocks.filter.any'
    );
    await expectChange(onChange, {
      ...group([first, last]),
      conjunction: 'or'
    });
  });

  it('resets field, operator and value source without changing the canonical empty-value contract', async () => {
    const initial = condition();
    const onChange = vi.fn();
    render(<Harness initial={group([initial])} onChange={onChange} />);
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.filter.value_source' })
    );
    fireEvent.click(
      await screen.findByRole('menuitem', { name: 'Current user' })
    );
    await expectChange(
      onChange,
      group([{ ...initial, value_source: 'variable', value: 'currentUser.id' }])
    );
    expect(
      screen.getByRole('combobox', { name: 'data_blocks.filter.variable' })
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.filter.value_source' })
    );
    fireEvent.click(
      await screen.findByRole('menuitem', {
        name: 'data_blocks.filter.literal'
      })
    );
    await expectChange(
      onChange,
      group([{ ...initial, value_source: 'literal', value: undefined }])
    );
    await select(
      screen.getByRole('combobox', { name: 'data_blocks.filter.operator' }),
      'data_blocks.filter.is_empty'
    );
    await expectChange(
      onChange,
      group([
        { ...initial, operator: '$eq', value_source: 'literal', value: null }
      ])
    );
    expect(
      screen.queryByRole('textbox', { name: 'data_blocks.filter.value' })
    ).not.toBeInTheDocument();
    await select(
      screen.getByRole('combobox', { name: 'data_blocks.filter.operator' }),
      'data_blocks.filter.is_not_empty'
    );
    await expectChange(
      onChange,
      group([
        { ...initial, operator: '$ne', value_source: 'literal', value: null }
      ])
    );
    await select(
      screen.getByRole('combobox', { name: 'data_blocks.filter.field' }),
      'Amount'
    );
    const numeric = {
      ...initial,
      field_code: 'amount',
      operator: '$eq' as const,
      value_source: 'literal' as const,
      value: undefined
    };
    await expectChange(onChange, group([numeric]));
    await select(
      screen.getByRole('combobox', { name: 'data_blocks.filter.operator' }),
      'data_blocks.filter.greater'
    );
    await expectChange(onChange, group([{ ...numeric, operator: '$gt' }]));
  });

  it('keeps numeric zero as a number', async () => {
    const initial = condition({ field_code: 'amount', value: 3 });
    const onChange = vi.fn();
    render(<Harness initial={group([initial])} onChange={onChange} />);
    fireEvent.change(
      screen.getByRole('spinbutton', { name: 'data_blocks.filter.value' }),
      { target: { value: '0' } }
    );
    await expectChange(onChange, group([{ ...initial, value: 0 }]));
  });

  it.each([
    {
      field_code: 'enabled',
      current: true,
      label: 'data_blocks.common.no',
      expected: false
    },
    { field_code: 'choice', current: 7, label: 'Nine', expected: 9 }
  ])(
    'keeps the $field_code scalar value type',
    async ({ field_code, current, label, expected }) => {
      const initial = condition({ field_code, value: current });
      const onChange = vi.fn();
      render(<Harness initial={group([initial])} onChange={onChange} />);
      await select(
        screen.getByRole('combobox', { name: 'data_blocks.filter.value' }),
        label
      );
      await expectChange(onChange, group([{ ...initial, value: expected }]));
    }
  );

  it.each([
    { field_code: 'choice', current: 7, label: 'Nine', expected: 9 },
    {
      field_code: 'enabled',
      current: true,
      label: 'data_blocks.common.no',
      expected: false
    }
  ])(
    'keeps typed $in values for $field_code',
    async ({ field_code, current, label, expected }) => {
      const initial = condition({
        field_code,
        operator: '$in',
        value: [current]
      });
      const onChange = vi.fn();
      render(<Harness initial={group([initial])} onChange={onChange} />);
      await select(
        screen.getByRole('combobox', { name: 'data_blocks.filter.value' }),
        label
      );
      await expectChange(
        onChange,
        group([{ ...initial, value: [current, expected] }])
      );
    }
  );

  it('creates UUIDs only for clicked Add actions and keeps condition and group payloads distinct', async () => {
    const onChange = vi.fn();
    const randomUUID = vi.spyOn(crypto, 'randomUUID');
    render(<Harness initial={group()} onChange={onChange} />);
    expect(randomUUID).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.filter.add_condition' })
    );
    expect(randomUUID).toHaveBeenCalledTimes(1);
    const first = {
      id: randomUUID.mock.results[0].value,
      field_code: '',
      operator: '$eq' as const,
      value: undefined
    };
    await expectChange(onChange, group([first]));
    fireEvent.click(
      screen.getByRole('button', { name: 'data_blocks.filter.add_group' })
    );
    expect(randomUUID).toHaveBeenCalledTimes(2);
    await expectChange(
      onChange,
      group([
        first,
        {
          id: randomUUID.mock.results[1].value,
          conjunction: 'and',
          conditions: []
        }
      ])
    );
  });

  it('disables Add condition without fields while leaving Add group available', () => {
    const onChange = vi.fn();
    render(
      <Harness initial={group()} onChange={onChange} availableFields={[]} />
    );
    const add = screen.getByRole('button', {
      name: 'data_blocks.filter.add_condition'
    });
    expect(add).toBeDisabled();
    fireEvent.click(add);
    expect(onChange).not.toHaveBeenCalled();
    expect(
      screen.getByRole('button', { name: 'data_blocks.filter.add_group' })
    ).toBeEnabled();
  });
});
