import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { expect, test, vi } from 'vitest';
import { McpInputMappingEditor } from '../McpInputMappingEditor';
import {
  normalizeInputMapping,
  parseMappingDefault,
  mappingDefaultError
} from '../mcp-input-mapping-model';

vi.mock(
  '../../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel',
  () => ({ InlineJsonCodeEditor: () => null })
);

const initial = {
  interface_parameters: [
    {
      name: 'count',
      field_type: 'integer',
      parameter_type: 'json_body',
      required: true
    }
  ],
  mappings: [
    { interface_param: 'count', mcp_param: 'body.count', required: true }
  ]
};

test('editing defaults and hidden state survives parent updates and reports missing required defaults', async () => {
  const changed = vi.fn();
  const valid = vi.fn();
  function Editor() {
    const [value, setValue] = useState<unknown>(initial);
    return (
      <McpInputMappingEditor
        value={value}
        onValidityChange={valid}
        onChange={(next) => {
          changed(next);
          setValue(next);
        }}
      />
    );
  }
  render(<Editor />);
  const tabs = screen.getAllByRole('tab');
  fireEvent.click(tabs[1]);
  fireEvent.click(screen.getByRole('checkbox', { name: 'hidden count' }));
  expect(valid).toHaveBeenLastCalledWith(false);
  fireEvent.change(
    screen.getByRole('textbox', { name: 'default_value count' }),
    { target: { value: '0' } }
  );
  expect(valid).toHaveBeenLastCalledWith(true);
  expect(changed.mock.lastCall?.[0].mappings[0]).toMatchObject({
    hidden: true,
    default_value: 0
  });
  expect(
    screen.getByRole('textbox', { name: 'default_value count' })
  ).toHaveValue('0');
  fireEvent.change(
    screen.getByRole('textbox', { name: 'default_value count' }),
    { target: { value: 'nope' } }
  );
  expect(valid).toHaveBeenLastCalledWith(false);
});

test('normalization preserves configured values and parser retains JSON types', () => {
  for (const value of [false, 0, [], {}, 'hello']) {
    const normalized = normalizeInputMapping({
      ...initial,
      mappings: [{ ...initial.mappings[0], hidden: true, default_value: value }]
    });
    expect(normalized.mappings[0]).toMatchObject({
      hidden: true,
      default_value: value
    });
  }
  expect(parseMappingDefault('false', 'boolean')).toBe(false);
  expect(parseMappingDefault('[]', 'array<string>')).toEqual([]);
  expect(parseMappingDefault('0', 'string')).toBe('0');
  expect(parseMappingDefault('', 'string')).toBeNull();
  expect(
    mappingDefaultError(normalizeInputMapping(initial).mappings[0])
  ).toBeUndefined();
});
