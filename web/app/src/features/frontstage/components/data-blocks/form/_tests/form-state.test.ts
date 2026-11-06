import { describe, expect, it } from 'vitest';
import {
  createFormBlockConfig,
  type DataBlockField,
  type DataFormItem
} from '../../../../lib/data-blocks/types';
import {
  defaultFormValues,
  fieldState,
  formWriteValues,
  moveFormItem,
  recordIdentity
} from '../form-state';

const binding = { model_id: 'm', model_code: 'orders', data_source_id: null };
const field = (code: string, extra = {}) =>
  ({
    code,
    title: code,
    field_kind: 'string',
    is_writable: true,
    is_required: false,
    api_required: false,
    ...extra
  }) as DataBlockField;
const item = (code: string, extra = {}): DataFormItem => ({
  id: code,
  kind: 'field',
  field_code: code,
  row: code,
  span: 24,
  ...extra
});

describe('form record and design boundaries', () => {
  it('preserves false and zero defaults and never imports an edit record into create', () => {
    expect(
      defaultFormValues([
        item('count', { default_value: 0 }),
        item('active', { default_value: false })
      ])
    ).toEqual({ count: 0, active: false });
    expect(
      recordIdentity(
        { ...createFormBlockConfig(binding), mode: 'create' },
        { record_id: 'A' }
      )
    ).toBe('orders:create:');
    expect(
      recordIdentity({ ...createFormBlockConfig(binding), mode: 'edit' }, {})
    ).toBe('orders:edit:');
  });
  it('isolates A/B identities and models', () => {
    const config = { ...createFormBlockConfig(binding), mode: 'edit' as const };
    expect(recordIdentity(config, { record_id: 'A' })).not.toBe(
      recordIdentity(config, { record_id: 'B' })
    );
    expect(recordIdentity(config, { record_id: 'A' })).not.toBe(
      recordIdentity(
        { ...config, binding: { ...binding, model_code: 'users' } },
        { record_id: 'A' }
      )
    );
  });
  it('does not let optional/editable rules weaken backend constraints', () => {
    const config = createFormBlockConfig(binding);
    config.rules = ['optional', 'editable'].map((effect) => ({
      id: effect,
      condition: { id: 'r', conjunction: 'and', conditions: [] },
      target_field: 'locked',
      effect: effect as 'optional' | 'editable'
    }));
    expect(
      fieldState(
        field('locked', { is_writable: false, api_required: true }),
        item('locked'),
        config,
        {}
      )
    ).toMatchObject({ required: true, readOnly: true });
  });
  it('writes only visible writable participating fields and injects the parent ID', () => {
    const config = createFormBlockConfig(binding);
    config.items = [
      item('name'),
      item('readonly', { read_only: true }),
      item('hidden')
    ];
    config.rules = [
      {
        id: 'hide',
        condition: { id: 'r', conjunction: 'and', conditions: [] },
        target_field: 'hidden',
        effect: 'hide'
      }
    ];
    config.relation = {
      field_code: 'customer',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    const fields = ['name', 'readonly', 'hidden', 'customer'].map((code) =>
      field(code)
    );
    expect(
      formWriteValues(
        config,
        fields,
        { name: 'Draft', readonly: 'No', hidden: 'Retained' },
        { parent_record_id: 'parent-1' }
      )
    ).toEqual({ name: 'Draft', customer: 'parent-1' });
    expect(() => formWriteValues(config, fields, {}, {})).toThrow(
      'missing_parent'
    );
  });
  it('does not invent relation IDs from display objects', () => {
    const config = {
      ...createFormBlockConfig(binding),
      items: [item('customer')]
    };
    expect(() =>
      formWriteValues(
        config,
        [field('customer', { field_kind: 'many_to_one' })],
        { customer: { title: 'Alice' } },
        {}
      )
    ).toThrow('invalid_relation');
  });
  it('moves into the same row or a new row without touching business drafts', () => {
    const items = [item('a'), item('b'), item('c')];
    const joined = moveFormItem(items, 'c', 'a', 'beside');
    expect(joined.filter((i) => i.row === 'a').map((i) => i.span)).toEqual([
      12, 12
    ]);
    expect(items[2].row).toBe('c');
    expect(moveFormItem(joined, 'c', 'b', 'before').map((i) => i.id)).toEqual([
      'a',
      'c',
      'b'
    ]);
  });
});
