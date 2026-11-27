import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { expect, test } from 'vitest';

import {
  JsonSchemaEditorContent,
  type JsonSchemaEditorContentHandle
} from '../JsonSchemaSettingsPanel';

test('a parent echo preserves the editable row and a new session loads its own schema', () => {
  const ref = createRef<JsonSchemaEditorContentHandle>();
  const editor = (resetKey: string, field: string) => (
    <JsonSchemaEditorContent
      ref={ref}
      resetKey={resetKey}
      schema={{
        type: 'object',
        properties: { [field]: { type: 'string' } },
        title: 'Example',
        description: 'Description'
      }}
    />
  );
  const view = render(editor('first', 'original'));
  for (const key of ['type', 'properties', 'title', 'description']) {
    expect(screen.getByDisplayValue(key)).toBeDisabled();
  }
  fireEvent.change(screen.getByDisplayValue('original'), {
    target: { value: 'draft' }
  });
  view.rerender(editor('first', 'echo'));
  expect(screen.getByDisplayValue('draft')).toBeInTheDocument();
  expect(ref.current?.getSchema()).toMatchObject({
    ok: true,
    schema: { properties: { draft: { type: 'string' } } }
  });
  view.rerender(editor('second', 'replacement'));
  expect(screen.queryByDisplayValue('draft')).not.toBeInTheDocument();
  expect(screen.getByDisplayValue('replacement')).toBeInTheDocument();
});
