import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SelectionSeed,
  renderWithProviders,
  setupNodeInspectorTest
} from '../node-inspector/support';
import { containerDocument, state } from './fixtures';

beforeEach(setupNodeInspectorTest);
describe('container configuration panels', () => {
  test('edits the loop deadline and typed initial value through the real store and schema adapter', async () => {
    const document = containerDocument();
    delete document.graph.nodes.find((node) => node.id === 'outer')!.config
      .round_timeout_ms;
    let saved: FlowAuthoringDocument = document;
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state(document)}>
        <SelectionSeed nodeId="outer" />
        <DocumentObserver
          onChange={(document) => {
            saved = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );
    const timeout = await screen.findByRole('spinbutton', {
      name: '每轮期限（毫秒）'
    });
    expect(timeout).toHaveDisplayValue('');
    fireEvent.change(timeout, { target: { value: '2500' } });
    fireEvent.change(
      await screen.findByRole('spinbutton', { name: '初始值 count' }),
      { target: { value: '3' } }
    );
    fireEvent.change(screen.getByRole('textbox', { name: '变量名称 1' }), {
      target: { value: 'total' }
    });
    await waitFor(() => {
      const loop = saved.graph.nodes.find((node) => node.id === 'outer')!;
      expect(loop.config.round_timeout_ms).toBe(2500);
      expect(loop.config.loop_variables).toEqual([
        { key: 'total', value_type: 'number' }
      ]);
      expect(loop.bindings.variables).toEqual({
        kind: 'named_bindings',
        value: [{ name: 'total', value: { kind: 'constant', value: 3 } }]
      });
    });
  });

  test('saves parallelism and flattening on iteration without exposing the ordinary node policy', async () => {
    const document = containerDocument();
    let saved: FlowAuthoringDocument = document;
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state(document)}>
        <SelectionSeed nodeId="inner" />
        <DocumentObserver
          onChange={(document) => {
            saved = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );
    fireEvent.change(
      await screen.findByRole('spinbutton', { name: '并行数量' }),
      { target: { value: '7' } }
    );
    fireEvent.click(screen.getByRole('switch', { name: '展平一层输出' }));
    await waitFor(() =>
      expect(
        saved.graph.nodes.find((node) => node.id === 'inner')!.config
      ).toMatchObject({
        parallelism: 7,
        flatten_output: true,
        error_policy: 'fail',
        round_timeout_ms: 500
      })
    );
    expect(screen.getByRole('switch', { name: '启用并行' })).toHaveAttribute(
      'aria-checked',
      'true'
    );
    expect(screen.queryByText('Policy')).not.toBeInTheDocument();
  });
});
