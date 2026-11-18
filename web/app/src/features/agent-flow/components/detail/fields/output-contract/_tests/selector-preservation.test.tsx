import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import { i18nText } from '../../../../../../../shared/i18n/text';
import {
  codeOutputSelector,
  normalizeCodeOutput
} from '../../../../../lib/output-contract/code-output';
import { OutputContractDefinitionField } from '../OutputContractDefinitionField';

const schema = { type: 'object', properties: { score: { type: 'number' } } };
describe('explicit Code output selectors', () => {
  test('normalization preserves whole result, nested paths, and schema', () => {
    for (const selector of [['result'], ['result', 'report', 'score'], []]) {
      expect(
        normalizeCodeOutput({
          key: 'report',
          title: 'Report',
          valueType: 'object',
          selector,
          jsonSchema: schema
        })
      ).toMatchObject({ selector, jsonSchema: schema });
    }
    expect(
      normalizeCodeOutput({ key: 'score', title: 'Score', valueType: 'number' })
        .selector
    ).toEqual(['result', 'score']);
  });

  test('editing the output name retains its explicit whole-object selector and schema', () => {
    const onChange = vi.fn();
    render(
      <OutputContractDefinitionField
        title="Outputs"
        value={[
          {
            key: 'report',
            title: 'report',
            valueType: 'object',
            selector: ['result'],
            jsonSchema: schema
          }
        ]}
        onChange={onChange}
        syncTitleWithKey
        selectorForKey={codeOutputSelector}
      />
    );
    fireEvent.change(
      screen.getByLabelText(
        i18nText('agentFlow', 'auto.output_variable_name', { value1: 1 })
      ),
      { target: { value: 'renamed' } }
    );
    expect(onChange).toHaveBeenLastCalledWith([
      {
        key: 'renamed',
        title: 'renamed',
        valueType: 'object',
        selector: ['result'],
        jsonSchema: schema
      }
    ]);
  });
});
