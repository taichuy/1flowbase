import { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test } from 'vitest';
import type { FlowVariableGroupDocument } from '@1flowbase/flow-schema';
import { VariableGroupsField } from '../VariableGroupsField';
import { i18nText } from '../../../../../shared/i18n/text';

function Harness() {
  const [value, setValue] = useState<FlowVariableGroupDocument[]>([
    { key: 'group1', valueType: 'string', candidates: [[]] },
    { key: 'group2', valueType: 'string', candidates: [[]] }
  ]);
  return (
    <VariableGroupsField
      ariaLabel="Groups"
      value={value}
      options={[]}
      onChange={setValue}
    />
  );
}

test('a surviving variable group keeps its section and input when an earlier group is removed', () => {
  const { container } = render(<Harness />);
  const survivor = container.querySelectorAll(
    '.agent-flow-variable-groups__item'
  )[1]!;
  const input = survivor.querySelector('input')!;
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.delete_variable_group', {
        value1: 'group1'
      })
    })
  );
  expect(container.querySelector('.agent-flow-variable-groups__item')).toBe(
    survivor
  );
  expect(survivor.querySelector('input')).toBe(input);
});
