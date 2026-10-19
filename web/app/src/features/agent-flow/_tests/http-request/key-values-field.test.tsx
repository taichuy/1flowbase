import { act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { useState } from 'react';
import { afterAll, beforeAll, describe, expect, test } from 'vitest';

import type { FlowBinding } from '@1flowbase/flow-schema';
import { HttpRequestBodyField } from '../../components/detail/fields/http-request/HttpRequestBodyField';
import { HttpRequestKeyValuesField } from '../../components/detail/fields/http-request/HttpRequestKeyValuesField';
import type { FlowSelectorOption } from '../../lib/selector-options';

const startQueryOption: FlowSelectorOption = {
  nodeId: 'node-start',
  nodeLabel: 'Start',
  outputKey: 'query',
  outputLabel: 'query',
  valueType: 'string',
  value: ['node-start', 'query'],
  displayLabel: 'Start/query'
};

const textPrototype = Text.prototype as unknown as {
  getBoundingClientRect?: () => DOMRect;
};
const originalTextGetBoundingClientRect = textPrototype.getBoundingClientRect;
const rangePrototype = Range.prototype as unknown as {
  getBoundingClientRect?: () => DOMRect;
};
const originalRangeGetBoundingClientRect = rangePrototype.getBoundingClientRect;

beforeAll(() => {
  textPrototype.getBoundingClientRect = () => new DOMRect();
  rangePrototype.getBoundingClientRect = () => new DOMRect();
});

afterAll(() => {
  if (originalTextGetBoundingClientRect) {
    textPrototype.getBoundingClientRect = originalTextGetBoundingClientRect;
  } else {
    delete textPrototype.getBoundingClientRect;
  }

  if (originalRangeGetBoundingClientRect) {
    rangePrototype.getBoundingClientRect = originalRangeGetBoundingClientRect;
  } else {
    delete rangePrototype.getBoundingClientRect;
  }
});

function HttpRequestKeyValuesHarness() {
  const [value, setValue] = useState<FlowBinding>({
    kind: 'named_bindings',
    value: [
      {
        name: 'p',
        value: { kind: 'templated_text', value: '1' }
      }
    ]
  });

  return (
    <>
      <HttpRequestKeyValuesField
        ariaLabel="Request Params"
        options={[startQueryOption]}
        value={value}
        onChange={setValue}
      />
      <output data-testid="params-value">{JSON.stringify(value)}</output>
    </>
  );
}

function HttpRequestFormDataHarness() {
  const [formDataValue, setFormDataValue] = useState<FlowBinding>({
    kind: 'named_bindings',
    value: [
      {
        name: 'p',
        valueType: 'text',
        value: { kind: 'templated_text', value: '1' }
      }
    ]
  });

  return (
    <>
      <HttpRequestBodyField
        binaryValue={{ kind: 'selector', value: [] }}
        bodyType="form-data"
        bodyValue={{ kind: 'templated_text', value: '' }}
        formDataValue={formDataValue}
        options={[startQueryOption]}
        urlencodedValue={{ kind: 'named_bindings', value: [] }}
        onBinaryChange={() => undefined}
        onBodyChange={() => undefined}
        onBodyTypeChange={() => undefined}
        onFormDataChange={setFormDataValue}
        onUrlencodedChange={() => undefined}
      />
      <output data-testid="form-data-value">
        {JSON.stringify(formDataValue)}
      </output>
    </>
  );
}

describe('HttpRequestKeyValuesField', () => {
  test('keeps focus on the key editor after the key name changes', async () => {
    await renderSettled(<HttpRequestKeyValuesHarness />);

    const keyEditor = screen.getByLabelText('Request Params-0-key');

    await dispatch(() => {
      keyEditor.focus();
      // The append assertion requires the user's caret after the existing key.
      const range = document.createRange();
      range.selectNodeContents(keyEditor);
      range.collapse(false);
      const selection = window.getSelection();
      if (!selection) throw new Error('expected editor DOM selection');
      selection.removeAllRanges();
      selection.addRange(range);
      document.dispatchEvent(new Event('selectionchange'));
    });
    expect(keyEditor).toHaveFocus();

    const insertKeyVariableButton = screen.getAllByRole('button', {
      name: '插入变量'
    })[0];

    if (!insertKeyVariableButton) {
      throw new Error('expected key variable insert button');
    }

    await domEvents.click(insertKeyVariableButton);
    await domEvents.click(await screen.findByRole('option', { name: 'Start/query' }));

    await waitFor(() => {
      expect(screen.getByTestId('params-value')).toHaveTextContent(
        '"name":"p{{node-start.query}}"'
      );
      expect(keyEditor).toHaveFocus();
    });
    expect(keyEditor).toHaveFocus();
  });

  test('keeps focus on the form-data key input after the key name changes', async () => {
    await renderSettled(<HttpRequestFormDataHarness />);

    const keyInput = screen.getByDisplayValue('p');

    await dispatch(() => keyInput.focus());
    expect(keyInput).toHaveFocus();

    await domEvents.change(keyInput, {
      target: { value: 'pz' }
    });

    await waitFor(() => {
      expect(screen.getByTestId('form-data-value')).toHaveTextContent(
        '"name":"pz"'
      );
    });
    expect(keyInput).toHaveFocus();
  });
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => { result = event(); });
  return result;
}

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => { view = render(...args); });
  return view;
}

const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) => dispatch(() => fireEvent.click(...args)),
  change: (...args: Parameters<typeof fireEvent.change>) => dispatch(() => fireEvent.change(...args))
};
