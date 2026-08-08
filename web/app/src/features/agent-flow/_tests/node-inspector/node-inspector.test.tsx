import { readFileSync } from 'node:fs';

import { useState } from 'react';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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

describe('NodeInspector core', () => {
  test('preserves legacy SQL text and writes the templated binding on edit', async () => {
    const state = createInitialState();
    const sqlNode = createNodeDocument('sql', 'node-sql', 720, 240);
    delete sqlNode.bindings.sql;
    sqlNode.config.sql = 'select {{node-start.query}}';
    state.draft.document.graph.nodes.push(sqlNode);
    let latestDocument = state.draft.document;

    renderWithProviders(
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
    fireEvent.change(editor, {
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

  test('AC-012/013 renders backend-projected i18n_text as inert text and falls back to the typed English key', () => {
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

    const { unmount } = renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={translatedState}>
        <SelectionSeed nodeId="node-answer-i18n" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const translated = screen.getByTestId('i18n-text-binding');
    expect(translated).toHaveAttribute('data-ready', 'true');
    expect(translated).toHaveTextContent('<strong>欢迎</strong>');
    expect(translated.querySelector('strong')).toBeNull();
    expect(screen.queryByRole('textbox', { name: '回答内容' })).not.toBeInTheDocument();
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

    renderWithProviders(
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
    renderWithProviders(
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
    expect(createAgentFlowNodeSchemaAdapterSpy).toHaveBeenCalledTimes(1);
  });

  test('renders config sections as always-open blocks without repeating basics once summary content moves out', async () => {
    renderWithProviders(
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

  test('updates node identity through header interactions instead of mutating document inline', () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    renderWithProviders(
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

    fireEvent.change(within(header).getByLabelText('节点别名'), {
      target: { value: '入口节点' }
    });
    fireEvent.change(within(header).getByLabelText('节点简介'), {
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
    renderWithProviders(
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

    renderWithProviders(
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

  test('edits LLM mounted tools through compact rows and a modal form', async () => {
    const state = createInitialState();
    let latestDocument = state.draft.document;
    const mountedLlm = createNodeDocument('llm', 'node-mounted-llm', 720, 240);
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    mountedLlm.alias = '视觉 LLM';
    state.draft.document.graph.nodes.push(mountedLlm);
    llmNodeConfig.visible_internal_llm_tools_enabled = true;
    llmNodeConfig.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'inspect_visible_context',
        connector_id: 'inspect_visible_context',
        target_node_id: 'node-mounted-llm',
        description: 'Inspect visible context',
        input_schema: { type: 'object' }
      }
    ];
    state.draft.document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-llm-mounted-tool',
        source: 'node-llm',
        target: 'node-mounted-llm',
        sourceHandle: 'visible_internal_llm_tool:inspect_visible_context'
      })
    );

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

    expect(
      await screen.findByText('inspect_visible_context')
    ).toBeInTheDocument();
    const mountToolsField = screen.getByTestId(
      'inspector-field-config.visible_internal_llm_tools_enabled'
    );
    const mountToolsToolbar = within(mountToolsField).getByTestId(
      'agent-flow-llm-tool-registrations-toolbar'
    );
    const addToolButton = within(mountToolsToolbar).getByRole('button', {
      name: '添加工具'
    });
    const mountToolsSwitch = within(mountToolsToolbar).getByRole('switch', {
      name: '挂载 LLM'
    });
    const mountToolsActions = mountToolsToolbar.querySelector(
      '.agent-flow-llm-tool-registrations__actions'
    );

    expect(mountToolsActions).not.toBeNull();
    expect(mountToolsActions).toContainElement(addToolButton);
    expect(mountToolsActions).toContainElement(mountToolsSwitch);

    expect(within(mountToolsToolbar).getByText('挂载 LLM')).toBeInTheDocument();
    expect(
      within(addToolButton).getByTestId(
        'agent-flow-llm-tool-registration-add-icon'
      )
    ).toBeInTheDocument();
    expect(
      within(mountToolsToolbar).queryByText('添加工具')
    ).not.toBeInTheDocument();
    expect(mountToolsSwitch).toHaveClass(
      'agent-flow-llm-tool-registrations__switch'
    );
    expect(
      readFileSync(
        'src/features/agent-flow/components/editor/styles/inspector.css',
        'utf8'
      )
    ).toMatch(
      /\.agent-flow-llm-tool-registrations__actions\s*\{[^}]*margin-left:\s*auto;/s
    );
    expect(within(mountToolsField).getAllByText('挂载 LLM')).toHaveLength(1);
    expect(
      screen.queryByRole('columnheader', { name: '目标 LLM' })
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: '编辑 inspect_visible_context' })
    );

    const dialog = await screen.findByRole('dialog', { name: '编辑 工具注册' });

    expect(within(dialog).queryByLabelText('目标 LLM')).not.toBeInTheDocument();
    const internalLlmSwitch = within(dialog).getByRole('switch', {
      name: '允许分支 LLM'
    });
    expect(internalLlmSwitch).not.toBeChecked();
    expect(
      within(dialog).queryByRole('combobox', { name: '工具模式' })
    ).not.toBeInTheDocument();
    const saveToolButton = within(dialog).getByRole('button', {
      name: '保存工具'
    });

    fireEvent.change(within(dialog).getByLabelText('工具标识'), {
      target: { value: 'inspect-image' }
    });
    expect(saveToolButton).toBeDisabled();
    expect(
      within(dialog).getByText('仅支持 1-64 位数字、大小写字母、下划线。')
    ).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('工具标识'), {
      target: { value: 'inspect_image' }
    });
    fireEvent.change(within(dialog).getByLabelText('工具名称'), {
      target: { value: 'inspect_image' }
    });
    fireEvent.change(within(dialog).getByLabelText('描述'), {
      target: { value: 'Inspect uploaded image' }
    });
    fireEvent.click(internalLlmSwitch);
    const toolModeSelect = within(dialog).getByRole('combobox', {
      name: '工具模式'
    });
    expect(
      internalLlmSwitch.compareDocumentPosition(toolModeSelect) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    fireEvent.click(saveToolButton);

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument)).toEqual(
        expect.objectContaining({
          visible_internal_llm_tools: [
            expect.objectContaining({
              tool_name: 'inspect_image',
              connector_id: 'inspect_image',
              target_node_id: 'node-mounted-llm',
              description: 'Inspect uploaded image',
              tool_mode: 'agent',
              internal_llm_node_policy: 'allowed'
            })
          ]
        })
      );
    });
    expect(latestDocument.graph.edges).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-llm-mounted-tool',
          sourceHandle: 'visible_internal_llm_tool:inspect_image'
        })
      ])
    );
    expect(await screen.findByText('inspect_image')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '删除 inspect_image' }));

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument)).toEqual(
        expect.objectContaining({
          visible_internal_llm_tools: []
        })
      );
    });
    expect(screen.getByText('暂无工具注册')).toBeInTheDocument();
  });

  test('AC-011 renders MCP mounts as one embedded collection field', async () => {
    const state = createInitialState();
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    llmNodeConfig.mcp_instance_ids = ['missing-instance'];

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const mcpMountField = await screen.findByTestId(
      'inspector-field-config.mcp_instance_ids'
    );

    expect(within(mcpMountField).getAllByText('挂载 MCP')).toHaveLength(1);
    expect(
      within(mcpMountField).getByTestId('agent-flow-mcp-instances-toolbar')
    ).toBeInTheDocument();
    expect(
      within(mcpMountField).getByText('missing-instance')
    ).toBeInTheDocument();
    expect(
      within(mcpMountField).queryByRole('combobox')
    ).not.toBeInTheDocument();
    expect(within(mcpMountField).queryByRole('switch')).not.toBeInTheDocument();
  });

  test('keeps Fusion mounted tools from saving inherited external tools', async () => {
    const state = createInitialState();
    let latestDocument = state.draft.document;
    const mountedLlm = createNodeDocument('llm', 'node-panel-seed', 720, 240);
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    state.draft.document.graph.nodes.push(mountedLlm);
    llmNodeConfig.visible_internal_llm_tools_enabled = true;
    llmNodeConfig.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'compare_panels',
        connector_id: 'compare_panels',
        target_node_id: 'node-panel-seed',
        description: 'Compare panels',
        tool_mode: 'fusion',
        internal_llm_node_policy: 'allowed',
        external_tool_policy: 'inherited',
        input_schema: { type: 'object' }
      }
    ];
    state.draft.document.graph.edges.push(
      createEdgeDocument({
        id: 'edge-llm-fusion-tool',
        source: 'node-llm',
        target: 'node-panel-seed',
        sourceHandle: 'visible_internal_llm_tool:compare_panels'
      })
    );

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

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 compare_panels' })
    );

    const dialog = await screen.findByRole('dialog', { name: '编辑 工具注册' });

    const internalLlmSwitch = within(dialog).getByRole('switch', {
      name: '允许分支 LLM'
    });
    const toolModeSelect = within(dialog).getByRole('combobox', {
      name: '工具模式'
    });
    expect(internalLlmSwitch).toBeChecked();
    expect(
      internalLlmSwitch.compareDocumentPosition(toolModeSelect) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(dialog).queryByRole('switch', { name: '开放外部工具' })
    ).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '保存工具' }));

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument)).toEqual(
        expect.objectContaining({
          visible_internal_llm_tools: [
            expect.objectContaining({
              tool_name: 'compare_panels',
              tool_mode: 'fusion',
              internal_llm_node_policy: 'allowed',
              external_tool_policy: 'forbidden'
            })
          ]
        })
      );
    });
  });

  test('edits routed LLM tool input parameters from a full protocol JSON', async () => {
    const state = createInitialState();
    let latestDocument = state.draft.document;
    const mountedLlm = createNodeDocument('llm', 'node-mounted-llm', 720, 240);
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    state.draft.document.graph.nodes.push(mountedLlm);
    llmNodeConfig.visible_internal_llm_tools_enabled = true;
    llmNodeConfig.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'image_llm',
        connector_id: 'image_llm',
        target_node_id: 'node-mounted-llm',
        description: 'Inspect images',
        preconditions: [
          {
            kind: 'media_content_available',
            argument_path: ['media'],
            media_kind: 'image'
          }
        ],
        input_schema: {
          type: 'object',
          required: ['task'],
          properties: {
            task: {
              type: 'string',
              description: '给多模态模型的任务指示提示词'
            },
            media: {
              type: 'array',
              description: '需要交给多模态模型处理的媒体引用',
              items: {
                type: 'object',
                required: ['kind', 'source', 'path'],
                properties: {
                  kind: {
                    type: 'string',
                    enum: ['image'],
                    description: '媒体类型'
                  },
                  source: {
                    type: 'string',
                    enum: ['workspace_path'],
                    description: '媒体来源'
                  },
                  path: {
                    type: 'string',
                    description:
                      '工作区内图片路径，例如 uploads/image_aionui_1781014667000.png'
                  }
                }
              }
            }
          }
        }
      }
    ];

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

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 image_llm' })
    );

    const dialog = await screen.findByRole('dialog', { name: '编辑 工具注册' });
    const preconditionsEditor = within(dialog).getByTestId(
      'agent-flow-llm-tool-preconditions-json-editor'
    );

    expect(within(dialog).getByText('前置拦截')).toBeInTheDocument();
    expect(preconditionsEditor).toHaveClass(
      'agent-flow-llm-tool-registration-preconditions'
    );
    fireEvent.click(
      within(preconditionsEditor).getByRole('tab', { name: 'JSON 解析' })
    );
    const preconditionsInput =
      await within(preconditionsEditor).findByLabelText('前置拦截 JSON');
    expect(preconditionsInput).toHaveValue(
      JSON.stringify(
        [
          {
            kind: 'media_content_available',
            argument_path: ['media'],
            media_kind: 'image'
          }
        ],
        null,
        2
      )
    );
    expect(within(dialog).getByLabelText('Schema 字段名 1')).toHaveValue(
      'task'
    );
    expect(within(dialog).getByLabelText('Schema 字段描述 1')).toHaveValue(
      '给多模态模型的任务指示提示词'
    );
    expect(within(dialog).getByLabelText('Schema 字段名 2')).toHaveValue(
      'media'
    );
    expect(
      within(dialog).queryByLabelText('Schema 字段名 3')
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByDisplayValue('preconditions')
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(dialog).getAllByRole('tab', { name: 'JSON 解析' })[1]
    );
    fireEvent.change(await within(dialog).findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify(
          {
            input_schema: {
              type: 'object',
              required: ['task', 'attachments'],
              properties: {
                task: {
                  type: 'string',
                  description: '给多模态模型的新任务指示'
                },
                attachments: {
                  type: 'array',
                  description: '需要交给多模态模型处理的新媒体引用',
                  items: {
                    type: 'object',
                    required: ['kind', 'source', 'path'],
                    properties: {
                      kind: {
                        type: 'string',
                        enum: ['image'],
                        description: '媒体类型'
                      },
                      source: {
                        type: 'string',
                        enum: ['workspace_path'],
                        description: '媒体来源'
                      },
                      path: {
                        type: 'string',
                        description: '工作区内图片路径'
                      }
                    }
                  }
                }
              }
            },
            preconditions: [
              {
                kind: 'media_content_available',
                argument_path: ['attachments'],
                media_kind: 'image'
              }
            ]
          },
          null,
          2
        )
      }
    });
    fireEvent.click(
      within(dialog).getAllByRole('tab', { name: 'Schema 字段' })[1]
    );

    await waitFor(() => {
      expect(within(dialog).getByLabelText('Schema 字段名 1')).toHaveValue(
        'task'
      );
      expect(within(dialog).getByLabelText('Schema 字段名 2')).toHaveValue(
        'attachments'
      );
      expect(
        within(dialog).queryByDisplayValue('input_schema')
      ).not.toBeInTheDocument();
      expect(
        within(dialog).queryByDisplayValue('preconditions')
      ).not.toBeInTheDocument();
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '保存工具' }));

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument)).toEqual(
        expect.objectContaining({
          visible_internal_llm_tools: [
            expect.objectContaining({
              tool_name: 'image_llm',
              preconditions: [
                {
                  kind: 'media_content_available',
                  argument_path: ['attachments'],
                  media_kind: 'image'
                }
              ],
              input_schema: expect.objectContaining({
                properties: expect.objectContaining({
                  task: expect.objectContaining({
                    description: '给多模态模型的新任务指示'
                  }),
                  attachments: expect.objectContaining({
                    description: '需要交给多模态模型处理的新媒体引用'
                  })
                })
              })
            })
          ]
        })
      );
    });
  });

  test('saves routed LLM tool preconditions from the fields editor', async () => {
    const state = createInitialState();
    let latestDocument = state.draft.document;
    const mountedLlm = createNodeDocument('llm', 'node-mounted-llm', 720, 240);
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);

    state.draft.document.graph.nodes.push(mountedLlm);
    llmNodeConfig.visible_internal_llm_tools_enabled = true;
    llmNodeConfig.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'image_llm',
        connector_id: 'image_llm',
        target_node_id: 'node-mounted-llm',
        description: 'Inspect images',
        preconditions: [{}],
        input_schema: {
          type: 'object',
          required: ['task'],
          properties: {
            task: {
              type: 'string'
            },
            media: {
              type: 'array'
            }
          }
        }
      }
    ];

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

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 image_llm' })
    );

    const dialog = await screen.findByRole('dialog', { name: '编辑 工具注册' });
    const preconditionsEditor = within(dialog).getByTestId(
      'agent-flow-llm-tool-preconditions-json-editor'
    );

    expect(
      within(preconditionsEditor).getByLabelText(
        '前置拦截 1 字段值 argument_path'
      )
    ).toHaveValue('media');
    fireEvent.change(
      within(preconditionsEditor).getByLabelText(
        '前置拦截 1 字段值 argument_path'
      ),
      {
        target: { value: 'media' }
      }
    );
    fireEvent.change(
      within(preconditionsEditor).getByLabelText(
        '前置拦截 1 字段值 media_kind'
      ),
      {
        target: { value: 'image' }
      }
    );
    fireEvent.click(within(dialog).getByRole('button', { name: '保存工具' }));

    await waitFor(() => {
      expect(getLlmNodeConfig(latestDocument)).toEqual(
        expect.objectContaining({
          visible_internal_llm_tools: [
            expect.objectContaining({
              tool_name: 'image_llm',
              preconditions: [
                {
                  kind: 'media_content_available',
                  argument_path: ['media'],
                  media_kind: 'image'
                }
              ]
            })
          ]
        })
      );
    });
  });

  test('keeps the floating tool editor drag handle visible while the body crosses viewport boundaries', async () => {
    const state = createInitialState();
    const llmNodeConfig = getLlmNodeConfig(state.draft.document);
    const margin = 16;
    const dragHandleWidth = 180;
    const dragHandleHeight = 42;

    llmNodeConfig.visible_internal_llm_tools_enabled = true;
    llmNodeConfig.visible_internal_llm_tools = [
      {
        type: 'visible_internal_llm_tool',
        tool_name: 'inspect_visible_context',
        connector_id: 'inspect_visible_context',
        target_node_id: 'node-llm',
        description: 'Inspect visible context',
        input_schema: { type: 'object' }
      }
    ];

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    fireEvent.click(
      await screen.findByRole('button', {
        name: '编辑 inspect_visible_context'
      })
    );

    const dialog = await screen.findByRole('dialog', {
      name: '编辑 工具注册'
    });
    const dragHandle = within(dialog).getByTestId(
      'agent-flow-llm-tool-registration-drag-handle'
    );

    vi.spyOn(dragHandle, 'getBoundingClientRect').mockReturnValue({
      bottom: dragHandleHeight,
      height: dragHandleHeight,
      left: 0,
      right: dragHandleWidth,
      top: 0,
      width: dragHandleWidth,
      x: 0,
      y: 0,
      toJSON: () => ({})
    } as DOMRect);

    const panelHeight = Number.parseFloat(dialog.style.height);
    const panelWidth = Number.parseFloat(dialog.style.width);
    const fullPanelMaxLeft = window.innerWidth - panelWidth - margin;
    const fullPanelMaxTop = window.innerHeight - panelHeight - margin;

    fireEvent.mouseDown(dragHandle, {
      button: 0,
      clientX: 24,
      clientY: 24
    });
    fireEvent.mouseMove(window, {
      clientX: window.innerWidth + 200,
      clientY: 24
    });
    fireEvent.mouseUp(window);

    await waitFor(() =>
      expect(Number.parseFloat(dialog.style.left)).toBeGreaterThan(
        fullPanelMaxLeft
      )
    );

    const panelLeft = Number.parseFloat(dialog.style.left);

    expect(panelLeft).toBeGreaterThanOrEqual(margin);
    expect(panelLeft + dragHandleWidth).toBeLessThanOrEqual(
      window.innerWidth - margin
    );
    expect(panelLeft + panelWidth).toBeGreaterThan(window.innerWidth);

    fireEvent.mouseDown(dragHandle, {
      button: 0,
      clientX: window.innerWidth - margin - 24,
      clientY: 24
    });
    fireEvent.mouseMove(window, {
      clientX: window.innerWidth - margin - 24,
      clientY: window.innerHeight + 200
    });
    fireEvent.mouseUp(window);

    await waitFor(() =>
      expect(Number.parseFloat(dialog.style.top)).toBeGreaterThan(
        fullPanelMaxTop
      )
    );

    const panelTop = Number.parseFloat(dialog.style.top);

    expect(panelTop + dragHandleHeight).toBeLessThanOrEqual(
      window.innerHeight - margin
    );
    expect(panelTop + panelHeight).toBeGreaterThan(window.innerHeight);
  });

  test('collapses generated outputs by default and keeps output contract editing hidden', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByText('输出变量')).toBeInTheDocument();
    expect(screen.queryByText('text')).not.toBeInTheDocument();
    expect(screen.queryByText('usage')).not.toBeInTheDocument();
    expect(screen.queryByText('reasoning_content')).not.toBeInTheDocument();
    expect(screen.queryByText('节点产出的数据字段')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '新增输出变量' })
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText('输出变量名 1')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '输出变量' }));

    expect(screen.getByText('text')).toBeInTheDocument();
    expect(screen.getByText('usage')).toBeInTheDocument();
  });

  test('renders field validation errors under the owning inspector field', () => {
    const state = createInitialState();
    const answerNode = state.draft.document.graph.nodes.find(
      (node) => node.id === 'node-answer'
    );

    if (!answerNode) {
      throw new Error('expected default Answer node');
    }

    answerNode.bindings.answer_template = {
      kind: 'templated_text',
      value: '{{node-llm.text}}\n----\n{{node-llm-1.text}}'
    };

    const schema = nodeSchemaRegistry.resolveAgentFlowNodeSchema('answer');
    const adapter = nodeSchemaAdapterApi.createAgentFlowNodeSchemaAdapter({
      document: state.draft.document,
      nodeId: 'node-answer',
      issues: validateDocument(state.draft.document),
      setWorkingDocument: vi.fn(),
      dispatch: vi.fn()
    });

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <NodeInspector schema={schema} adapter={adapter} />
      </AgentFlowEditorStoreProvider>
    );

    const field = screen.getByTestId(
      'inspector-field-bindings.answer_template'
    );

    expect(field).toHaveClass('agent-flow-editor__inspector-field--error');
    expect(
      within(field).getByText(
        '当前 binding 引用了已删除节点 node-llm-1 的输出。'
      )
    ).toBeInTheDocument();
  });

  test('renders HTTP Request config panel and imports a basic curl command', async () => {
    const state = createInitialStateWithHttpRequestNode();
    let latestDocument = state.draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-http-request" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      await screen.findByRole('combobox', { name: '请求方法' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('URL')).toHaveAttribute(
      'contenteditable',
      'true'
    );
    expect(screen.getByText('Params')).toBeInTheDocument();
    expect(screen.getByText('Headers')).toBeInTheDocument();
    expect(screen.getAllByText('body').length).toBeGreaterThan(0);
    expect(
      screen.queryByText('支持正文变量块，输入"/"或左花括号可快速引用')
    ).not.toBeInTheDocument();
    expect(
      screen.getAllByText('输入"/"或左花括号可快速引用').length
    ).toBeGreaterThan(0);
    expect(screen.getByLabelText('验证 SSL 证书')).toBeChecked();
    const storeResponseAsFileSwitch = screen.getByRole('switch', {
      name: '转存为文件'
    });
    expect(storeResponseAsFileSwitch).not.toBeChecked();
    fireEvent.click(storeResponseAsFileSwitch);
    await waitFor(() => {
      const httpNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-http-request'
      );

      expect(httpNode?.config.store_response_as_file).toBe(true);
    });
    expect(screen.getByLabelText('超时设置(ms)')).toBeInTheDocument();
    const maxResponseSizeInput = screen.getByLabelText('最大响应体(MB)');
    expect(maxResponseSizeInput).toHaveValue('6');
    fireEvent.change(maxResponseSizeInput, { target: { value: '8' } });
    await waitFor(() => {
      const httpNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-http-request'
      );

      expect(httpNode?.config.max_response_bytes).toBe(8 * 1024 * 1024);
    });
    expect(screen.queryByText('status_code')).not.toBeInTheDocument();
    expect(screen.queryByText('headers')).not.toBeInTheDocument();
    expect(screen.queryByText('files')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '输出变量' }));

    expect(screen.getAllByText('body').length).toBeGreaterThan(0);
    expect(screen.getByText('status_code')).toBeInTheDocument();
    expect(screen.getByText('headers')).toBeInTheDocument();
    expect(screen.getByText('files')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '导入 cURL' }));
    fireEvent.change(await screen.findByLabelText('cURL 命令'), {
      target: {
        value:
          "curl -X POST 'https://api.example.com/orders?page=1' -H 'Authorization: Bearer token' -H 'Content-Type: application/json' -d '{\"query\":\"{{node-start.query}}\"}'"
      }
    });
    fireEvent.click(screen.getByRole('button', { name: '导入请求' }));

    await waitFor(() => {
      const httpNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-http-request'
      );

      expect(httpNode?.config).toMatchObject({
        method: 'POST',
        url: 'https://api.example.com/orders',
        body_type: 'json'
      });
      expect(httpNode?.bindings.params).toEqual({
        kind: 'named_bindings',
        value: [
          {
            name: 'page',
            value: { kind: 'templated_text', value: '1' }
          }
        ]
      });
      expect(httpNode?.bindings.headers).toEqual({
        kind: 'named_bindings',
        value: [
          {
            name: 'Authorization',
            value: { kind: 'templated_text', value: 'Bearer token' }
          },
          {
            name: 'Content-Type',
            value: { kind: 'templated_text', value: 'application/json' }
          }
        ]
      });
      expect(httpNode?.bindings.body).toEqual({
        kind: 'templated_text',
        value: '{"query":"{{node-start.query}}"}'
      });
    });
  }, 10000);

  test('renders repeated HTTP Request sections without duplicate React keys', async () => {
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);

    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithHttpRequestNode()}
      >
        <SelectionSeed nodeId="node-http-request" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      await screen.findByRole('combobox', { name: '请求方法' })
    ).toBeInTheDocument();
    expect(
      consoleErrorSpy.mock.calls
        .flat()
        .some((message) =>
          String(message).includes('Encountered two children with the same key')
        )
    ).toBe(false);

    consoleErrorSpy.mockRestore();
  });

  test('keeps code output contract definition editable without rendering the shared output contract card', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithCodeNode()}
      >
        <SelectionSeed nodeId="node-code" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      await screen.findByLabelText(/JavaScript 代码|JavaScript code/)
    ).toBeInTheDocument();
    expect(screen.queryByText('JavaScript 代码')).not.toBeInTheDocument();
    expect(screen.queryByText('输出契约')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '新增变量' })
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('代码结果')).not.toBeInTheDocument();
  });

  test('keeps Code boolean input selector values visible in the single value column', () => {
    renderWithProviders(
      <TemplatedNamedBindingsField
        ariaLabel="inputs"
        options={[
          {
            nodeId: 'node-start',
            nodeLabel: 'Start',
            outputKey: 'approved',
            outputLabel: 'approved',
            valueType: 'boolean',
            value: ['node-start', 'approved'],
            displayLabel: 'Start.approved'
          }
        ]}
        value={[
          {
            name: 'approved',
            valueType: 'boolean',
            value: {
              kind: 'selector',
              selector: ['node-start', 'approved']
            }
          }
        ]}
        onChange={vi.fn()}
      />
    );

    expect(screen.getByText('Start.approved')).toBeInTheDocument();
    expect(
      screen.queryByLabelText('inputs-0-value-mode')
    ).not.toBeInTheDocument();
  });

  test('adds Code input rows without preselecting a parameter type', () => {
    const handleChange = vi.fn();

    renderWithProviders(
      <TemplatedNamedBindingsField
        ariaLabel="inputs"
        options={[]}
        value={[]}
        onChange={handleChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '新增变量' }));

    expect(handleChange).toHaveBeenCalledWith([
      {
        name: '',
        value: { kind: 'constant', value: '' }
      }
    ]);
  });

  test('keeps named binding input focused when editing its name', () => {
    renderWithProviders(<NamedBindingsFocusHarness />);

    const nameInput = screen.getByLabelText('bindings-0-name');
    nameInput.focus();
    fireEvent.change(nameInput, {
      target: { value: 'arg2' }
    });

    expect(screen.getByLabelText('bindings-0-name')).toHaveFocus();
  });
});
