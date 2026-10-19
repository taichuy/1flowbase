import { useState } from 'react';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { VariableAssignmentField, type VariableAssignmentValue } from '../components/bindings/VariableAssignmentField';
import type { FlowSelectorOption } from '../lib/selector-options';

vi.mock('../components/bindings/TemplatedTextField', () => ({
  TemplatedTextField: ({
    ariaLabel,
    label,
    value
  }: {
    ariaLabel: string;
    label: string;
    value: string;
  }) => (
    <label>
      {label}
      <input aria-label={ariaLabel} readOnly value={value} />
    </label>
  )
}));

const selectorOptions: FlowSelectorOption[] = [
  {
    nodeId: 'node-start',
    nodeLabel: 'Start',
    outputKey: 'query',
    outputLabel: 'query',
    valueType: 'string',
    value: ['node-start', 'query'],
    displayLabel: 'Start/query'
  }
];

describe('VariableAssignmentField', () => {
  test('uses templated text value editing for string conversation variables', () => {
    render(
      <VariableAssignmentField
        ariaLabel="变量赋值"
        conversationVariables={[
          {
            name: 'ApiBaseUrl',
            valueType: 'string',
            description: ''
          }
        ]}
        selectorOptions={selectorOptions}
        value={[
          {
            path: ['conversation', 'ApiBaseUrl'],
            operator: 'set',
            value: {
              kind: 'templated_text',
              value: 'https://{{node-start.query}}/v1'
            }
          }
        ]}
        onChange={vi.fn()}
      />
    );

    expect(screen.getByLabelText('变量赋值-0-value')).toHaveValue(
      'https://{{node-start.query}}/v1'
    );
    expect(
      screen.getAllByText('conversation.ApiBaseUrl').length
    ).toBeGreaterThan(0);
    expect(screen.queryByText('env.ApiBaseUrl')).not.toBeInTheDocument();
  });
});


test('keeps the surviving assignment editor mounted when deleting an earlier row', () => {
  function Harness() {
    const [value, setValue] = useState<VariableAssignmentValue[]>([
      { path: ['conversation', 'ApiBaseUrl'], operator: 'set', value: { kind: 'templated_text', value: 'first' } },
      { path: ['conversation', 'ApiBaseUrl'], operator: 'set', value: { kind: 'templated_text', value: 'second' } }
    ]);
    return <VariableAssignmentField ariaLabel="assignments" value={value}
      conversationVariables={[{ name: 'ApiBaseUrl', valueType: 'string', description: '' }]}
      selectorOptions={selectorOptions} onChange={setValue} />;
  }
  const { container } = render(<Harness />);
  const survivor = screen.getByLabelText('assignments-1-value');
  const first = container.querySelector('.agent-flow-variable-assignment-row')!;
  fireEvent.click(within(first as HTMLElement).getByRole('button'));
  expect(screen.getByLabelText('assignments-0-value')).toBe(survivor);
  expect(survivor).toHaveValue('second');
});
