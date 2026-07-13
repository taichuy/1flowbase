/* eslint-disable testing-library/no-node-access */
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  createDefaultWorkflowDocument
} from '@1flowbase/flow-schema';
import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import {
  modelProviderOptionsContract,
  modelProviderOptionsProviders
} from '../../../test/model-provider-contract-fixtures';

import { NodeConfigTab } from '../components/detail/tabs/NodeConfigTab';
import { NodeDetailPanel } from '../components/detail/NodeDetailPanel';
import '../../workflow/register';
import * as modelProviderOptionsApi from '../api/model-provider-options';
import { AgentFlowEditorStoreProvider } from '../store/editor/AgentFlowEditorStoreProvider';
import * as nodeSchemaAdapterApi from '../schema/node-schema-adapter';
import * as nodeSchemaRegistry from '../schema/node-schema-registry';
import { useAgentFlowEditorStore } from '../store/editor/provider';
import { selectWorkingDocument } from '../store/editor/selectors';

const NODE_DETAIL_PANEL_TEST_TIMEOUT = 15_000;
const primaryProviderOption = modelProviderOptionsProviders[0];
const primaryProviderFirstGroup = primaryProviderOption.model_groups[0];
const primaryProviderFirstModel = primaryProviderFirstGroup.model;
const primaryProviderSecondGroup = primaryProviderOption.model_groups[1];
const primaryProviderSecondModel = primaryProviderSecondGroup.model;
const secondaryProviderOption = modelProviderOptionsProviders[1];
const secondaryProviderFirstGroup = secondaryProviderOption.model_groups[0];
const secondaryProviderFirstModel = secondaryProviderFirstGroup.model;
const fetchModelProviderOptionsSpy = vi.spyOn(
  modelProviderOptionsApi,
  'fetchModelProviderOptions'
);
const resolveAgentFlowNodeSchemaSpy = vi.spyOn(
  nodeSchemaRegistry,
  'resolveAgentFlowNodeSchema'
);
const createAgentFlowNodeSchemaAdapterSpy = vi.spyOn(
  nodeSchemaAdapterApi,
  'createAgentFlowNodeSchemaAdapter'
);

function createInitialState() {
  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

function createWorkflowInitialState() {
  const document = createDefaultWorkflowDocument({ flowId: 'flow-1' });
  const startNode = document.graph.nodes.find(
    (node) => node.id === 'node-workflow-start'
  );

  if (!startNode) {
    throw new Error('expected workflow start node');
  }

  startNode.config.input_fields = [
    {
      key: 'customer_id',
      label: '客户 ID',
      inputType: 'text',
      valueType: 'string',
      required: true,
      hidden: false
    }
  ];

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
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

function renderWithProviders(ui: ReactNode) {
  return render(<AppProviders>{ui}</AppProviders>);
}

function getLlmNodeConfig(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
) {
  const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

  if (!llmNode) {
    throw new Error('expected default LLM node');
  }

  return llmNode.config;
}

async function openModelSettings() {
  fireEvent.click(await screen.findByRole('button', { name: /模型|model/ }));
  expect(
    await screen.findByRole('heading', { name: '模型设置' })
  ).toBeInTheDocument();
}

async function openModelDropdown() {
  fireEvent.mouseDown(
    await screen.findByRole('combobox', { name: '选择供应商和模型' })
  );
}

async function clickModelOption(label: string) {
  const [option] = await screen.findAllByText((content, element) => {
    if (
      !element ||
      !element.matches('.agent-flow-model-settings__option-main')
    ) {
      return false;
    }

    return content.trim() === label;
  });

  fireEvent.click(option.closest('button') as HTMLButtonElement);
}

describe('NodeDetailPanel', () => {
  beforeEach(async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    fetchModelProviderOptionsSpy.mockReset();
    fetchModelProviderOptionsSpy.mockResolvedValue({
      locale_meta: {
        requested_locale: 'zh_Hans',
        resolved_locale: 'zh_Hans',
        fallback_locale: 'en_US',
        supported_locales: ['zh_Hans', 'en_US']
      },
      i18n_catalog: {},
      providers: []
    });
    resolveAgentFlowNodeSchemaSpy.mockClear();
    createAgentFlowNodeSchemaAdapterSpy.mockClear();
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: null
    });
  });

  test(
    'builds node detail from the schema registry and node schema adapter',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            onClose={vi.fn()}
            onRunNode={undefined}
          />
        </AgentFlowEditorStoreProvider>
      );

      expect(resolveAgentFlowNodeSchemaSpy).toHaveBeenCalledWith('llm');
      expect(createAgentFlowNodeSchemaAdapterSpy).toHaveBeenCalledTimes(1);
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'renders header, config tab and last-run tab for the selected node',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            onClose={vi.fn()}
            onRunNode={undefined}
          />
        </AgentFlowEditorStoreProvider>
      );

      expect(screen.getByRole('tab', { name: /设置|配置/ })).toHaveAttribute(
        'aria-selected',
        'true'
      );
      expect(screen.getByRole('tab', { name: '上次运行' })).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: '关闭节点详情' })
      ).toBeInTheDocument();
      expect(screen.getByLabelText('节点别名')).toHaveValue('LLM');
      expect(screen.getByTestId('node-detail-body')).toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'keeps run loading state from leaking into the debug action',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            onClose={vi.fn()}
            onDebugNode={vi.fn()}
            onRunNode={vi.fn()}
            runLoading
          />
        </AgentFlowEditorStoreProvider>
      );

      const header = screen.getByTestId('node-detail-header');
      const runButton = within(header).getByRole('button', {
        name: '运行当前节点'
      });
      const debugButton = within(header).getByRole('button', {
        name: '调试当前节点'
      });

      expect(runButton).toHaveClass('ant-btn-loading');
      expect(debugButton).not.toHaveClass('ant-btn-loading');
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'keeps debug loading state from leaking into the run action',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            debugLoading
            onClose={vi.fn()}
            onDebugNode={vi.fn()}
            onRunNode={vi.fn()}
          />
        </AgentFlowEditorStoreProvider>
      );

      const header = screen.getByTestId('node-detail-header');
      const runButton = within(header).getByRole('button', {
        name: '运行当前节点'
      });
      const debugButton = within(header).getByRole('button', {
        name: '调试当前节点'
      });

      expect(debugButton).toHaveClass('ant-btn-loading');
      expect(runButton).not.toHaveClass('ant-btn-loading');
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'uses the same node type icon in detail header as the canvas card',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            onClose={vi.fn()}
            onRunNode={undefined}
          />
        </AgentFlowEditorStoreProvider>
      );

      const header = screen.getByTestId('node-detail-header');

      expect(
        within(header).getByRole('img', { name: 'thunderbolt' })
      ).toBeInTheDocument();
      expect(
        within(header).queryByRole('img', { name: 'home' })
      ).not.toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'renders alias and description editors inside the header exactly once',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeDetailPanel
            activeTab="config"
            onTabChange={vi.fn()}
            onClose={vi.fn()}
            onRunNode={undefined}
          />
        </AgentFlowEditorStoreProvider>
      );

      const header = screen.getByTestId('node-detail-header');

      expect(within(header).getByLabelText('节点别名')).toHaveValue('LLM');
      expect(within(header).getByLabelText('节点简介')).toHaveValue('');
      expect(screen.getAllByLabelText('节点别名')).toHaveLength(1);
      expect(screen.getAllByLabelText('节点简介')).toHaveLength(1);
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'keeps config tab focused on editable settings and relations without redundant summary cards',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      expect(screen.queryByText('节点说明')).not.toBeInTheDocument();
      expect(screen.queryByText('帮助文档')).not.toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: /模型|model/ })
      ).toBeInTheDocument();
      expect(screen.queryByText('LLM 参数')).not.toBeInTheDocument();
      expect(screen.queryByText('返回格式')).not.toBeInTheDocument();
      expect(screen.queryByText('输出契约')).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', { name: '添加下一个节点' })
      ).not.toBeInTheDocument();
      expect(screen.getAllByText('添加并行节点')).toHaveLength(1);
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'does not duplicate identity or summary content inside config tab',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      expect(screen.queryByText('节点说明')).not.toBeInTheDocument();
      expect(screen.queryByText('节点别名')).not.toBeInTheDocument();
      expect(screen.queryByText('节点简介')).not.toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'hides retry and exception policy controls for the start node',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-start" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      expect(
        screen.queryByRole('switch', { name: '失败重试' })
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole('combobox', { name: '异常处理' })
      ).not.toBeInTheDocument();
      expect(screen.queryByText('策略')).not.toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'renders the standard node shell with only workflow start input settings',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider
          initialState={createWorkflowInitialState()}
        >
          <SelectionSeed nodeId="node-workflow-start" />
          <NodeDetailPanel
            activeTab="config"
            onClose={vi.fn()}
            onTabChange={vi.fn()}
            workflowTriggerContext={{
              applicationId: 'app-workflow',
              triggerType: 'extension',
              mapping: null,
              schedule: null
            }}
          />
        </AgentFlowEditorStoreProvider>
      );

      expect(screen.getByTestId('node-detail-header')).toBeInTheDocument();
      expect(
        screen.getByTestId('inspector-field-config.input_fields')
      ).toBeInTheDocument();
      expect(screen.getByText('input.customer_id')).toBeInTheDocument();
      expect(
        screen.queryByText('userinput.customer_id')
      ).not.toBeInTheDocument();
      expect(screen.queryByText('userinput.history')).not.toBeInTheDocument();
      expect(screen.queryByText('系统变量')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('接口 slug')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('定时表达式')).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));
      expect(
        screen.getByRole('combobox', { name: '输入字段参数来源' })
      ).toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'does not expose extension mapping actions in workflow start details',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider
          initialState={createWorkflowInitialState()}
        >
          <SelectionSeed nodeId="node-workflow-start" />
          <NodeDetailPanel
            activeTab="config"
            onClose={vi.fn()}
            onTabChange={vi.fn()}
            workflowTriggerContext={{
              applicationId: 'app-workflow',
              triggerType: 'extension',
              mapping: null,
              schedule: null
            }}
          />
        </AgentFlowEditorStoreProvider>
      );

      expect(screen.queryByText('触发器配置')).not.toBeInTheDocument();
      expect(screen.queryByText('目标 selector')).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', { name: '保存修改' })
      ).not.toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'keeps schedule context outside workflow start node details',
    () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider
          initialState={createWorkflowInitialState()}
        >
          <SelectionSeed nodeId="node-workflow-start" />
          <NodeDetailPanel
            activeTab="config"
            onClose={vi.fn()}
            onTabChange={vi.fn()}
            workflowTriggerContext={{
              applicationId: 'app-workflow',
              triggerType: 'schedule',
              mapping: null,
              schedule: null
            }}
          />
        </AgentFlowEditorStoreProvider>
      );

      expect(
        screen.getByTestId('inspector-field-config.input_fields')
      ).toBeInTheDocument();
      expect(screen.queryByLabelText('定时表达式')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('时区')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('输入 payload')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('接口 slug')).not.toBeInTheDocument();

      fireEvent.click(screen.getByRole('button', { name: '新增输入字段' }));
      expect(
        screen.queryByRole('combobox', { name: '输入字段参数来源' })
      ).not.toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'renders exception handling as a three-state strategy selector',
    async () => {
      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      const policyRows = screen.getAllByTestId('node-policy-row');
      expect(policyRows).toHaveLength(3);
      expect(policyRows[0]).toHaveTextContent('推理强度');
      expect(policyRows[1]).toHaveTextContent('失败重试');
      expect(screen.getByLabelText('使用外部传入推理强度')).toBeInTheDocument();
      expect(
        screen.getByRole('switch', { name: '失败重试' })
      ).toBeInTheDocument();
      expect(screen.getByTestId('node-policy-error')).toHaveTextContent('无');
      expect(screen.getByTestId('node-policy-error')).toHaveClass(
        'agent-flow-node-detail__policy-select-shell--compact'
      );

      fireEvent.mouseDown(screen.getByRole('combobox', { name: '异常处理' }));

      expect(
        await screen.findByText('当发生异常且未处理时，节点将停止运行')
      ).toBeInTheDocument();
      expect(
        screen.getByText('当发生异常时，指定默认输出内容。')
      ).toBeInTheDocument();
      expect(
        screen.getByText('当发生异常时，将执行异常分支')
      ).toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'writes the selected exception handling strategy back to the node document',
    async () => {
      let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <DocumentObserver
            onChange={(document) => {
              latestDocument = document;
            }}
          />
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      fireEvent.mouseDown(screen.getByRole('combobox', { name: '异常处理' }));
      fireEvent.click(await screen.findByText('默认值'));

      await waitFor(() => {
        expect(screen.getByTestId('node-policy-error')).toHaveTextContent(
          '默认值'
        );
      });
      expect(latestDocument.graph.nodes).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            id: 'node-llm',
            config: expect.objectContaining({
              error_policy: 'default_value'
            })
          })
        ])
      );
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'opens a default output form when selecting the default value exception strategy',
    async () => {
      let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <DocumentObserver
            onChange={(document) => {
              latestDocument = document;
            }}
          />
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      fireEvent.mouseDown(screen.getByRole('combobox', { name: '异常处理' }));
      fireEvent.click(await screen.findByText('默认值'));

      expect(
        await screen.findByRole('dialog', { name: '默认输出' })
      ).toBeInTheDocument();

      fireEvent.change(screen.getByRole('textbox', { name: '默认输出' }), {
        target: { value: '{"text":"兜底回复"}' }
      });
      fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

      await waitFor(() => {
        expect(getLlmNodeConfig(latestDocument)).toEqual(
          expect.objectContaining({
            error_policy: 'default_value',
            error_default_output: {
              text: '兜底回复'
            }
          })
        );
      });
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'writes the reasoning effort strategy from the policy group',
    async () => {
      let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <DocumentObserver
            onChange={(document) => {
              latestDocument = document;
            }}
          />
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      fireEvent.click(screen.getByRole('switch', { name: '推理强度' }));

      expect(latestDocument.graph.nodes).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            id: 'node-llm',
            config: expect.objectContaining({
              external_reasoning_policy: {
                follow_external_reasoning: true
              }
            })
          })
        ])
      );
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'keeps llm_parameters when switching models within the same provider',
    async () => {
      let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
      const state = createInitialState();
      const llmNodeConfig = getLlmNodeConfig(state.draft.document);

      llmNodeConfig.model_provider = {
        provider_code: primaryProviderOption.provider_code,
        model_id: primaryProviderFirstModel.model_id,
        provider_label: primaryProviderOption.display_name,
        model_label: primaryProviderFirstModel.display_name
      };
      llmNodeConfig.llm_parameters = {
        schema_version: '1.0.0',
        items: {
          temperature: {
            enabled: true,
            value: 0.42
          }
        }
      };
      fetchModelProviderOptionsSpy.mockResolvedValueOnce(
        modelProviderOptionsContract
      );

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={state}>
          <DocumentObserver
            onChange={(document) => {
              latestDocument = document;
            }}
          />
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      await openModelSettings();
      await openModelDropdown();
      await clickModelOption(primaryProviderSecondModel.display_name);

      await waitFor(() => {
        expect(getLlmNodeConfig(latestDocument)).toMatchObject({
          model_provider: {
            provider_code: primaryProviderOption.provider_code,
            model_id: primaryProviderSecondModel.model_id
          },
          llm_parameters: {
            schema_version: '1.0.0',
            items: {
              temperature: {
                enabled: true,
                value: 0.42
              }
            }
          }
        });
      });
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'reinitializes llm_parameters when switching to a different provider',
    async () => {
      let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
      const duplicatedContract = JSON.parse(
        JSON.stringify(modelProviderOptionsContract)
      ) as typeof modelProviderOptionsContract;
      const duplicatedSecondaryProvider = duplicatedContract.providers[1];
      const duplicatedSecondaryModel =
        duplicatedSecondaryProvider.model_groups[0].model;
      const state = createInitialState();
      const llmNodeConfig = getLlmNodeConfig(state.draft.document);

      duplicatedSecondaryProvider.parameter_form = {
        schema_version: '1.0.0',
        fields: [
          {
            key: 'top_p',
            label: 'Top P',
            type: 'number',
            send_mode: 'optional',
            enabled_by_default: true,
            options: [],
            visible_when: [],
            disabled_when: [],
            default_value: 0.9
          }
        ]
      };
      llmNodeConfig.model_provider = {
        provider_code: primaryProviderOption.provider_code,
        model_id: primaryProviderFirstModel.model_id,
        provider_label: primaryProviderOption.display_name,
        model_label: primaryProviderFirstModel.display_name
      };
      llmNodeConfig.llm_parameters = {
        schema_version: '1.0.0',
        items: {
          temperature: {
            enabled: true,
            value: 0.42
          }
        }
      };
      fetchModelProviderOptionsSpy.mockResolvedValueOnce(duplicatedContract);

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={state}>
          <DocumentObserver
            onChange={(document) => {
              latestDocument = document;
            }}
          />
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      await openModelSettings();
      await openModelDropdown();
      await clickModelOption(duplicatedSecondaryModel.display_name);

      await waitFor(() => {
        expect(getLlmNodeConfig(latestDocument)).toMatchObject({
          model_provider: {
            provider_code: duplicatedSecondaryProvider.provider_code,
            model_id: duplicatedSecondaryModel.model_id
          },
          llm_parameters: {
            schema_version: '1.0.0',
            items: {
              top_p: {
                enabled: true,
                value: 0.9
              }
            }
          }
        });
      });
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );

  test(
    'renders the empty-state copy when the selected provider has no parameter schema',
    async () => {
      const state = createInitialState();
      const llmNodeConfig = getLlmNodeConfig(state.draft.document);

      llmNodeConfig.model_provider = {
        provider_code: secondaryProviderOption.provider_code,
        model_id: secondaryProviderFirstModel.model_id,
        provider_label: secondaryProviderOption.display_name,
        model_label: secondaryProviderFirstModel.display_name
      };
      fetchModelProviderOptionsSpy.mockResolvedValueOnce(
        modelProviderOptionsContract
      );

      renderWithProviders(
        <AgentFlowEditorStoreProvider initialState={state}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      );

      await openModelSettings();

      expect(
        await screen.findByText('当前供应商没有可调参数。')
      ).toBeInTheDocument();
    },
    NODE_DETAIL_PANEL_TEST_TIMEOUT
  );
});
