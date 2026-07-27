import { beforeEach, describe, expect, test } from 'vitest';

import {
  AgentFlowEditorStoreProvider,
  DocumentObserver,
  NodeConfigTab,
  SelectionSeed,
  createInitialStateWithProtocolContextCodeNode,
  fireEvent,
  getLlmNodeConfig,
  openSelect,
  renderWithProviders,
  screen,
  selectOption,
  setupNodeInspectorTest,
  waitFor,
  within
} from './support';

beforeEach(setupNodeInspectorTest);

describe('NodeInspector protocol context', () => {
  test('WP-D1D drives enablement and whole-object selection from one nullable reference', async () => {
    const state = createInitialStateWithProtocolContextCodeNode();
    let latestDocument = state.draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const field = await screen.findByTestId(
      'inspector-field-config.protocol_context'
    );
    const protocolContextSwitch = within(field).getByRole('switch', {
      name: '协议上下文'
    });
    const protocolContextSelector = within(field).getByRole('combobox', {
      name: '协议上下文变量'
    });

    expect(protocolContextSwitch).toBeChecked();
    expect(protocolContextSelector).toBeEnabled();
    expect(field).toHaveTextContent('sys.protocol_context');
    expect(getLlmNodeConfig(latestDocument)).not.toHaveProperty(
      'protocol_context_enabled'
    );

    fireEvent.click(protocolContextSwitch);

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument).protocol_context).toBeNull();
      expect(protocolContextSelector).toBeDisabled();
    });

    fireEvent.click(protocolContextSwitch);

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument).protocol_context).toEqual({
        kind: 'selector',
        value: ['sys', 'protocol_context']
      });
    });

    await openSelect('协议上下文变量');
    await selectOption('Protocol Builder');
    await selectOption('result');
    await selectOption('protocol_context');

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument).protocol_context).toEqual({
        kind: 'selector',
        value: ['node-code', 'result', 'protocol_context']
      });
    });
    expect(screen.queryByTitle('headers')).not.toBeInTheDocument();
  });
});
