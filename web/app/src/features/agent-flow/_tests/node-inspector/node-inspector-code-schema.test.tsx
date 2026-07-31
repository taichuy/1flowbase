import { readFileSync } from 'node:fs';

import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SelectionSeed,
  createInitialStateWithCustomCodeNode,
  createInitialStateWithStructuredCodeNode,
  getCodeNode,
  openSelect,
  renderWithProviders,
  selectOption,
  setupNodeInspectorTest
} from './support';

beforeEach(setupNodeInspectorTest);

describe('NodeInspector code schema', () => {
  test('renders Code as input variables, JavaScript editor, then output variables and persists edits', async () => {
    const inspectorStyles = readFileSync(
      'src/features/agent-flow/components/editor/styles/inspector.css',
      'utf8'
    );
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    expect(inspectorStyles).toMatch(
      /\.agent-flow-templated-binding-row\s*\{[^}]*grid-template-columns:\s*minmax\(88px,\s*0\.7fr\)\s*minmax\(96px,\s*0\.65fr\)\s*minmax\(\s*168px,\s*1\.5fr\s*\)\s*28px;/su
    );

    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithCustomCodeNode()}
      >
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(screen.queryByText('Advanced')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '运行语言' })
    ).not.toBeInTheDocument();

    const inputField = screen.getByTestId(
      'inspector-field-bindings.named_bindings'
    );
    const sourceField = await screen.findByTestId(
      'inspector-field-config.source'
    );
    const outputField = screen.getByTestId(
      'inspector-field-config.output_contract'
    );
    const outputHeader = within(outputField).getByTestId(
      'agent-flow-collection-field-header'
    );

    expect(inputField.compareDocumentPosition(sourceField)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(sourceField.compareDocumentPosition(outputField)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(
      within(outputHeader).getByRole('heading', { name: '输出变量' })
    ).toBeInTheDocument();
    expect(
      within(outputHeader).getByRole('button', { name: '新增输出变量' })
    ).toBeInTheDocument();
    expect(
      screen.getByLabelText(/输入变量-0-name|input variables-0-name/)
    ).toHaveValue('arg1');
    expect(
      screen.getAllByLabelText(/输入变量-0-type|input variables-0-type/).length
    ).toBeGreaterThan(0);
    expect(
      screen.queryByLabelText(
        /输入变量-0-value-mode|input variables-0-value-mode/
      )
    ).not.toBeInTheDocument();
    expect(
      screen.getAllByLabelText(/输入变量-0-value|input variables-0-value/)
        .length
    ).toBeGreaterThan(0);
    expect(
      screen.queryByLabelText(/输入变量-0-selector|input variables-0-selector/)
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '复制arg1' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '放大编辑arg1' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '删除变量 arg1' })
    ).toBeInTheDocument();
    const codeEditor = await screen.findByLabelText(
      /JavaScript 代码|JavaScript code/
    );

    expect(codeEditor).toHaveValue('return { riskScore: 0.82 };');
    expect(screen.getByLabelText('输出变量名 1')).toHaveValue('riskScore');
    expect(screen.queryByLabelText('输出显示名 1')).not.toBeInTheDocument();

    const inputVariableName = screen.getByLabelText(
      /输入变量-0-name|input variables-0-name/
    );
    inputVariableName.focus();
    fireEvent.change(inputVariableName, {
      target: { value: 'score_1' }
    });
    expect(
      screen.getByLabelText(/输入变量-0-name|input variables-0-name/)
    ).toHaveFocus();
    fireEvent.change(codeEditor, {
      target: { value: 'return { risk_score: inputs.score };' }
    });
    const outputVariableName = screen.getByLabelText('输出变量名 1');
    outputVariableName.focus();
    fireEvent.change(outputVariableName, {
      target: { value: 'risk_score' }
    });
    expect(screen.getByLabelText('输出变量名 1')).toHaveFocus();

    await waitFor(() => {
      expect(getCodeNode(latestDocument).config).toMatchObject({
        language: 'javascript',
        source: 'return { risk_score: inputs.score };'
      });
      expect(getCodeNode(latestDocument).bindings.named_bindings).toEqual({
        kind: 'named_bindings',
        value: [
          {
            name: 'score_1',
            valueType: 'string',
            value: {
              kind: 'templated_text',
              value: '{{sys.conversation_id}}'
            }
          }
        ]
      });
      expect(getCodeNode(latestDocument).outputs).toEqual([
        {
          key: 'risk_score',
          title: 'risk_score',
          valueType: 'number',
          selector: ['result', 'risk_score']
        }
      ]);
    });
  });

  test('renders JSON schema controls for structured code outputs', async () => {
    let latestDocument =
      createInitialStateWithStructuredCodeNode().draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={{
          flow_id: 'flow-1',
          messages: [],
          draft: {
            id: 'draft-1',
            flow_id: 'flow-1',
            updated_at: '2026-04-16T10:00:00Z',
            document: latestDocument
          },
          autosave_interval_seconds: 30,
          user_protection_limit: 10,
          versions: []
        }}
      >
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByLabelText('输出变量名 1')).toHaveValue(
      'chat_history'
    );
    fireEvent.click(screen.getByRole('button', { name: '编辑 JSON Schema' }));

    const dialog = await screen.findByRole('dialog', { name: 'JSON Schema' });
    expect(dialog).toHaveClass('agent-flow-model-settings__panel');
    expect(
      screen.getByRole('tab', { name: 'Schema 字段' })
    ).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'JSON 解析' })).toBeInTheDocument();
    expect(screen.getByLabelText('Schema 字段名 1')).toHaveValue('role');
    expect(screen.getByLabelText('Schema 字段名 2')).toHaveValue('content');

    fireEvent.click(screen.getByRole('button', { name: '添加 Schema 字段' }));
    const schemaFieldNameInput = screen.getByLabelText('Schema 字段名 3');
    schemaFieldNameInput.focus();
    fireEvent.change(schemaFieldNameInput, {
      target: { value: 'metadata' }
    });
    expect(screen.getByLabelText('Schema 字段名 3')).toHaveFocus();
    await openSelect('Schema 字段类型 3');
    await selectOption('Object');
    fireEvent.click(
      screen.getByRole('button', { name: '添加 metadata 子字段' })
    );
    fireEvent.change(screen.getByLabelText('Schema 字段名 3.1'), {
      target: { value: 'source' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(getCodeNode(latestDocument).outputs[0].jsonSchema).toMatchObject({
        type: 'array',
        items: {
          type: 'object',
          required: ['role', 'content', 'metadata'],
          properties: {
            role: { type: 'string' },
            content: { type: 'string' },
            metadata: {
              type: 'object',
              required: ['source'],
              properties: {
                source: { type: 'string' }
              }
            }
          }
        }
      });
      expect(getCodeNode(latestDocument).outputs[0]).toMatchObject({
        key: 'chat_history',
        title: 'chat_history',
        selector: ['result', 'chat_history']
      });
    });
  }, 20_000);

  test('parses JSON Schema from highlighted code mode', async () => {
    let latestDocument =
      createInitialStateWithStructuredCodeNode().draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={{
          flow_id: 'flow-1',
          messages: [],
          draft: {
            id: 'draft-1',
            flow_id: 'flow-1',
            updated_at: '2026-04-16T10:00:00Z',
            document: latestDocument
          },
          autosave_interval_seconds: 30,
          user_protection_limit: 10,
          versions: []
        }}
      >
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByLabelText('输出变量名 1')).toHaveValue(
      'chat_history'
    );
    fireEvent.click(screen.getByRole('button', { name: '编辑 JSON Schema' }));
    fireEvent.click(await screen.findByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(await screen.findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify(
          {
            type: 'object',
            properties: {
              summary: { type: 'string' }
            },
            required: ['summary']
          },
          null,
          2
        )
      }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(getCodeNode(latestDocument).outputs[0]).toMatchObject({
        valueType: 'object',
        jsonSchema: {
          type: 'object',
          properties: {
            summary: { type: 'string' }
          },
          required: ['summary']
        }
      });
    });
  });

  test('keeps parsed JSON Schema root type when returning to field mode', async () => {
    const initialState = createInitialStateWithStructuredCodeNode();
    const codeNode = getCodeNode(initialState.draft.document);
    codeNode.outputs = [
      {
        key: 'chat_history',
        title: 'Chat History',
        valueType: 'object',
        jsonSchema: {
          type: 'object',
          required: [],
          properties: {}
        }
      }
    ];
    let latestDocument = initialState.draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 JSON Schema' })
    );
    fireEvent.click(await screen.findByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(await screen.findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify(
          {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                role: { type: 'string' },
                content: { type: 'string' }
              },
              required: ['role', 'content']
            }
          },
          null,
          2
        )
      }
    });
    fireEvent.click(screen.getByRole('tab', { name: 'Schema 字段' }));

    await waitFor(() => {
      expect(screen.getByLabelText('Schema 字段名 1')).toHaveValue('role');
      expect(screen.getByLabelText('Schema 字段名 2')).toHaveValue('content');
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(getCodeNode(latestDocument).outputs[0]).toMatchObject({
        valueType: 'array',
        jsonSchema: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              role: { type: 'string' },
              content: { type: 'string' }
            },
            required: ['role', 'content']
          }
        }
      });
    });
  });

  test('preserves array item metadata from parsed JSON Schema when returning to field mode', async () => {
    const initialState = createInitialStateWithStructuredCodeNode();
    const codeNode = getCodeNode(initialState.draft.document);
    codeNode.outputs = [
      {
        key: 'tool_arguments',
        title: 'Tool Arguments',
        valueType: 'object',
        jsonSchema: {
          type: 'object',
          required: [],
          properties: {}
        }
      }
    ];
    let latestDocument = initialState.draft.document;
    const mediaToolSchema = {
      type: 'object',
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
            },
            required: ['kind', 'source', 'path']
          }
        }
      },
      required: ['task']
    };

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 JSON Schema' })
    );
    fireEvent.click(await screen.findByRole('tab', { name: 'JSON 解析' }));
    fireEvent.change(await screen.findByLabelText('JSON Schema 内容'), {
      target: {
        value: JSON.stringify(mediaToolSchema, null, 2)
      }
    });
    fireEvent.click(screen.getByRole('tab', { name: 'Schema 字段' }));

    await waitFor(() => {
      expect(screen.getByLabelText('Schema 字段名 1')).toHaveValue('task');
      expect(screen.getByLabelText('Schema 字段名 2')).toHaveValue('media');
      expect(screen.getByLabelText('Schema 字段名 2.1')).toHaveValue('kind');
      expect(screen.getByLabelText('Schema 字段名 2.2')).toHaveValue('source');
      expect(screen.getByLabelText('Schema 字段名 2.3')).toHaveValue('path');
      expect(screen.getByLabelText('Schema 枚举字段名 2.1')).toHaveValue(
        'enum'
      );
      expect(screen.getByLabelText('Schema 枚举字段值 2.1')).toHaveValue(
        '["image"]'
      );
      expect(
        screen.getByRole('combobox', { name: 'Schema 枚举字段类型 2.1' })
      ).toBeInTheDocument();
      expect(screen.getByLabelText('Schema 枚举项 2.1.1')).toHaveValue(
        'enum[1]'
      );
      expect(screen.getByLabelText('Schema 枚举值 2.1.1')).toHaveValue('image');
      expect(screen.getByLabelText('Schema 枚举项 2.2.1')).toHaveValue(
        'enum[1]'
      );
      expect(screen.getByLabelText('Schema 枚举值 2.2.1')).toHaveValue(
        'workspace_path'
      );
    });
    const mediaActions = screen.getByRole('group', {
      name: 'Schema 字段 media 操作'
    });
    const sourceActions = screen.getByRole('group', {
      name: 'Schema 字段 source 操作'
    });

    expect(
      within(mediaActions).getByRole('button', { name: '添加 media 子字段' })
    ).toBeInTheDocument();
    expect(
      within(sourceActions).getByRole('button', { name: '添加 source 枚举值' })
    ).toBeInTheDocument();
    expect(
      within(sourceActions).getByRole('button', {
        name: '删除 Schema 字段 source'
      })
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Schema 枚举值 2.2.1'), {
      target: { value: 'uploaded_file' }
    });
    fireEvent.click(screen.getByRole('button', { name: '添加 source 枚举值' }));
    fireEvent.change(screen.getByLabelText('Schema 枚举值 2.2.2'), {
      target: { value: 'workspace_path' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(getCodeNode(latestDocument).outputs[0]).toMatchObject({
        valueType: 'object',
        jsonSchema: {
          ...mediaToolSchema,
          properties: {
            ...mediaToolSchema.properties,
            media: {
              ...mediaToolSchema.properties.media,
              items: {
                ...mediaToolSchema.properties.media.items,
                properties: {
                  ...mediaToolSchema.properties.media.items.properties,
                  source: {
                    ...mediaToolSchema.properties.media.items.properties.source,
                    enum: ['uploaded_file', 'workspace_path']
                  }
                }
              }
            }
          }
        }
      });
    });
  });

  test('edits scalar array item type and keeps enum values on array items', async () => {
    const initialState = createInitialStateWithStructuredCodeNode();
    const codeNode = getCodeNode(initialState.draft.document);
    codeNode.outputs = [
      {
        key: 'tool_arguments',
        title: 'Tool Arguments',
        valueType: 'object',
        jsonSchema: {
          type: 'object',
          required: ['tags'],
          properties: {
            tags: {
              type: 'array',
              description: '标签',
              items: {
                type: 'string',
                enum: ['image']
              }
            }
          }
        }
      }
    ];
    let latestDocument = initialState.draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-code" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: '编辑 JSON Schema' })
    );

    expect(await screen.findByLabelText('Schema 字段名 1')).toHaveValue('tags');
    expect(screen.getByLabelText('Schema 枚举字段名 1')).toHaveValue('enum');
    expect(screen.getByLabelText('Schema 枚举字段值 1')).toHaveValue(
      '["image"]'
    );
    expect(screen.getByLabelText('Schema 枚举值 1.1')).toHaveValue('image');
    expect(
      screen.getByRole('combobox', { name: 'Schema 枚举类型 1.1' })
    ).toBeInTheDocument();

    await openSelect('Schema 字段类型 1');
    await selectOption('Array<Number>');
    fireEvent.change(screen.getByLabelText('Schema 枚举值 1.1'), {
      target: { value: '7' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => {
      expect(getCodeNode(latestDocument).outputs[0]).toMatchObject({
        valueType: 'object',
        jsonSchema: {
          type: 'object',
          required: ['tags'],
          properties: {
            tags: {
              type: 'array',
              description: '标签',
              items: {
                type: 'number',
                enum: [7]
              }
            }
          }
        }
      });
    });
  });
});
