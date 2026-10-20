import { describe, test, expect, vi } from 'vitest';
import {
  NamedBindingsFocusHarness,
  dispatch,
  domEvents,
  nodeSchemaAdapterApi,
  nodeSchemaRegistry
} from './node-inspector.fixtures';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SLOW_NODE_INSPECTOR_TEST_TIMEOUT,
  SelectionSeed,
  createInitialState,
  createInitialStateWithCodeNode,
  createInitialStateWithHttpRequestNode,
  renderWithProviders
} from './support';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { NodeInspector } from '../../components/inspector/NodeInspector';
import { TemplatedNamedBindingsField } from '../../components/bindings/TemplatedNamedBindingsField';
import { screen, waitFor, within } from '@testing-library/react';
import { validateDocument } from '../../lib/validate-document';

describe('NodeInspector core', () => {
  test('collapses generated outputs by default and keeps output contract editing hidden', async () => {
    await renderWithProviders(
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

    await domEvents.click(screen.getByRole('button', { name: '输出变量' }));

    expect(screen.getByText('text')).toBeInTheDocument();
    expect(screen.getByText('usage')).toBeInTheDocument();
  });

  test('renders field validation errors under the owning inspector field', async () => {
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

    await renderWithProviders(
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

  test(
    'renders HTTP Request config panel and imports a basic curl command',
    async () => {
      const state = createInitialStateWithHttpRequestNode();
      let latestDocument = state.draft.document;

      await renderWithProviders(
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
      await domEvents.click(storeResponseAsFileSwitch);
      await waitFor(() => {
        const httpNode = latestDocument.graph.nodes.find(
          (node) => node.id === 'node-http-request'
        );

        expect(httpNode?.config.store_response_as_file).toBe(true);
      });
      expect(screen.getByLabelText('超时设置(ms)')).toBeInTheDocument();
      const maxResponseSizeInput = screen.getByLabelText('最大响应体(MB)');
      expect(maxResponseSizeInput).toHaveValue('6');
      await domEvents.change(maxResponseSizeInput, { target: { value: '8' } });
      await waitFor(() => {
        const httpNode = latestDocument.graph.nodes.find(
          (node) => node.id === 'node-http-request'
        );

        expect(httpNode?.config.max_response_bytes).toBe(8 * 1024 * 1024);
      });
      expect(screen.queryByText('status_code')).not.toBeInTheDocument();
      expect(screen.queryByText('headers')).not.toBeInTheDocument();
      expect(screen.queryByText('files')).not.toBeInTheDocument();

      await domEvents.click(screen.getByRole('button', { name: '输出变量' }));

      expect(screen.getAllByText('body').length).toBeGreaterThan(0);
      expect(screen.getByText('status_code')).toBeInTheDocument();
      expect(screen.getByText('headers')).toBeInTheDocument();
      expect(screen.getByText('files')).toBeInTheDocument();

      await domEvents.click(screen.getByRole('button', { name: '导入 cURL' }));
      await domEvents.change(await screen.findByLabelText('cURL 命令'), {
        target: {
          value:
            "curl -X POST 'https://api.example.com/orders?page=1' -H 'Authorization: Bearer token' -H 'Content-Type: application/json' -d '{\"query\":\"{{node-start.query}}\"}'"
        }
      });
      await domEvents.click(screen.getByRole('button', { name: '导入请求' }));

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
    },
    SLOW_NODE_INSPECTOR_TEST_TIMEOUT
  );

  test('renders repeated HTTP Request sections without duplicate React keys', async () => {
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);

    await renderWithProviders(
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
    await renderWithProviders(
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

  test('keeps Code boolean input selector values visible in the single value column', async () => {
    await renderWithProviders(
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

  test('adds Code input rows without preselecting a parameter type', async () => {
    const handleChange = vi.fn();

    await renderWithProviders(
      <TemplatedNamedBindingsField
        ariaLabel="inputs"
        options={[]}
        value={[]}
        onChange={handleChange}
      />
    );

    await domEvents.click(screen.getByRole('button', { name: '新增变量' }));

    expect(handleChange).toHaveBeenCalledWith([
      {
        name: '',
        value: { kind: 'constant', value: '' }
      }
    ]);
  });

  test('keeps named binding input focused when editing its name', async () => {
    await renderWithProviders(<NamedBindingsFocusHarness />);

    const nameInput = screen.getByLabelText('bindings-0-name');
    await dispatch(() => nameInput.focus());
    await domEvents.change(nameInput, {
      target: { value: 'arg2' }
    });

    expect(screen.getByLabelText('bindings-0-name')).toHaveFocus();
  });
});
