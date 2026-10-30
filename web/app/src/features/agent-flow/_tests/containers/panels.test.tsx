import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { AgentFlowEditorShell } from '../../components/editor/AgentFlowEditorShell';
import { renderReactFlowSceneSettled } from '../../../../test/renderers/render-react-flow-scene';
import { createApplicationNodeCatalog } from '../fixtures/application-node-catalog';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  openSelect,
  selectOption,
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
    expect(screen.getByRole('switch', { name: '启用并行' })).toBeChecked();
    expect(screen.queryByText('Policy')).not.toBeInTheDocument();
  });

  test('keeps an invalid structured draft attached to its variable through sibling deletion and rename, then resets it for a new type', async () => {
    const document = containerDocument();
    const outer = document.graph.nodes.find((node) => node.id === 'outer')!;
    outer.config.loop_variables = [
      { key: 'first', value_type: 'object' },
      { key: 'second', value_type: 'object' }
    ];
    outer.bindings.variables = {
      kind: 'named_bindings',
      value: [
        { name: 'first', value: { kind: 'constant', value: { first: true } } },
        { name: 'second', value: { kind: 'constant', value: { second: true } } }
      ]
    };
    let saved: FlowAuthoringDocument = document;
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state(document)}>
        <SelectionSeed nodeId="outer" />
        <DocumentObserver
          onChange={(next) => {
            saved = next;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );
    const draft = await screen.findByRole('textbox', { name: '初始值 second' });
    fireEvent.change(draft, { target: { value: '{"unfinished":' } });
    expect(draft).toHaveValue('{"unfinished":');
    expect(
      saved.graph.nodes.find((node) => node.id === 'outer')!.bindings.variables
    ).toMatchObject({
      value: [
        expect.anything(),
        { name: 'second', value: { value: { second: true } } }
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '删除变量 1' }));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: '初始值 second' })).toBe(draft)
    );
    expect(draft).toHaveValue('{"unfinished":');
    const name = screen.getByRole('textbox', { name: '变量名称 1' });
    await act(async () => {
      name.focus();
    });
    fireEvent.change(name, { target: { value: 'renamed' } });
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: '初始值 renamed' })).toBe(
        draft
      )
    );
    expect(name).toHaveFocus();
    expect(draft).toHaveValue('{"unfinished":');

    // A valid edit commits the real binding; a type change replaces its default
    // instead of leaving the old object draft attached to an array declaration.
    fireEvent.change(draft, { target: { value: '{"kept":2}' } });
    await waitFor(() =>
      expect(
        saved.graph.nodes.find((node) => node.id === 'outer')!.bindings
          .variables
      ).toEqual({
        kind: 'named_bindings',
        value: [
          { name: 'renamed', value: { kind: 'constant', value: { kept: 2 } } }
        ]
      })
    );
    await openSelect('变量类型 1');
    await selectOption('数组');
    await waitFor(() => {
      expect(
        screen.getByRole('textbox', { name: '初始值 renamed' })
      ).toHaveValue('[]');
      expect(
        saved.graph.nodes.find((node) => node.id === 'outer')!.config
          .loop_variables
      ).toEqual([{ key: 'renamed', value_type: 'array' }]);
      expect(
        saved.graph.nodes.find((node) => node.id === 'outer')!.bindings
          .variables
      ).toEqual({
        kind: 'named_bindings',
        value: [{ name: 'renamed', value: { kind: 'constant', value: [] } }]
      });
    });
  });

  test('selects a container through the delegated body click and separate native selection controls', async () => {
    const document = containerDocument();
    document.graph.nodes.find((node) => node.id === 'outer-start')!.alias =
      'Internal start';
    const outerAlias = document.graph.nodes.find(
      (node) => node.id === 'outer'
    )!.alias;
    const startAlias = document.graph.nodes.find(
      (node) => node.id === 'outer-start'
    )!.alias;
    const view = await renderReactFlowSceneSettled(
      <AgentFlowEditorShell
        applicationId="app-containers"
        applicationName="Containers"
        initialState={state(document)}
        nodeCatalog={createApplicationNodeCatalog([])}
      />
    );
    const body = view.container.querySelector<HTMLElement>(
      '[data-container-id="outer"] .agent-flow-container__body'
    )!;
    fireEvent.click(body);
    expect(
      await screen.findByRole('textbox', { name: '节点别名' })
    ).toHaveValue(outerAlias);

    const start = view.container.querySelector<HTMLButtonElement>(
      '.react-flow__node[data-id="outer-start"] .agent-flow-container-start__select'
    )!;
    expect(start.tagName).toBe('BUTTON');
    expect(start).toHaveAttribute('type', 'button');
    expect(start.querySelector('button, [role="button"]')).toBeNull();
    fireEvent.click(start);
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: '节点别名' })).toHaveValue(
        startAlias
      )
    );

    const header = view.container.querySelector<HTMLButtonElement>(
      '[data-container-id="outer"] .agent-flow-container__select'
    )!;
    expect(header.tagName).toBe('BUTTON');
    expect(header.querySelector('button, [role="button"]')).toBeNull();
    fireEvent.click(header);
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: '节点别名' })).toHaveValue(
        outerAlias
      )
    );
  });
});
