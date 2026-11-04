import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import { SelectorField } from '../../../components/bindings/SelectorField';

const base = {
  nodeId: 'code',
  nodeLabel: 'Compute',
  outputKey: 'result',
  outputLabel: 'Whole result',
  valueType: 'object',
  displayLabel: 'Compute / Whole result',
  value: ['code', 'result']
};
describe('backend catalog selection', () => {
  test('selects a declared whole object even when the catalog also contains nested fields', async () => {
    const onChange = vi.fn();
    render(
      <SelectorField
        ariaLabel="Source variable"
        value={[]}
        onChange={onChange}
        options={[
          base,
          {
            ...base,
            outputKey: 'score',
            outputLabel: 'Score',
            valueType: 'number',
            value: ['code', 'result', 'score']
          }
        ]}
      />
    );
    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: 'Source variable' })
    );
    fireEvent.click(await screen.findByTitle('Compute'));
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByTitle('Whole result'));
    await waitFor(() =>
      expect(onChange).toHaveBeenCalledWith(['code', 'result'])
    );
  });
});
