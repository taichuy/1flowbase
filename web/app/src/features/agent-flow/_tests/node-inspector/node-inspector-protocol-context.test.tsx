import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';

import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { appI18n } from '../../../../shared/i18n/app-i18n';
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
  test('AC-001 enables Start protocol context for a document without the field', async () => {
    const state = createInitialStateWithProtocolContextCodeNode();
    delete getLlmNodeConfig(state.draft.document).protocol_context;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const field = await screen.findByTestId(
      'inspector-field-config.protocol_context'
    );
    expect(
      within(field).getByRole('switch', { name: '协议上下文' })
    ).toBeChecked();
    expect(
      within(field).getByRole('combobox', { name: '协议上下文变量' })
    ).toBeEnabled();
    expect(
      within(field).getByLabelText(
        '将选中的协议上下文透传给当前 LLM 节点调用的模型服务。'
      )
    ).toBeInTheDocument();
    expect(field).toHaveTextContent('protocol_context');
    expect(getLlmNodeConfig(state.draft.document)).not.toHaveProperty(
      'protocol_context'
    );
  });

  test('shows the protocol passthrough help in English', async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'en_US');
    await appI18n.changeLanguage('en_US');
    const state = createInitialStateWithProtocolContextCodeNode();

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const field = await screen.findByTestId(
      'inspector-field-config.protocol_context'
    );
    expect(
      within(field).getByLabelText(
        'Passes the selected protocol context through to the model service called by this LLM node.'
      )
    ).toBeInTheDocument();
  });

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
    expect(field).toHaveTextContent('protocol_context');
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
        value: ['node-start', 'protocol_context']
      });
    });

    fireEvent.mouseDown(protocolContextSelector);
    fireEvent.keyDown(protocolContextSelector, { key: 'ArrowDown' });
    for (const title of ['Protocol Builder', 'result', 'protocol_context']) {
      const matches = await screen.findAllByTitle(title);
      fireEvent.click(matches[matches.length - 1]);
    }

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument).protocol_context).toEqual({
        kind: 'selector',
        value: ['node-code', 'result', 'protocol_context']
      });
    });
    expect(screen.queryByTitle('headers')).not.toBeInTheDocument();
  });
});
