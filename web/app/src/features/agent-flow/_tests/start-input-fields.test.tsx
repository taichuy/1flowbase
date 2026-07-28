import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import copyToClipboard from 'copy-to-clipboard';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { writeLocalePreferenceToStorage } from '../../../shared/user-preferences/locale-preference';

import { NodeConfigTab } from '../components/detail/tabs/NodeConfigTab';
import { StartInputFieldsField } from '../components/detail/fields/StartInputFieldsField';
import { AgentFlowEditorStoreProvider } from '../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../store/editor/provider';
import { selectWorkingDocument } from '../store/editor/selectors';

vi.mock('copy-to-clipboard', () => ({
  default: vi.fn(() => true)
}));

function createInitialState() {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-26T10:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

function renderWithProviders(ui: ReactNode) {
  return render(<AppProviders>{ui}</AppProviders>);
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

function escapedPattern(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function expectSystemVariableType(
  key: string,
  title: string,
  valueType: string
) {
  const row = screen.getByTestId(`start-system-variable-${key}`);

  expect(within(row).getByText(title)).toBeInTheDocument();
  expect(
    within(row).getByText(new RegExp(`^${escapedPattern(valueType)}$`, 'i'))
  ).toBeInTheDocument();
}

describe('start input fields', () => {
  beforeEach(async () => {
    window.history.replaceState(null, '', '/?language=zh-Hans');
    writeLocalePreferenceToStorage('zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    vi.clearAllMocks();
  });

  test('edits start input fields and keeps system variables readonly', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-start" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      await screen.findByTestId('inspector-field-config.input_fields')
    ).toBeInTheDocument();
    expect(screen.getByText('userinput.query')).toBeInTheDocument();
    expect(screen.getByText('userinput.system')).toBeInTheDocument();
    expect(screen.getByText('userinput.model')).toBeInTheDocument();
    expect(screen.getByText('userinput.reasoning_effort')).toBeInTheDocument();
    expect(screen.getByText('userinput.history')).toBeInTheDocument();
    expect(screen.getByText('userinput.files')).toBeInTheDocument();
    expect(screen.getByText('userinput.tools')).toBeInTheDocument();
    expect(screen.getByText('userinput.tool_choice')).toBeInTheDocument();
    expectSystemVariableType('history', 'userinput.history', 'array');
    expectSystemVariableType('files', 'userinput.files', 'array[object]');
    expectSystemVariableType('tools', 'userinput.tools', 'array[object]');
    expectSystemVariableType('tool_choice', 'userinput.tool_choice', 'json');
    expect(screen.queryByText('上一轮用户消息')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /userinput\.history/ }));

    expect(screen.getByText(/上一轮用户消息/)).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: '复制userinput.history JSON' })
    );
    expect(copyToClipboard).toHaveBeenCalledWith(
      JSON.stringify(
        [
          {
            role: 'user',
            content: '上一轮用户消息'
          },
          {
            role: 'assistant',
            content: '上一轮助手回复'
          }
        ],
        null,
        2
      )
    );

    fireEvent.click(screen.getByRole('button', { name: /userinput\.files/ }));

    expect(screen.getByText(/example\.pdf/)).toBeInTheDocument();
    expect(screen.getByText(/files\.example\.com/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));

    expect(
      await screen.findByRole('dialog', { name: '新增输入字段' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '从左侧调整新增输入字段宽度' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '从右侧调整新增输入字段宽度' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '向下调整新增输入字段高度' })
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('输入字段占位提示')).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('输入字段变量名'), {
      target: { value: 'customer_name' }
    });
    fireEvent.change(screen.getByLabelText('输入字段显示名'), {
      target: { value: '客户姓名' }
    });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '输入字段类型' }));
    fireEvent.click(await screen.findByTitle('文件列表'));
    fireEvent.click(screen.getByRole('button', { name: '保存输入字段' }));

    const startNode = latestDocument.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    expect(startNode?.config.input_fields).toEqual([
      expect.objectContaining({
        key: 'customer_name',
        label: '客户姓名',
        inputType: 'file_list',
        valueType: 'array[object]'
      })
    ]);
  }, 10000);

  test('opens the start input field panel with an initial height and supports dragging down', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-start" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await screen.findByTestId('inspector-field-config.input_fields');
    fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));

    const dialog = await screen.findByRole('dialog', { name: '新增输入字段' });
    const bottomResizeHandle = screen.getByRole('separator', {
      name: '向下调整新增输入字段高度'
    });

    expect(dialog).toHaveStyle({ height: '520px' });

    fireEvent.mouseDown(bottomResizeHandle, {
      clientX: 420,
      clientY: 520
    });
    fireEvent.mouseMove(window, {
      clientX: 420,
      clientY: 600
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({ height: '600px' });
    });
  });

  test('configures rich start input field options in the shared floating shell', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-start" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await screen.findByTestId('inspector-field-config.input_fields');
    fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));

    expect(
      await screen.findByRole('dialog', { name: '新增输入字段' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '从左侧调整新增输入字段宽度' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '从右侧调整新增输入字段宽度' })
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('输入字段变量名'), {
      target: { value: 'priority' }
    });
    fireEvent.change(screen.getByLabelText('输入字段显示名'), {
      target: { value: '优先级' }
    });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '输入字段类型' }));
    fireEvent.click(await screen.findByTitle('下拉选项'));

    fireEvent.change(screen.getByLabelText('输入字段选项 1'), {
      target: { value: '高' }
    });
    fireEvent.click(screen.getByRole('button', { name: '新增下拉选项' }));
    fireEvent.change(screen.getByLabelText('输入字段选项 2'), {
      target: { value: '低' }
    });
    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: '输入字段默认值' })
    );
    fireEvent.click(await screen.findByTitle('低'));
    fireEvent.click(screen.getByLabelText('隐藏输入字段'));
    fireEvent.click(screen.getByRole('button', { name: '保存输入字段' }));

    const startNode = latestDocument.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    expect(startNode?.config.input_fields).toEqual([
      expect.objectContaining({
        key: 'priority',
        label: '优先级',
        inputType: 'select',
        valueType: 'string',
        options: ['高', '低'],
        defaultValue: '低',
        hidden: true
      })
    ]);
  });

  test('drags and removes start input fields from single-line variable rows', async () => {
    const initialState = createInitialState();

    initialState.draft.document.graph.nodes =
      initialState.draft.document.graph.nodes.map((node) =>
        node.id === 'node-start'
          ? {
              ...node,
              config: {
                ...node.config,
                input_fields: [
                  {
                    key: 'first_name',
                    label: '名字',
                    inputType: 'text',
                    valueType: 'string',
                    required: true
                  },
                  {
                    key: 'age',
                    label: '年龄',
                    inputType: 'number',
                    valueType: 'number',
                    required: false
                  }
                ]
              }
            }
          : node
      );
    let latestDocument = initialState.draft.document;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-start" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      screen.queryByRole('button', { name: '下移输入字段 first_name' })
    ).not.toBeInTheDocument();

    const firstRow = await screen.findByTestId(
      'start-input-field-row-first_name'
    );
    const secondRow = screen.getByTestId('start-input-field-row-age');

    expect(
      within(firstRow).getByText('userinput.first_name')
    ).toBeInTheDocument();
    expect(within(firstRow).getByText('String')).toBeInTheDocument();
    expect(
      within(firstRow).getByRole('button', {
        name: '拖拽排序输入字段 first_name'
      })
    ).toBeInTheDocument();

    fireEvent.dragStart(
      within(firstRow).getByRole('button', {
        name: '拖拽排序输入字段 first_name'
      })
    );
    fireEvent.dragOver(secondRow);
    fireEvent.drop(secondRow);
    fireEvent.click(screen.getByRole('button', { name: '删除输入字段 age' }));

    const startNode = latestDocument.graph.nodes.find(
      (node) => node.id === 'node-start'
    );

    expect(startNode?.config.input_fields).toEqual([
      expect.objectContaining({ key: 'first_name' })
    ]);
  });

  test('AC-1237 shows parameter source only when source options are provided', async () => {
    const onWorkflowChange = vi.fn();
    const { rerender } = renderWithProviders(
      <StartInputFieldsField
        contractKind="workflow_http"
        value={[]}
        sourceOptions={[
          { value: 'path', label: 'path' },
          { value: 'query', label: 'query' },
          { value: 'body', label: 'body' },
          { value: 'form', label: 'form' }
        ]}
        onChange={onWorkflowChange}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));

    expect(
      await screen.findByRole('combobox', { name: '输入字段参数来源' })
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '保存输入字段' }));
    expect(onWorkflowChange).toHaveBeenCalledWith([
      expect.objectContaining({ source: 'path' })
    ]);

    rerender(
      <AppProviders>
        <StartInputFieldsField
          contractKind="agent"
          value={[]}
          onChange={vi.fn()}
        />
      </AppProviders>
    );
    fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));

    expect(
      screen.queryByRole('combobox', { name: '输入字段参数来源' })
    ).not.toBeInTheDocument();
  });
});
