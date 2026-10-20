import { readFileSync } from 'node:fs';

import { useState } from 'react';
import {
  act,
  fireEvent,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { modelProviderOptionsContract } from '../../../../test/model-provider-contract-fixtures';
import { NamedBindingsField } from '../../components/bindings/NamedBindingsField';
import { TemplatedNamedBindingsField } from '../../components/bindings/TemplatedNamedBindingsField';
import { NodeDetailPanel } from '../../components/detail/NodeDetailPanel';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { NodeInspector } from '../../components/inspector/NodeInspector';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import { createNodeDocument } from '../../lib/document/node-factory';
import * as nodeSchemaAdapterApi from '../../schema/node-schema-adapter';
import * as nodeSchemaRegistry from '../../schema/node-schema-registry';
import { validateDocument } from '../../lib/validate-document';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  FocusIssueSeed,
  SelectionSeed,
  SLOW_NODE_INSPECTOR_TEST_TIMEOUT,
  createInitialState,
  createInitialStateWithHttpRequestNode,
  createInitialStateWithCodeNode,
  createAgentFlowNodeSchemaAdapterSpy,
  fetchModelProviderOptionsSpy,
  getLlmNodeConfig,
  primaryProviderFirstModel,
  primaryProviderOption,
  renderWithProviders,
  resolveAgentFlowNodeSchemaSpy,
  setupNodeInspectorTest
} from './support';

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
  AgentFlowEditorStoreProvider,
  DocumentObserver,
  FocusIssueSeed,
  NamedBindingsFocusHarness,
  NodeConfigTab,
  NodeDetailPanel,
  NodeInspector,
  SLOW_NODE_INSPECTOR_TEST_TIMEOUT,
  SelectionSeed,
  TemplatedNamedBindingsField,
  createAgentFlowNodeSchemaAdapterSpy,
  createDefaultAgentFlowDocument,
  createEdgeDocument,
  createInitialState,
  createInitialStateWithCodeNode,
  createInitialStateWithHttpRequestNode,
  createNodeDocument,
  dispatch,
  domEvents,
  expect,
  fetchModelProviderOptionsSpy,
  getLlmNodeConfig,
  modelProviderOptionsContract,
  nodeSchemaAdapterApi,
  nodeSchemaRegistry,
  primaryProviderFirstModel,
  primaryProviderOption,
  readFileSync,
  renderWithProviders,
  resolveAgentFlowNodeSchemaSpy,
  screen,
  test,
  validateDocument,
  vi,
  waitFor,
  within
};
