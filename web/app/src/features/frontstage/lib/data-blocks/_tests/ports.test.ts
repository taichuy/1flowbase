import { describe, expect, it } from 'vitest';
import {
  dataBlockPorts,
  dataBlockSurfaceWidth,
  mergeDataBlockPorts
} from '../ports';
import { createFormBlockConfig, createTableBlockConfig } from '../types';

const binding = {
  model_id: 'orders-model',
  model_code: 'orders',
  data_source_id: null
};
const sharedInputs = [
  { name: 'record_id', schema: { type: 'string' } },
  { name: 'source_block_id', schema: { type: 'string' } },
  { name: 'source_model_code', schema: { type: 'string' } },
  { name: 'parent_record_id', schema: { type: 'string' } }
];

describe('data block source and parent ports', () => {
  it('declares row identity, originating block/model and parent identity as independent string inputs', () => {
    const table = dataBlockPorts(createTableBlockConfig(binding));
    const form = dataBlockPorts(createFormBlockConfig(binding, 'edit'));
    expect(table.inputs).toEqual(sharedInputs);
    expect(form.inputs).toEqual(sharedInputs);
    expect(table.inputs.every((input) => !input.source)).toBe(true);
    expect(form.inputs.every((input) => !input.source)).toBe(true);
  });

  it('publishes table selection IDs independently from form save results', () => {
    expect(dataBlockPorts(createTableBlockConfig(binding)).outputs).toEqual([
      { name: 'record_id', schema: { type: 'string' } },
      {
        name: 'selected_record_ids',
        schema: { type: 'array', items: { type: 'string' } }
      }
    ]);
    expect(dataBlockPorts(createFormBlockConfig(binding)).outputs).toEqual([
      { name: 'saved_record_id', schema: { type: 'string' } },
      { name: 'save_revision', schema: { type: 'integer' } },
      { name: 'saved_record', schema: { type: 'object' } }
    ]);
  });

  it('adds the configured parent input without deriving a source block from the parent model or business field', () => {
    const config = createFormBlockConfig(binding);
    config.relation = {
      field_code: 'customer_id',
      parent_model_code: 'customers',
      parent_record_input: 'selected_customer_id'
    };
    const ports = dataBlockPorts(config);
    expect(ports.inputs).toEqual([
      ...sharedInputs,
      { name: 'selected_customer_id', schema: { type: 'string' } }
    ]);
    expect(
      ports.inputs.find((input) => input.name === 'selected_customer_id')
    ).not.toHaveProperty('source');
    expect(ports.inputs.map((input) => input.name)).not.toContain(
      'customer_id'
    );
    expect(ports.inputs.map((input) => input.name)).not.toContain('customers');
  });

  it.each([
    'parent_record_id',
    'record_id',
    'source_block_id',
    'source_model_code'
  ])(
    'keeps input names unique when the configured parent input reuses %s',
    (parent_record_input) => {
      const config = createTableBlockConfig(binding);
      config.relation = {
        field_code: 'customer_id',
        parent_model_code: 'customers',
        parent_record_input
      };
      const ports = dataBlockPorts(config);
      expect(
        ports.inputs.filter((input) => input.name === parent_record_input)
      ).toEqual([{ name: parent_record_input, schema: { type: 'string' } }]);
      expect(new Set(ports.inputs.map((input) => input.name)).size).toBe(
        ports.inputs.length
      );
    }
  );
});

describe('linked data block ports', () => {
  it('binds a single source record to linked_value in page scope', () => {
    const config = createTableBlockConfig(binding);
    config.linkage = {
      source_block_id: 'customers-table',
      source_output: 'record_id',
      field_code: 'customer_id'
    };
    const ports = dataBlockPorts(config);
    expect(ports.inputs).toEqual([
      ...sharedInputs,
      {
        name: 'linked_value',
        schema: { type: 'string' },
        source: {
          block_id: 'customers-table',
          output: 'record_id',
          scope: 'page'
        }
      }
    ]);
    expect(ports.outputs).toEqual(
      dataBlockPorts(createTableBlockConfig(binding)).outputs
    );
  });

  it('keeps the multi-selection output an array and preserves an independent custom parent input', () => {
    const config = createTableBlockConfig(binding);
    config.relation = {
      field_code: 'region_id',
      parent_model_code: 'regions',
      parent_record_input: 'selected_region_id'
    };
    config.linkage = {
      source_block_id: 'customers-table',
      source_output: 'selected_record_ids',
      field_code: 'customer_id'
    };
    const before = structuredClone(config);
    const ports = dataBlockPorts(config);
    expect(ports.inputs.find((input) => input.name === 'linked_value')).toEqual(
      {
        name: 'linked_value',
        schema: { type: 'array', items: { type: 'string' } },
        source: {
          block_id: 'customers-table',
          output: 'selected_record_ids',
          scope: 'page'
        }
      }
    );
    expect(
      ports.inputs.find((input) => input.name === 'selected_region_id')
    ).toEqual({ name: 'selected_region_id', schema: { type: 'string' } });
    expect(ports.inputs.filter((input) => input.source)).toHaveLength(1);
    expect(config).toEqual(before);
  });

  it('drops the old source mapping when linkage is cleared or changed', () => {
    const config = createTableBlockConfig(binding);
    config.linkage = {
      source_block_id: 'old-table',
      source_output: 'selected_record_ids',
      field_code: 'customer_id'
    };
    expect(
      dataBlockPorts(config).inputs.find(
        (input) => input.name === 'linked_value'
      )?.source?.block_id
    ).toBe('old-table');
    config.linkage = {
      source_block_id: 'new-table',
      source_output: 'record_id',
      field_code: 'customer_id'
    };
    expect(
      dataBlockPorts(config).inputs.find(
        (input) => input.name === 'linked_value'
      )
    ).toEqual({
      name: 'linked_value',
      schema: { type: 'string' },
      source: { block_id: 'new-table', output: 'record_id', scope: 'page' }
    });
    delete config.linkage;
    expect(dataBlockPorts(config).inputs).toEqual(sharedInputs);
  });
});

describe('data form surface sizes', () => {
  it('maps persisted form sizes and supplies the default width for other content', () => {
    const form = createFormBlockConfig(binding);
    expect(dataBlockSurfaceWidth(form)).toBe(720);
    expect(dataBlockSurfaceWidth({ ...form, surface_size: 'small' })).toBe(480);
    expect(dataBlockSurfaceWidth({ ...form, surface_size: 'middle' })).toBe(
      720
    );
    expect(dataBlockSurfaceWidth({ ...form, surface_size: 'large' })).toBe(
      'min(1100px, 94vw)'
    );
    expect(dataBlockSurfaceWidth(createTableBlockConfig(binding))).toBe(720);
    expect(dataBlockSurfaceWidth(null)).toBe(720);
  });
});

describe('data block port merge ownership', () => {
  it('preserves custom author inputs, outputs and sources while keeping generated record schemas canonical', () => {
    const previous = createTableBlockConfig(binding);
    const generated = dataBlockPorts(previous);
    const recordSource = {
      block_id: 'record-picker',
      output: 'current_id',
      scope: 'tab' as const,
      tab_id: 'orders-tab'
    };
    const customInput = {
      name: 'customer_context',
      schema: { type: 'object', properties: { name: { type: 'string' } } },
      source: {
        block_id: 'customer-summary',
        output: 'customer',
        scope: 'page' as const
      }
    };
    const customOutput = {
      name: 'export_requested',
      schema: { type: 'boolean' }
    };
    const current = {
      inputs: [
        ...generated.inputs.map((port) =>
          port.name === 'record_id'
            ? { ...port, schema: { type: 'number' }, source: recordSource }
            : port
        ),
        customInput
      ],
      outputs: [...generated.outputs, customOutput]
    };
    const before = structuredClone(current);
    const merged = mergeDataBlockPorts(current, previous, {
      ...previous,
      title: 'Updated table'
    });
    expect(merged.inputs).toContainEqual(customInput);
    expect(merged.outputs).toContainEqual(customOutput);
    expect(merged.inputs.find((port) => port.name === 'record_id')).toEqual({
      name: 'record_id',
      schema: { type: 'string' },
      source: recordSource
    });
    expect(new Set(merged.inputs.map((port) => port.name)).size).toBe(
      merged.inputs.length
    );
    expect(current).toEqual(before);
  });

  it('replaces auto linkage mappings and cardinality without disturbing an authored record_id source', () => {
    const previous = createTableBlockConfig(binding);
    previous.linkage = {
      source_block_id: 'old-table',
      source_output: 'selected_record_ids',
      field_code: 'customer_id'
    };
    const current = dataBlockPorts(previous);
    const recordSource = {
      block_id: 'detail-picker',
      output: 'record_id',
      scope: 'page' as const
    };
    current.inputs = current.inputs.map((port) =>
      port.name === 'record_id' ? { ...port, source: recordSource } : port
    );
    const next = {
      ...previous,
      linkage: {
        source_block_id: 'new-table',
        source_output: 'record_id' as const,
        field_code: 'customer_id'
      }
    };
    const merged = mergeDataBlockPorts(current, previous, next);
    expect(
      merged.inputs.filter((port) => port.name === 'linked_value')
    ).toEqual([
      {
        name: 'linked_value',
        schema: { type: 'string' },
        source: { block_id: 'new-table', output: 'record_id', scope: 'page' }
      }
    ]);
    expect(
      merged.inputs.find((port) => port.name === 'record_id')?.source
    ).toEqual(recordSource);
  });

  it('removes stale generated linkage and parent inputs after clearing settings while preserving unrelated author ports', () => {
    const previous = createTableBlockConfig(binding);
    previous.linkage = {
      source_block_id: 'customers',
      source_output: 'selected_record_ids',
      field_code: 'customer_id'
    };
    previous.relation = {
      field_code: 'region_id',
      parent_model_code: 'regions',
      parent_record_input: 'selected_region_id'
    };
    const current = dataBlockPorts(previous);
    const customInput = {
      name: 'report_year',
      schema: { type: 'integer' },
      source: {
        block_id: 'year-picker',
        output: 'year',
        scope: 'page' as const
      }
    };
    current.inputs.push(customInput);
    const merged = mergeDataBlockPorts(
      current,
      previous,
      createTableBlockConfig(binding)
    );
    expect(merged.inputs.map((port) => port.name)).not.toContain(
      'linked_value'
    );
    expect(merged.inputs.map((port) => port.name)).not.toContain(
      'selected_region_id'
    );
    expect(merged.inputs).toContainEqual(customInput);
    expect(merged.inputs.filter((port) => port.source)).toEqual([customInput]);
  });

  it('initializes missing stored ports while preserving author-defined ports on the first configuration save', () => {
    const config = createFormBlockConfig(binding);
    expect(mergeDataBlockPorts(null, null, config)).toEqual(
      dataBlockPorts(config)
    );
    const customOutput = { name: 'form_closed', schema: { type: 'boolean' } };
    const merged = mergeDataBlockPorts(
      { inputs: [], outputs: [customOutput] },
      null,
      config
    );
    expect(merged.outputs).toEqual([
      customOutput,
      ...dataBlockPorts(config).outputs
    ]);
  });
});
