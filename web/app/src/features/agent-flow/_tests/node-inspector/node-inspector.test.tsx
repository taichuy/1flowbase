import { describe, test, expect, vi } from 'vitest';
import { domEvents } from './node-inspector.fixtures';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  FocusIssueSeed,
  SelectionSeed,
  createAgentFlowNodeSchemaAdapterSpy,
  createInitialState,
  fetchModelProviderOptionsSpy,
  getLlmNodeConfig,
  primaryProviderFirstModel,
  primaryProviderOption,
  renderWithProviders,
  resolveAgentFlowNodeSchemaSpy
} from './support';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { NodeDetailPanel } from '../../components/detail/NodeDetailPanel';
import { NodeInspector } from '../../components/inspector/NodeInspector';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { createNodeDocument } from '../../lib/document/node-factory';
import { modelProviderOptionsContract } from '../../../../test/model-provider-contract-fixtures';
import { screen, waitFor, within } from '@testing-library/react';

describe('NodeInspector core', () => {
  test('reads only the current SQL binding and writes it on edit', async () => {
    const state = createInitialState();
    const sqlNode = createNodeDocument('sql', 'node-sql', 720, 240);
    sqlNode.bindings.sql = {
      kind: 'templated_text',
      value: 'select {{node-start.query}}'
    };
    sqlNode.config.sql = 'retired SQL must not override the current binding';
    state.draft.document.graph.nodes.push(sqlNode);
    let latestDocument = state.draft.document;

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-sql" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const editor = await screen.findByDisplayValue(
      'select {{node-start.query}}'
    );
    await domEvents.change(editor, {
      target: { value: 'select * from users where id = {{node-start.query}}' }
    });

    await waitFor(() => {
      const updatedSqlNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-sql'
      );
      expect(updatedSqlNode?.bindings.sql).toEqual({
        kind: 'templated_text',
        value: 'select * from users where id = {{node-start.query}}'
      });
    });
  });

  test('AC-012/013 renders backend-projected i18n_text as inert text and falls back to the typed English key', async () => {
    const translatedState = createInitialState();
    const answerNode = createNodeDocument(
      'answer',
      'node-answer-i18n',
      720,
      240
    );
    answerNode.bindings.answer_template = {
      kind: 'i18n_text',
      value: {
        key: 'Welcome'
      }
    };
    translatedState.draft.document.graph.nodes.push(answerNode);
    translatedState.messages = [
      {
        key: 'Welcome',
        text: '<strong>欢迎</strong>'
      }
    ];

    const { unmount } = await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={translatedState}>
        <SelectionSeed nodeId="node-answer-i18n" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const translated = screen.getByTestId('i18n-text-binding');
    expect(translated).toHaveAttribute('data-ready', 'true');
    expect(translated).toHaveTextContent('<strong>欢迎</strong>');
    expect(translated.querySelector('strong')).toBeNull();
    expect(
      screen.queryByRole('textbox', { name: '回答内容' })
    ).not.toBeInTheDocument();
    unmount();

    const fallbackState = createInitialState();
    const fallbackNode = createNodeDocument(
      'answer',
      'node-answer-i18n-fallback',
      720,
      240
    );
    fallbackNode.bindings.answer_template = answerNode.bindings.answer_template;
    fallbackState.draft.document.graph.nodes.push(fallbackNode);

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={fallbackState}>
        <SelectionSeed nodeId="node-answer-i18n-fallback" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(screen.getByTestId('i18n-text-binding')).toHaveTextContent(
      'Welcome'
    );
  });

  test('reads config sections through the node schema registry and adapter bridge', async () => {
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeInspector />
      </AgentFlowEditorStoreProvider>
    );

    await waitFor(() => {
      expect(screen.getByLabelText('USER 消息内容')).toHaveAttribute(
        'contenteditable',
        'true'
      );
    });
    expect(resolveAgentFlowNodeSchemaSpy).toHaveBeenCalledWith('llm');
    expect(createAgentFlowNodeSchemaAdapterSpy).toHaveBeenCalledWith(
      expect.objectContaining({ nodeId: 'node-llm' })
    );
  });

  test('renders config sections as always-open blocks without repeating basics once summary content moves out', async () => {
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeInspector />
      </AgentFlowEditorStoreProvider>
    );

    await waitFor(() => {
      expect(screen.getByLabelText('USER 消息内容')).toHaveAttribute(
        'contenteditable',
        'true'
      );
    });
    expect(
      screen.queryByRole('button', { name: 'Inputs' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Policy' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Advanced' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: 'USER 消息内容' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('Basics')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('节点别名')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('节点简介')).not.toBeInTheDocument();
    expect(screen.queryByText('Inputs')).not.toBeInTheDocument();
    expect(screen.queryByText('Outputs')).not.toBeInTheDocument();
    expect(screen.queryByText('Advanced')).not.toBeInTheDocument();
    expect(screen.queryByText('LLM 参数')).not.toBeInTheDocument();
    expect(screen.queryByText('返回格式')).not.toBeInTheDocument();
    expect(screen.getByLabelText('失败重试')).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '异常处理' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('SYSTEM 消息内容')).toBeInTheDocument();
    expect(screen.getByLabelText('USER 消息内容').tagName).toBe('DIV');
    expect(screen.getByLabelText('USER 消息内容')).toHaveAttribute(
      'contenteditable',
      'true'
    );
  }, 10000);

  test('updates node identity through header interactions instead of mutating document inline', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-start" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeDetailPanel
          activeTab="config"
          onTabChange={vi.fn()}
          onClose={vi.fn()}
          onRunNode={undefined}
        />
      </AgentFlowEditorStoreProvider>
    );

    const header = screen.getByTestId('node-detail-header');

    await domEvents.change(within(header).getByLabelText('节点别名'), {
      target: { value: '入口节点' }
    });
    await domEvents.change(within(header).getByLabelText('节点简介'), {
      target: { value: '收集首轮用户输入并启动工作流。' }
    });

    expect(within(header).getByLabelText('节点别名')).toHaveValue('入口节点');
    expect(within(header).getByLabelText('节点简介')).toHaveValue(
      '收集首轮用户输入并启动工作流。'
    );
    expect(latestDocument.graph.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-start',
          alias: '入口节点',
          description: '收集首轮用户输入并启动工作流。'
        })
      ])
    );
  });

  test('keeps issue-driven focus working after the inspector loses its header chrome', async () => {
    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <FocusIssueSeed />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /模型|model/ })).toHaveFocus();
    });
  });

  test('shows the effective context in the model summary trigger', async () => {
    const state = createInitialState();
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    llmNodeConfig.model_provider = {
      provider_code: primaryProviderOption.provider_code,
      model_id: primaryProviderFirstModel.model_id,
      provider_label: primaryProviderOption.display_name,
      model_label: primaryProviderFirstModel.display_name
    };
    fetchModelProviderOptionsSpy.mockResolvedValue(
      modelProviderOptionsContract
    );

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const modelTrigger = await screen.findByRole('button', {
      name: /模型|model/
    });

    await waitFor(() => {
      expect(
        within(modelTrigger).getByLabelText('上下文 128K')
      ).toBeInTheDocument();
    });
  });
});
