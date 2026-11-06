import { describe, expect, it, vi } from 'vitest';
import type { DataBlockHost } from '../../../../lib/data-blocks/host-context';
import {
  createTableBlockConfig,
  emptyDataFilter,
  type DataBlockAction,
  type DataBlockField
} from '../../../../lib/data-blocks/types';
import {
  deleteTableRecords,
  persistTableConfig,
  prepareTableAction,
  tableActionAllowed,
  tableActionInputs,
  tableActionParentContext,
  tableQueryScope,
  tableRecordId
} from '../table-model';

const binding = {
  model_id: 'orders-model',
  model_code: 'orders',
  data_source_id: null
};
const capabilities = {
  can_list: true,
  can_get: true,
  can_create: true,
  can_update: true,
  can_delete: true
};
const action: DataBlockAction = {
  id: 'edit',
  kind: 'edit',
  enabled: true,
  presentation: 'modal'
};
function host(overrides: Partial<DataBlockHost> = {}): DataBlockHost {
  return {
    block_id: 'orders-table',
    design_mode: true,
    saving: false,
    targets: [],
    saveConfig: vi.fn().mockResolvedValue(undefined),
    prepareAction: vi
      .fn()
      .mockResolvedValue({ ...action, target_block_id: 'child-form' }),
    openAction: vi.fn(),
    close: vi.fn(),
    ...overrides
  };
}

describe('table backend scope and record identity', () => {
  it('intersects fixed OR groups, user filters, relation and selection linkage without weakening the fixed scope', () => {
    const config = createTableBlockConfig(binding);
    config.filter = {
      id: 'fixed',
      conjunction: 'or',
      conditions: [
        { id: 'a', field_code: 'status', operator: '$eq', value: 'open' },
        { id: 'b', field_code: 'status', operator: '$eq', value: 'pending' }
      ]
    };
    config.relation = {
      field_code: 'customer_id',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    config.linkage = {
      source_block_id: 'invoices',
      source_output: 'selected_record_ids',
      field_code: 'invoice_id'
    };
    const user = {
      ...emptyDataFilter(),
      conditions: [
        {
          id: 'user',
          field_code: 'amount',
          operator: '$gt' as const,
          value: 100
        }
      ]
    };
    expect(
      tableQueryScope(config, user, {
        parent_record_id: 'customer-A',
        linked_value: ['invoice-B', 'invoice-C']
      })
    ).toEqual({
      enabled: true,
      filter: {
        $and: [
          {
            $or: [{ status: { $eq: 'open' } }, { status: { $eq: 'pending' } }]
          },
          { $and: [{ amount: { $gt: 100 } }] },
          { customer_id: { $eq: 'customer-A' } },
          { invoice_id: { $in: ['invoice-B', 'invoice-C'] } }
        ]
      }
    });
  });
  it('disables related data before a parent or source record is selected', () => {
    const config = createTableBlockConfig(binding);
    config.relation = {
      field_code: 'parent',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    expect(tableQueryScope(config, emptyDataFilter(), {}).enabled).toBe(false);
    config.linkage = {
      source_block_id: 'source',
      source_output: 'selected_record_ids',
      field_code: 'invoice'
    };
    expect(
      tableQueryScope(config, emptyDataFilter(), {
        parent_record_id: 'A',
        linked_value: []
      }).enabled
    ).toBe(false);
  });
  it('uses the metadata primary key, including numeric zero, and never falls back to id', () => {
    expect(tableRecordId({ order_no: 'A', id: 'wrong' }, 'order_no')).toBe('A');
    expect(tableRecordId({ order_no: 0 }, 'order_no')).toBe('0');
    expect(tableRecordId({ id: 'wrong' }, 'order_no')).toBeNull();
    expect(tableRecordId({ order_no: {} }, 'order_no')).toBeNull();
  });
  it('opens a row with its real ID and keeps the parent and source context distinct', () => {
    const config = createTableBlockConfig(binding);
    config.relation = {
      field_code: 'customer_id',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    expect(
      tableActionInputs(
        config,
        'orders-table',
        { parent_record_id: 'customer-A', unrelated: 'not-forwarded' },
        'order-B'
      )
    ).toEqual({
      record_id: 'order-B',
      source_block_id: 'orders-table',
      source_model_code: 'orders',
      parent_record_id: 'customer-A',
      parent_model_code: 'customers'
    });
    expect(
      tableActionInputs(config, 'orders-table', {
        parent_record_id: 'customer-A'
      })
    ).not.toHaveProperty('record_id');
  });
});

describe('table data and design write boundaries', () => {
  it('never deletes without selection and reports independent partial failures for retry', async () => {
    const remove = vi.fn(async (id: string) => {
      if (id === 'B') throw new Error('forbidden');
    });
    expect(await deleteTableRecords([], remove)).toEqual({
      deleted: [],
      failed: []
    });
    expect(remove).not.toHaveBeenCalled();
    expect(await deleteTableRecords(['A', 'B', 'C', 'A'], remove)).toEqual({
      deleted: ['A', 'C'],
      failed: ['B']
    });
    expect(remove.mock.calls.map(([id]) => id)).toEqual(['A', 'B', 'C']);
  });
  it('does not expose forbidden record actions even if configured', () => {
    expect(
      tableActionAllowed(action, { ...capabilities, can_update: false })
    ).toBe(false);
    expect(
      tableActionAllowed(action, { ...capabilities, can_get: false })
    ).toBe(false);
    expect(
      tableActionAllowed(
        { ...action, kind: 'delete' },
        { ...capabilities, can_delete: false }
      )
    ).toBe(false);
    expect(
      tableActionAllowed({ ...action, enabled: false }, capabilities)
    ).toBe(false);
    expect(
      tableActionAllowed({ ...action, kind: 'create' }, capabilities)
    ).toBe(true);
  });
  it('does not apply a configuration when persistence fails', async () => {
    const committed = vi.fn();
    const config = createTableBlockConfig(binding);
    const failedHost = host({
      saveConfig: vi.fn().mockRejectedValue(new Error('save rejected'))
    });
    await expect(
      persistTableConfig(failedHost, config, committed)
    ).rejects.toThrow('save rejected');
    expect(committed).not.toHaveBeenCalled();
    await persistTableConfig(host(), config, committed);
    expect(committed).toHaveBeenCalledWith(config);
  });
  it('prepares a real child target for create/edit/view and column-open with the correct mode and relation', async () => {
    const owner = host();
    const config = createTableBlockConfig(binding);
    config.relation = {
      field_code: 'customer_id',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    };
    for (const [kind, mode] of [
      ['create', 'create'],
      ['edit', 'edit'],
      ['view', 'view'],
      ['open', 'view']
    ] as const) {
      const candidate = { ...action, kind };
      const prepared = await prepareTableAction(owner, config, candidate);
      expect(owner.prepareAction).toHaveBeenLastCalledWith(
        candidate,
        binding,
        mode,
        config.relation
      );
      expect(prepared.target_block_id).toBe('child-form');
    }
    const disabled = { ...action, enabled: false };
    expect(await prepareTableAction(owner, config, disabled)).toBe(disabled);
    expect(owner.prepareAction).toHaveBeenCalledTimes(4);
  });
});

describe('linked parent form writes', () => {
  const customerField = {
    id: 'customer-field',
    code: 'customer_id',
    title: 'Customer',
    description: '',
    field_kind: 'many_to_one',
    is_required: false,
    api_required: false,
    is_writable: true,
    default_value: null,
    display_interface: 'relation',
    display_options: null,
    relation_target_model_id: 'customers-model',
    relation_target_model_code: 'customers',
    relation_options: null
  } as DataBlockField;
  it('passes the selected parent into a linked create form and refuses ambiguous multiple parents', async () => {
    const config = createTableBlockConfig(binding);
    config.linkage = {
      source_block_id: 'customers-table',
      source_output: 'selected_record_ids',
      field_code: 'customer_id'
    };
    const context = tableActionParentContext(config, [customerField], {
      linked_value: ['customer-A']
    });
    expect(context.canCreate).toBe(true);
    expect(context.relation).toEqual({
      field_code: 'customer_id',
      parent_model_code: 'customers',
      parent_record_input: 'parent_record_id'
    });
    expect(
      tableActionInputs(
        { ...config, relation: context.relation },
        'orders-table',
        context.inputs
      )
    ).toMatchObject({
      parent_record_id: 'customer-A',
      parent_model_code: 'customers'
    });
    expect(
      tableActionParentContext(config, [customerField], {
        linked_value: ['A', 'B']
      }).canCreate
    ).toBe(false);
    expect(
      tableActionParentContext(config, [customerField], {}).canCreate
    ).toBe(false);
    const owner = host();
    await prepareTableAction(owner, config, { ...action, kind: 'create' }, [
      customerField
    ]);
    expect(owner.prepareAction).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'create' }),
      binding,
      'create',
      context.relation
    );
  });
});
