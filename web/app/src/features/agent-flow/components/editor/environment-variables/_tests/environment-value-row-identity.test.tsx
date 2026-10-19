import { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test } from 'vitest';
import { EnvironmentVariableValueEditor } from '../EnvironmentVariableValueEditor';
import { i18nText } from '../../../../../../shared/i18n/text';

function ArrayHarness() {
  const [value, setValue] = useState<unknown>([
    { first: 'a' },
    { second: 'b' }
  ]);
  return (
    <EnvironmentVariableValueEditor
      valueType="array[object]"
      value={value}
      onChange={setValue}
    />
  );
}

test('deleting a preceding array object preserves the surviving empty field draft and DOM identity', () => {
  const { container } = render(<ArrayHarness />);
  const survivor = container.querySelectorAll(
    '.agent-flow-editor__env-array-row'
  )[1]!;
  const input = survivor.querySelector('input')!;
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.add_array_object_field', { value1: 2 })
    })
  );
  expect(
    survivor.querySelectorAll('.agent-flow-editor__env-object-row')
  ).toHaveLength(2);
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.delete_array_item', { value1: 1 })
    })
  );
  expect(container.querySelector('.agent-flow-editor__env-array-row')).toBe(
    survivor
  );
  expect(survivor.querySelector('input')).toBe(input);
  expect(
    survivor.querySelectorAll('.agent-flow-editor__env-object-row')
  ).toHaveLength(2);
});

function ObjectHarness() {
  const [value, setValue] = useState<unknown>({ first: 'a', second: 'b' });
  return (
    <EnvironmentVariableValueEditor
      valueType="object"
      value={value}
      onChange={setValue}
    />
  );
}

test('deleting a preceding object field preserves the surviving input instance', () => {
  const { container } = render(<ObjectHarness />);
  const survivor = container.querySelectorAll(
    '.agent-flow-editor__env-object-row'
  )[1]!;
  const input = survivor.querySelector('input')!;
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.delete_field', {
        value1: i18nText('agentFlow', 'auto.object'),
        value2: 1
      })
    })
  );
  expect(container.querySelector('.agent-flow-editor__env-object-row')).toBe(
    survivor
  );
  expect(survivor.querySelector('input')).toBe(input);
});
