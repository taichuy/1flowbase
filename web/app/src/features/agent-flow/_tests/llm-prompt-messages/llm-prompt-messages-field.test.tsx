import { fireEvent, render, screen, within } from '@testing-library/react';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { useEffect, type ReactNode } from 'react';
import { describe, expect, test } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  type LlmPromptMessage
} from '@1flowbase/flow-schema';

import { AppProviders } from '../../../../app/AppProviders';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { normalizePromptMessagesBinding } from '../../lib/llm-prompt-messages';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';

const PROMPT_MESSAGES_REORDER_TEST_TIMEOUT = 30_000;

function createInitialState(
  document = createDefaultAgentFlowDocument({ flowId: 'flow-1' })
) {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-28T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

function renderWithProviders(ui: ReactNode) {
  return render(<AppProviders>{ui}</AppProviders>);
}

function DocumentObserver({
  onChange
}: {
  onChange: (
    document: ReturnType<typeof createDefaultAgentFlowDocument>
  ) => void;
}) {
  const document = useAgentFlowEditorStore(selectWorkingDocument);

  useEffect(() => {
    onChange(document);
  }, [document, onChange]);

  return null;
}

function SelectionSeed({ nodeId }: { nodeId: string }) {
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);

  useEffect(() => {
    setSelection({
      selectedNodeId: nodeId,
      selectedNodeIds: [nodeId]
    });
  }, [nodeId, setSelection]);

  return null;
}

function llmNodeFrom(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
) {
  const node = document.graph.nodes.find((entry) => entry.id === 'node-llm');

  if (!node) {
    throw new Error('expected default LLM node');
  }

  return node;
}

function promptMessagesFrom(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
): LlmPromptMessage[] {
  const promptMessages = llmNodeFrom(document).bindings.prompt_messages;

  if (promptMessages?.kind !== 'prompt_messages') {
    throw new Error('expected prompt_messages binding');
  }

  return promptMessages.value;
}

describe('LLM prompt messages field', () => {
  test('uses dark role trigger text to match the fixed system label', async () => {
    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/agent-flow/components/editor/styles/inspector.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain(
      '.agent-flow-llm-prompt-messages__role-trigger {\n' +
        '  display: inline-flex;\n' +
        '  align-items: center;\n' +
        '  gap: 4px;\n' +
        '  height: 28px;\n' +
        '  padding: 0;\n' +
        '  border: 0;\n' +
        '  border-radius: 0;\n' +
        '  background: transparent;\n' +
        '  color: #101828;'
    );
    expect(cssSource).toContain(
      '.agent-flow-llm-prompt-messages__role-icon {\n' + '  color: #101828;'
    );
  });

  test('does not read removed prompt bindings in the field renderer', async () => {
    const rendererSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/agent-flow/schema/agent-flow-field-renderers.tsx'
      ),
      'utf8'
    );

    expect(rendererSource).not.toContain('bindings.system_prompt');
    expect(rendererSource).not.toContain('bindings.user_prompt');
  });

  test('keeps system first and only lets dynamic messages switch between user and assistant', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findAllByText('上下文')).not.toHaveLength(0);
    expect(screen.getByLabelText('SYSTEM 消息内容')).toBeInTheDocument();
    expect(screen.getByLabelText('USER 消息内容')).toBeInTheDocument();
    expect(promptMessagesFrom(latestDocument)[1]?.content.value).toBe(
      '{{node-start.query}}'
    );

    const systemRow = screen.getByTestId('llm-prompt-message-row-system-1');
    expect(systemRow).toHaveClass('agent-flow-llm-prompt-messages__row--fixed');
    expect(systemRow).not.toHaveClass(
      'agent-flow-llm-prompt-messages__row--draggable'
    );
    expect(within(systemRow).getByText('SYSTEM')).toBeInTheDocument();
    expect(
      within(systemRow).queryByRole('button', { name: /删除/ })
    ).not.toBeInTheDocument();
    expect(
      within(systemRow).queryByRole('button', { name: /拖拽排序/ })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '新增消息' }));
    expect(screen.getAllByLabelText('USER 消息内容')).toHaveLength(2);

    const rows = screen.getAllByTestId(/llm-prompt-message-row-/);
    const addedRow = rows.at(-1);

    if (!addedRow) {
      throw new Error('expected appended prompt message row');
    }

    expect(addedRow).toHaveClass(
      'agent-flow-llm-prompt-messages__row--draggable'
    );
    const addedRoleSelect = within(addedRow).getByRole('button', {
      name: /消息角色/
    });
    expect(addedRoleSelect).toHaveClass(
      'agent-flow-llm-prompt-messages__role-trigger'
    );
    expect(
      within(addedRow).getByRole('button', { name: /拖拽排序/ })
    ).toBeInTheDocument();
    fireEvent.click(addedRoleSelect);

    expect(
      screen.queryByRole('menuitem', { name: 'SYSTEM' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'USER' })).toBeInTheDocument();
    const assistantOption = screen.getByRole('menuitem', {
      name: 'ASSISTANT'
    });
    expect(assistantOption).toBeInTheDocument();

    fireEvent.click(assistantOption);

    fireEvent.dragStart(
      within(addedRow).getByRole('button', { name: /拖拽排序/ })
    );
    fireEvent.dragOver(rows[1]);
    fireEvent.drop(rows[1]);

    const latestRows = screen.getAllByTestId(/llm-prompt-message-row-/);
    fireEvent.click(
      within(latestRows[1]).getByRole('button', { name: /删除/ })
    );

    expect(
      promptMessagesFrom(latestDocument).map((message) => message.role)
    ).toEqual(['system', 'user']);
  }, PROMPT_MESSAGES_REORDER_TEST_TIMEOUT);

  test('renders dynamic messages as an addable group after the fixed system prompt', async () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm'
        ? {
            ...node,
            bindings: {
              prompt_messages: {
                kind: 'prompt_messages',
                value: [
                  {
                    id: 'system-only',
                    role: 'system',
                    content: { kind: 'templated_text', value: '' }
                  }
                ]
              }
            }
          }
        : node
    );
    let latestDocument = document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState(document)}>
        <SelectionSeed nodeId="node-llm" />
        <DocumentObserver
          onChange={(nextDocument) => {
            latestDocument = nextDocument;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByLabelText('SYSTEM 消息内容')).toBeInTheDocument();
    expect(screen.queryByLabelText('USER 消息内容')).not.toBeInTheDocument();
    const dynamicList = screen.getByTestId('llm-prompt-message-dynamic-list');
    expect(dynamicList).toBeInTheDocument();
    expect(
      within(dynamicList).getByRole('button', { name: '新增消息' })
    ).toBeInTheDocument();
    expect(within(dynamicList).queryByText('暂无消息')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '新增消息' }));
    fireEvent.click(screen.getByRole('button', { name: '新增消息' }));

    expect(screen.getAllByLabelText('USER 消息内容')).toHaveLength(2);
    expect(
      promptMessagesFrom(latestDocument).map((message) => message.role)
    ).toEqual(['system', 'user', 'user']);
  });

  test('ignores removed system and user prompt bindings without prompt_messages', () => {
    const messages = normalizePromptMessagesBinding(undefined);

    expect(messages).toEqual([
      expect.objectContaining({
        role: 'system',
        content: { kind: 'templated_text', value: '' }
      })
    ]);
  });

  test('collapses local system prompt when integration context is enabled', async () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm'
        ? {
            ...node,
            config: {
              ...node.config,
              context_policy: {
                integration_context: 'enabled'
              }
            },
            bindings: {
              prompt_messages: {
                kind: 'prompt_messages',
                value: [
                  {
                    id: 'system-1',
                    role: 'system',
                    content: {
                      kind: 'templated_text',
                      value: 'Keep answers short.'
                    }
                  },
                  {
                    id: 'user-1',
                    role: 'user',
                    content: {
                      kind: 'templated_text',
                      value: '{{node-start.query}}'
                    }
                  }
                ]
              }
            }
          }
        : node
    );
    let latestDocument = document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState(document)}>
        <SelectionSeed nodeId="node-llm" />
        <DocumentObserver
          onChange={(nextDocument) => {
            latestDocument = nextDocument;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      await screen.findByRole('button', { name: '展开本地 SYSTEM' })
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('SYSTEM 消息内容')).not.toBeInTheDocument();
    expect(promptMessagesFrom(latestDocument)[0]?.content.value).toBe(
      'Keep answers short.'
    );

    fireEvent.click(screen.getByRole('button', { name: '展开本地 SYSTEM' }));

    expect(screen.getByLabelText('SYSTEM 消息内容')).toBeInTheDocument();
    expect(screen.getByText('Keep answers short.')).toBeInTheDocument();
  });

  test('updates the LLM context policy from the inspector', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialState(latestDocument)}
      >
        <SelectionSeed nodeId="node-llm" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const switchControl = await screen.findByRole('switch', {
      name: '上下文'
    });
    expect(switchControl).toBeChecked();
    expect(screen.getByRole('combobox', { name: '上下文变量' })).toBeInTheDocument();

    fireEvent.click(switchControl);

    expect(llmNodeFrom(latestDocument).config.context_policy).toEqual({
      integration_context: 'disabled',
      context_selector: ['node-start', 'history']
    });
  });

  test('renders LLM context policy as a single inspector row', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await screen.findByRole('switch', { name: '上下文' });

    const contextRow = screen.getByTestId(
      'inspector-field-config.context_policy'
    );

    expect(contextRow).toHaveClass(
      'agent-flow-editor__inspector-field--policy'
    );
    expect(within(contextRow).getByText('上下文')).toBeInTheDocument();
    expect(
      within(contextRow).getByRole('combobox', { name: '上下文变量' })
    ).toBeInTheDocument();
    expect(
      screen.getByLabelText('将传入上下文注入当前LLM节点中')
    ).toBeInTheDocument();
  });

  test('renders disconnected context selector references with the missing value style', async () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    document.graph.edges = document.graph.edges.filter(
      (edge) => !(edge.source === 'node-start' && edge.target === 'node-llm')
    );

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState(document)}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const missingValue = await screen.findByText('history');

    expect(missingValue).toHaveClass(
      'agent-flow-node-detail__context-select-missing-value'
    );
    expect(screen.queryByText('node-start.history')).not.toBeInTheDocument();
    expect(
      screen.queryByText('["node-start","history"]')
    ).not.toBeInTheDocument();
  });

  test('normalizes existing prompt messages so system remains the first fixed row', async () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-llm'
        ? {
            ...node,
            bindings: {
              prompt_messages: {
                kind: 'prompt_messages',
                value: [
                  {
                    id: 'user-first',
                    role: 'user',
                    content: { kind: 'templated_text', value: 'Question' }
                  },
                  {
                    id: 'system-second',
                    role: 'system',
                    content: { kind: 'templated_text', value: 'Rules' }
                  },
                  {
                    id: 'assistant-third',
                    role: 'assistant',
                    content: { kind: 'templated_text', value: 'Earlier answer' }
                  }
                ]
              }
            }
          }
        : node
    );

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState(document)}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: '展开本地 SYSTEM' })
    );
    expect(screen.getByText('Rules')).toBeInTheDocument();
    const rows = screen.getAllByTestId(/llm-prompt-message-row-/);
    expect(rows[0]).toHaveAttribute(
      'data-testid',
      'llm-prompt-message-row-system-second'
    );
    expect(within(rows[0]).getByText('SYSTEM')).toBeInTheDocument();
    expect(within(rows[1]).getByLabelText('USER 消息内容')).toBeInTheDocument();
    expect(
      within(rows[2]).getByLabelText('ASSISTANT 消息内容')
    ).toBeInTheDocument();
  });
});
