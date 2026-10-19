import { fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { expect, test } from 'vitest';
import { ReturnFieldRows } from '../ReturnFieldRows';

test('keeps the surviving return field input mounted after deletion and ordinary edits', () => {
  function Harness() {
    const [fields, setFields] = useState(['first', 'second']);
    return <ReturnFieldRows fields={fields} fieldLabel="Fields" onChange={setFields} />;
  }
  const { container } = render(<Harness />);
  const survivor = screen.getByLabelText('Fields 2');
  fireEvent.change(survivor, { target: { value: 'edited' } });
  expect(screen.getByLabelText('Fields 2')).toBe(survivor);
  const first = container.querySelector('.mcp-management__return-field-row')!;
  fireEvent.click(within(first as HTMLElement).getByRole('button'));
  expect(screen.getByLabelText('Fields 1')).toBe(survivor);
  expect(survivor).toHaveValue('edited');
});
