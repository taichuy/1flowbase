import { Profiler, useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
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

function mountValueMode(valueType: string, initialValue: unknown) {
  const firstCommits = new Map<string, {
    jsonOpen: boolean;
    jsonContent: string | null;
    objectRows: number;
    arrayRows: number;
  }>();
  let currentType = valueType;
  let value = initialValue;
  let action = 'mount';
  const onValueErrorChange = vi.fn();
  const onChange = vi.fn((nextValue: unknown) => {
    value = nextValue;
    view.rerender(scene());
  });
  const scene = () => (
    <Profiler id="environment-value-mode-commits" onRender={() => {
      if (firstCommits.has(action)) return;
      const root = screen.queryByTestId('environment-value-mode-owner');
      if (!root) return;
      const json = root.querySelector<HTMLTextAreaElement>(
        '.agent-flow-editor__env-value-json-editor textarea'
      );
      firstCommits.set(action, {
        jsonOpen: json !== null,
        jsonContent: json?.value ?? null,
        objectRows: root.querySelectorAll('.agent-flow-editor__env-object-row').length,
        arrayRows: root.querySelectorAll('.agent-flow-editor__env-array-row').length
      });
    }}>
      <div data-testid="environment-value-mode-owner">
        <EnvironmentVariableValueEditor
          valueType={currentType}
          value={value}
          onChange={onChange}
          onValueErrorChange={onValueErrorChange}
        />
      </div>
    </Profiler>
  );
  const view = render(scene());
  return {
    onChange,
    onValueErrorChange,
    replace(nextType: string, nextValue: unknown, nextAction: string) {
      currentType = nextType;
      value = nextValue;
      action = nextAction;
      view.rerender(scene());
    },
    firstCommit(expectedAction: string) {
      expect(firstCommits.has(expectedAction)).toBe(true);
      return firstCommits.get(expectedAction);
    }
  };
}

test.each([
  {
    from: 'object', value: { previous: 'old-object' },
    to: 'array[string]', nextValue: ['new-array'],
    objectRows: 0, arrayRows: 1, nextText: 'new-array'
  },
  {
    from: 'array[string]', value: ['old-array'],
    to: 'object', nextValue: { next: 'new-object' },
    objectRows: 1, arrayRows: 0, nextText: 'new-object'
  }
])('a $from to $to type change leaves JSON mode on the first actual committed UI', ({
  from, value, to, nextValue, objectRows, arrayRows, nextText
}) => {
  const owner = mountValueMode(from, value);
  fireEvent.click(screen.getByRole('button', { name: /JSON$/u }));
  fireEvent.change(screen.getByRole('textbox'), {
    target: { value: '{"unfinished":' }
  });
  expect(owner.onChange).not.toHaveBeenCalled();

  owner.replace(to, nextValue, 'changed-type');

  expect(owner.firstCommit('changed-type')).toEqual({
    jsonOpen: false, jsonContent: null, objectRows, arrayRows
  });
  expect(screen.getByDisplayValue(nextText)).toBeInTheDocument();
  expect(owner.onChange).not.toHaveBeenCalled();
});

test('same-type rerenders preserve an invalid JSON buffer and Apply keeps validation and parsed-value behavior', () => {
  const source = { original: 'source' };
  const owner = mountValueMode('object', source);
  fireEvent.click(screen.getByRole('button', { name: /JSON$/u }));
  const editor = screen.getByRole('textbox');
  fireEvent.change(editor, { target: { value: '{"edited":' } });

  owner.replace('object', source, 'same-type');

  expect(owner.firstCommit('same-type')).toMatchObject({
    jsonOpen: true, jsonContent: '{"edited":'
  });
  expect(screen.getByRole('textbox')).toBe(editor);
  fireEvent.click(screen.getByRole('button', {
    name: i18nText('agentFlow', 'auto.apply_json')
  }));
  expect(owner.onChange).not.toHaveBeenCalled();
  expect(owner.onValueErrorChange).toHaveBeenLastCalledWith(
    i18nText('agentFlow', 'auto.json_format_invalid')
  );
  expect(editor).toHaveValue('{"edited":');

  fireEvent.change(editor, { target: { value: '{"edited":"applied"}' } });
  fireEvent.click(screen.getByRole('button', {
    name: i18nText('agentFlow', 'auto.apply_json')
  }));

  expect(owner.onChange).toHaveBeenLastCalledWith({ edited: 'applied' });
  expect(screen.getByDisplayValue('edited')).toBeInTheDocument();
  expect(screen.getByDisplayValue('applied')).toBeInTheDocument();
  expect(screen.queryByText(i18nText('agentFlow', 'auto.apply_json'))).not.toBeInTheDocument();
});
