import { fireEvent, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';

import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SelectionSeed,
  createInitialStateWithProtocolContextCodeNode,
  getLlmNodeConfig,
  renderWithProviders,
  setupNodeInspectorTest
} from './support';

beforeEach(setupNodeInspectorTest);

describe('NodeInspector protocol context', () => {
  test('WP-D1D drives enablement and whole-object selection from one nullable reference', async () => {
    const state = createInitialStateWithProtocolContextCodeNode();
    let latestDocument = state.draft.document;

    const view = renderWithProviders(
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

    const field = await view.findByTestId(
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

    fireEvent.mouseDown(protocolContextSelector);
    fireEvent.keyDown(protocolContextSelector, { key: 'ArrowDown' });
    for (const title of ['Protocol Builder', 'result', 'protocol_context']) {
      const matches = await view.findAllByTitle(title);
      fireEvent.click(matches[matches.length - 1]);
    }

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument).protocol_context).toEqual({
        kind: 'selector',
        value: ['node-code', 'result', 'protocol_context']
      });
    });
    expect(view.queryByTitle('headers')).not.toBeInTheDocument();
  });
});
