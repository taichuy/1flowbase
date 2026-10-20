import { useState } from 'react';
import { act, fireEvent } from '@testing-library/react';
import { beforeEach } from 'vitest';

import { NamedBindingsField } from '../../components/bindings/NamedBindingsField';

import * as nodeSchemaAdapterApi from '../../schema/node-schema-adapter';
import * as nodeSchemaRegistry from '../../schema/node-schema-registry';

import { setupNodeInspectorTest } from './support';

beforeEach(setupNodeInspectorTest);

function NamedBindingsFocusHarness() {
  const [value, setValue] = useState<
    Array<{ name: string; selector: string[] }>
  >([{ name: 'arg1', selector: [] }]);

  return (
    <NamedBindingsField
      ariaLabel="bindings"
      value={value}
      options={[]}
      onChange={setValue}
    />
  );
}

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => {
    result = event();
  });
  return result;
}

const domEvents = {
  change: (...args: Parameters<typeof fireEvent.change>) =>
    dispatch(() => fireEvent.change(...args)),
  click: (...args: Parameters<typeof fireEvent.click>) =>
    dispatch(() => fireEvent.click(...args)),
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) =>
    dispatch(() => fireEvent.mouseDown(...args)),
  mouseMove: (...args: Parameters<typeof fireEvent.mouseMove>) =>
    dispatch(() => fireEvent.mouseMove(...args)),
  mouseUp: (...args: Parameters<typeof fireEvent.mouseUp>) =>
    dispatch(() => fireEvent.mouseUp(...args))
};

export {
  NamedBindingsFocusHarness,
  dispatch,
  domEvents,
  nodeSchemaAdapterApi,
  nodeSchemaRegistry
};
