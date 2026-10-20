import { describe, test, expect, vi } from 'vitest';
import {
  AgentFlowEditorStoreProvider,
  DocumentObserver,
  NodeConfigTab,
  SelectionSeed,
  createEdgeDocument,
  createInitialState,
  createNodeDocument,
  domEvents,
  getLlmNodeConfig,
  readFileSync,
  renderWithProviders,
  screen,
  waitFor,
  within
} from './node-inspector.fixtures';

describe('NodeInspector core', () => {
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

    await renderWithProviders(
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

    await domEvents.click(
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

    await domEvents.change(within(dialog).getByLabelText('工具标识'), {
      target: { value: 'inspect-image' }
    });
    expect(saveToolButton).toBeDisabled();
    expect(
      within(dialog).getByText('仅支持 1-64 位数字、大小写字母、下划线。')
    ).toBeInTheDocument();
    await domEvents.change(within(dialog).getByLabelText('工具标识'), {
      target: { value: 'inspect_image' }
    });
    await domEvents.change(within(dialog).getByLabelText('工具名称'), {
      target: { value: 'inspect_image' }
    });
    await domEvents.change(within(dialog).getByLabelText('描述'), {
      target: { value: 'Inspect uploaded image' }
    });
    await domEvents.click(internalLlmSwitch);
    const toolModeSelect = within(dialog).getByRole('combobox', {
      name: '工具模式'
    });
    expect(
      internalLlmSwitch.compareDocumentPosition(toolModeSelect) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    await domEvents.click(saveToolButton);

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

    await domEvents.click(
      screen.getByRole('button', { name: '删除 inspect_image' })
    );

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

    await renderWithProviders(
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

    await renderWithProviders(
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

    await domEvents.click(
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

    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存工具' })
    );

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

    await renderWithProviders(
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

    await domEvents.click(
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
    await domEvents.click(
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

    await domEvents.click(
      within(dialog).getAllByRole('tab', { name: 'JSON 解析' })[1]
    );
    await domEvents.change(
      await within(dialog).findByLabelText('JSON Schema 内容'),
      {
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
      }
    );
    await domEvents.click(
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
    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存工具' })
    );

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

    await renderWithProviders(
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

    await domEvents.click(
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
    await domEvents.change(
      within(preconditionsEditor).getByLabelText(
        '前置拦截 1 字段值 argument_path'
      ),
      {
        target: { value: 'media' }
      }
    );
    await domEvents.change(
      within(preconditionsEditor).getByLabelText(
        '前置拦截 1 字段值 media_kind'
      ),
      {
        target: { value: 'image' }
      }
    );
    await domEvents.click(
      within(dialog).getByRole('button', { name: '保存工具' })
    );

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

    await renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await domEvents.click(
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

    await domEvents.mouseDown(dragHandle, {
      button: 0,
      clientX: 24,
      clientY: 24
    });
    await domEvents.mouseMove(window, {
      clientX: window.innerWidth + 200,
      clientY: 24
    });
    await domEvents.mouseUp(window);

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

    await domEvents.mouseDown(dragHandle, {
      button: 0,
      clientX: window.innerWidth - margin - 24,
      clientY: 24
    });
    await domEvents.mouseMove(window, {
      clientX: window.innerWidth - margin - 24,
      clientY: window.innerHeight + 200
    });
    await domEvents.mouseUp(window);

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
});
