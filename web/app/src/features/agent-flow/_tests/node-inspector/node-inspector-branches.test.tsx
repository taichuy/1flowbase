import { readFileSync } from 'node:fs';

import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';

import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SelectionSeed,
  createInitialStateWithIfElseNode,
  createInitialStateWithLoopNode,
  openSelect,
  renderWithProviders,
  selectOption,
  setupNodeInspectorTest
} from './support';

beforeEach(setupNodeInspectorTest);

describe('NodeInspector branches', () => {
  test('renders loop number fields in compact inline rows while keeping condition groups stacked', async () => {
    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithLoopNode()}
      >
        <SelectionSeed nodeId="node-loop" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(screen.queryByText('Inputs')).not.toBeInTheDocument();
    expect(screen.getByText('Policy')).toBeInTheDocument();
    const toolbar = screen.getByTestId('condition-group-toolbar');

    expect(
      within(toolbar).getByRole('combobox', { name: /入口条件-operator|Entry conditions-operator/ })
    ).toBeInTheDocument();
    expect(
      within(toolbar).getByRole('button', { name: /新增条件$/ })
    ).toBeInTheDocument();
    expect(screen.getByTestId('inspector-field-config.max_rounds')).toHaveClass(
      'agent-flow-editor__inspector-field--inline'
    );
    expect(
      screen.getByTestId('inspector-field-bindings.entry_condition')
    ).not.toHaveClass('agent-flow-editor__inspector-field--inline');
  });

  test('keeps If / Else condition rule controls inside narrow inspector bounds', async () => {
    const inspectorStyles = readFileSync(
      'src/features/agent-flow/components/editor/styles/inspector.css',
      'utf8'
    );

    expect(inspectorStyles).toContain(
      '.agent-flow-condition-group__rule {\n  display: flex;\n  flex-wrap: wrap;'
    );
    expect(inspectorStyles).toContain(
      '.agent-flow-condition-group__rule > :not(.agent-flow-binding-row__delete) {\n  flex: 1 1 136px;\n  min-width: 0;'
    );
    expect(inspectorStyles).toContain(
      '.agent-flow-condition-group__rule > .agent-flow-binding-row__delete {\n  flex: 0 0 28px;'
    );
  });

  test('edits If / Else branches as first-class branch handles', async () => {
    const initialState = createInitialStateWithIfElseNode();
    let latestDocument = initialState.draft.document;

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-if-else" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const branchField = await screen.findByTestId(
      'inspector-field-bindings.branches'
    );

    expect(screen.getByTestId('if-else-branch-if')).toBeInTheDocument();
    expect(screen.getByTestId('if-else-branch-else')).toBeInTheDocument();
    expect(
      within(screen.getByTestId('if-else-branch-else')).getByLabelText(
        'Else 分支名称'
      )
    ).toBeDisabled();
    expect(
      screen.queryByRole('button', { name: '删除 Else 分支' })
    ).not.toBeInTheDocument();

    const addElseIfButton = within(branchField).getByTestId(
      'if-else-add-else-if'
    );
    expect(
      addElseIfButton.compareDocumentPosition(
        screen.getByTestId('if-else-branch-else')
      ) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    fireEvent.click(
      addElseIfButton
    );

    const elseIfBranch = await screen.findByTestId(
      'if-else-branch-else-if-1'
    );

    expect(
      within(elseIfBranch).getByLabelText('Else If 1 分支名称')
    ).toHaveValue('Else If 1');
    expect(
      within(elseIfBranch).getByRole('button', {
        name: '删除 Else If 1 分支'
      })
    ).toBeInTheDocument();

    fireEvent.click(
      within(screen.getByTestId('if-else-branch-if')).getByRole('button', {
        name: /新增条件组/
      })
    );

    expect(
      within(screen.getByTestId('if-else-branch-if')).getAllByTestId(
        'condition-group-toolbar'
      )
    ).toHaveLength(2);

    await waitFor(() => {
      const ifElseNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-if-else'
      );
      const branchBinding = ifElseNode?.bindings.branches;

      if (!branchBinding || branchBinding.kind !== 'if_else_branches') {
        throw new Error('expected If / Else branch binding');
      }

      expect(branchBinding.value.branches).toEqual([
        expect.objectContaining({
          kind: 'if',
          sourceHandle: 'if',
          condition: {
            operator: 'and',
            conditions: [{ operator: 'and', conditions: [] }]
          }
        }),
        expect.objectContaining({
          id: 'else-if-1',
          kind: 'else_if',
          title: 'Else If 1',
          sourceHandle: 'else-if-1',
          condition: { operator: 'and', conditions: [] }
        }),
        expect.objectContaining({
          kind: 'else',
          sourceHandle: 'else'
        })
      ]);
    });
  });

  test('supports empty comparator in If / Else condition rules without right value', async () => {
    const initialState = createInitialStateWithIfElseNode();
    let latestDocument = initialState.draft.document;

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-if-else" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const ifBranch = await screen.findByTestId('if-else-branch-if');

    fireEvent.click(
      within(ifBranch).getByRole('button', { name: /新增条件$/ })
    );

    await openSelect('分支-if-0-comparator');
    expect(await screen.findByTitle('为空')).toBeInTheDocument();
    await selectOption('为空');

    expect(
      screen.queryByRole('combobox', { name: '分支-if-0-right-kind' })
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText('分支-if-0-right')).not.toBeInTheDocument();

    await waitFor(() => {
      const ifElseNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-if-else'
      );
      const branchBinding = ifElseNode?.bindings.branches;

      if (!branchBinding || branchBinding.kind !== 'if_else_branches') {
        throw new Error('expected If / Else branch binding');
      }

      const conditions = branchBinding.value.branches[0]?.condition?.conditions;

      expect(conditions).toEqual([
        expect.objectContaining({
          comparator: 'empty'
        })
      ]);
      expect(conditions?.[0]).not.toHaveProperty('right');
    });
  });

  test('stores typed fixed values in If / Else condition rules', async () => {
    const initialState = createInitialStateWithIfElseNode();
    const startNode = initialState.draft.document.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    if (!startNode) {
      throw new Error('expected Start node');
    }

    startNode.config.input_fields = [
      {
        key: 'amount',
        label: 'Amount',
        inputType: 'number',
        required: false
      }
    ];
    initialState.draft.document.graph.edges.push({
      id: 'edge-start-if-else',
      source: 'node-start',
      target: 'node-if-else',
      sourceHandle: null,
      targetHandle: null,
      containerId: null,
      points: []
    });
    let latestDocument = initialState.draft.document;

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-if-else" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const ifBranch = await screen.findByTestId('if-else-branch-if');

    fireEvent.click(
      within(ifBranch).getByRole('button', { name: /新增条件$/ })
    );

    await openSelect('分支-if-0-left');
    await selectOption('Start');
    await selectOption('amount');
    await openSelect('分支-if-0-comparator');
    await selectOption('等于');
    const rightInput = await screen.findByLabelText('分支-if-0-right');

    rightInput.focus();
    fireEvent.change(rightInput, {
      target: { value: '5' }
    });

    await waitFor(() => {
      expect(screen.getByLabelText('分支-if-0-right')).toHaveFocus();
    });

    await waitFor(() => {
      const ifElseNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-if-else'
      );
      const branchBinding = ifElseNode?.bindings.branches;

      if (!branchBinding || branchBinding.kind !== 'if_else_branches') {
        throw new Error('expected If / Else branch binding');
      }

      expect(
        branchBinding.value.branches[0]?.condition?.conditions[0]
      ).toMatchObject({
        left: ['node-start', 'amount'],
        comparator: 'equals',
        right: { kind: 'constant', value: 5 }
      });
    });
  });

  test('renders structured fixed values in If / Else condition rules as JSON text', async () => {
    const initialState = createInitialStateWithIfElseNode();
    const ifElseNode = initialState.draft.document.graph.nodes.find(
      (node) => node.id === 'node-if-else'
    );

    if (!ifElseNode) {
      throw new Error('expected If / Else node');
    }

    ifElseNode.bindings.branches = {
      kind: 'if_else_branches',
      value: {
        branches: [
          {
            id: 'if',
            kind: 'if',
            title: 'If',
            sourceHandle: 'if',
            condition: {
              operator: 'and',
              conditions: [
                {
                  kind: 'rule',
                  left: ['node-start', 'query'],
                  comparator: 'equals',
                  right: {
                    kind: 'constant',
                    value: { tier: 'gold' }
                  }
                }
              ]
            }
          },
          {
            id: 'else',
            kind: 'else',
            title: 'Else',
            sourceHandle: 'else'
          }
        ]
      }
    };

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-if-else" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByLabelText('分支-if-0-right')).toHaveValue(
      '{"tier":"gold"}'
    );
  });
});
