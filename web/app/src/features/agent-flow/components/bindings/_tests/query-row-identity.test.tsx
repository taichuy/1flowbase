import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { expect, it } from 'vitest';
import type { DataModelQueryBindingValue } from '@1flowbase/flow-schema';
import { i18nText } from '../../../../../shared/i18n/text';
import { DataModelQueryField } from '../DataModelQueryField';

it('keeps the surviving query input instance when an earlier filter is removed', () => {
  function Query() {
    const [value, setValue] = useState<DataModelQueryBindingValue>({
      filters: ['first', 'second'].map((value) => ({
        field_code: 'title',
        operator: 'eq',
        value: { kind: 'constant', value }
      })),
      sorts: [],
      expand_relations: [],
      page: { kind: 'constant', value: 1 },
      page_size: { kind: 'constant', value: 20 }
    });
    return (
      <DataModelQueryField
        ariaLabel="Query"
        value={value}
        onChange={setValue}
        hasDataModelSelected
        includePagination={false}
        selectorOptions={[]}
        fields={[
          {
            code: 'title',
            title: 'Title',
            valueType: 'string',
            required: false,
            writable: true
          }
        ]}
      />
    );
  }
  render(<Query />);
  const second = screen.getByDisplayValue('second');
  second.focus();
  fireEvent.click(
    screen.getAllByRole('button', {
      name: i18nText('agentFlow', 'auto.delete')
    })[0]
  );
  expect(screen.queryByDisplayValue('first')).not.toBeInTheDocument();
  expect(screen.getByDisplayValue('second')).toBe(second);
  expect(second).toHaveFocus();
});
