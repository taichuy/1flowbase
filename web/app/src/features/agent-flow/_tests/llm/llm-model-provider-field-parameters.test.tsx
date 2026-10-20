import { describe, test, expect } from 'vitest';
import {
  DocumentObserver,
  SelectionSeed,
  clickModelOption,
  createInitialState,
  fetchModelProviderOptionsSpy,
  mockElementRect,
  openModelDropdown,
  openModelSettings,
  primaryProviderFirstModel,
  renderWithProviders,
  secondaryProviderFirstGroup,
  secondaryProviderFirstModel,
  secondaryProviderOption
} from './llm-model-provider-field.fixtures';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { modelProviderOptionsContract } from '../../../../test/model-provider-contract-fixtures';

describe('LlmModelField', () => {
  test('renders provider-level parameter controls inside the model dialog instead of the inspector body', async () => {
    const duplicatedModelContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;
    const duplicatedProvider = duplicatedModelContract.providers[0];

    duplicatedProvider.parameter_form = {
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

    duplicatedProvider.model_groups = [
      {
        model_id: 'gpt-4o-mini',
        pricing_configured: false,
        distribution_rule: 'none',
        model: {
          ...primaryProviderFirstModel,
          model_id: 'gpt-4o-mini',
          display_name: 'GPT-4o Mini'
        },
        targets: [
          {
            source_instance_id: 'provider-openai-prod',
            source_instance_display_name: 'OpenAI Production',
            routing_enabled: true,
            model: {
              ...primaryProviderFirstModel,
              model_id: 'gpt-4o-mini',
              display_name: 'GPT-4o Mini'
            }
          },
          {
            source_instance_id: 'provider-openai-backup',
            source_instance_display_name: 'OpenAI Backup',
            routing_enabled: true,
            model: {
              ...primaryProviderFirstModel,
              model_id: 'gpt-4o-mini',
              display_name: 'GPT-4o Mini'
            }
          }
        ]
      }
    ];
    fetchModelProviderOptionsSpy.mockResolvedValueOnce(duplicatedModelContract);

    const state = createInitialState();
    const llmNode = state.draft.document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: duplicatedProvider.provider_code,
      model_id: 'gpt-4o-mini',
      provider_label: duplicatedProvider.display_name,
      model_label: 'GPT-4o Mini'
    };

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();
    expect(await screen.findByText('Top P')).toBeInTheDocument();
    expect(screen.queryByText('Temperature')).not.toBeInTheDocument();
    expect(screen.queryByText('返回格式')).not.toBeInTheDocument();
  });

  test('renders extended openai-compatible parameters with inline row sections in the model dialog', async () => {
    const extendedContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;
    const extendedProvider = extendedContract.providers[0];

    extendedProvider.parameter_form = {
      schema_version: '1.0.0',
      fields: [
        {
          key: 'temperature',
          label: 'Temperature',
          type: 'number',
          control: 'slider',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [],
          visible_when: [],
          disabled_when: [],
          default_value: 0.7,
          min: 0,
          max: 2,
          step: 0.1
        },
        {
          key: 'presence_penalty',
          label: 'Presence Penalty',
          type: 'number',
          control: 'slider',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [],
          visible_when: [],
          disabled_when: [],
          default_value: 0,
          min: -2,
          max: 2,
          step: 0.1
        },
        {
          key: 'stop',
          label: 'Stop',
          type: 'string',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [],
          visible_when: [],
          disabled_when: [],
          default_value: ''
        },
        {
          key: 'user',
          label: 'User',
          type: 'string',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [],
          visible_when: [],
          disabled_when: [],
          default_value: ''
        }
      ]
    };
    fetchModelProviderOptionsSpy.mockResolvedValueOnce(extendedContract);

    const state = createInitialState();
    const llmNode = state.draft.document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: extendedProvider.provider_code,
      model_id: primaryProviderFirstModel.model_id,
      provider_label: extendedProvider.display_name,
      model_label: primaryProviderFirstModel.display_name
    };

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();
    expect(await screen.findByText('Presence Penalty')).toBeInTheDocument();
    expect(screen.getByText('Stop')).toBeInTheDocument();
    expect(screen.getByText('User')).toBeInTheDocument();

    const temperatureRow = screen
      .getByText('Temperature')
      .closest('.agent-flow-llm-parameter-form__row');
    const stopRow = screen
      .getByText('Stop')
      .closest('.agent-flow-llm-parameter-form__row');
    const temperatureHead = temperatureRow?.querySelector(
      '.agent-flow-llm-parameter-form__row-head'
    );

    expect(temperatureRow).not.toBeNull();
    expect(temperatureHead).not.toBeNull();
    expect(
      temperatureHead?.querySelector(
        '.agent-flow-llm-parameter-form__row-label'
      )
    ).not.toBeNull();
    expect(
      temperatureRow?.querySelector(
        '.agent-flow-llm-parameter-form__row-control'
      )
    ).not.toBeNull();
    expect(
      temperatureHead?.querySelector(
        '.agent-flow-llm-parameter-form__row-toggle'
      )
    ).not.toBeNull();
    expect(temperatureHead?.nextElementSibling?.classList).toContain(
      'agent-flow-llm-parameter-form__row-control'
    );
    expect(temperatureRow?.querySelector('.ant-slider')).not.toBeNull();
    expect(stopRow?.querySelector('input')).not.toBeNull();
    expect(
      document.body.querySelectorAll(
        '.agent-flow-llm-parameter-form__row-control'
      ).length
    ).toBe(4);
  });

  test('AC-001 renders boolean parameters as one inline switch without transport metadata', async () => {
    const booleanContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;
    const booleanProvider = booleanContract.providers[0];
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    booleanProvider.parameter_form = {
      schema_version: '1.0.0',
      fields: [
        {
          key: 'store',
          label: '存储响应',
          type: 'boolean',
          control: 'switch',
          send_mode: 'always',
          default_value: true,
          options: [],
          visible_when: [],
          disabled_when: []
        },
        {
          key: 'use_responses_websocket',
          label: '使用 Responses WebSocket',
          type: 'boolean',
          control: 'switch',
          send_mode: 'always',
          default_value: false,
          options: [],
          visible_when: [],
          disabled_when: []
        },
        {
          key: 'enable_search',
          label: '启用搜索',
          type: 'boolean',
          control: 'toggle',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [],
          visible_when: [],
          disabled_when: []
        },
        {
          key: 'temperature',
          label: 'Temperature',
          type: 'number',
          control: 'slider',
          send_mode: 'optional',
          enabled_by_default: false,
          default_value: 0.7,
          min: 0,
          max: 2,
          step: 0.1,
          options: [],
          visible_when: [],
          disabled_when: []
        }
      ]
    };
    fetchModelProviderOptionsSpy.mockResolvedValueOnce(booleanContract);

    const state = createInitialState();
    const llmNode = state.draft.document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: booleanProvider.provider_code,
      model_id: primaryProviderFirstModel.model_id,
      provider_label: booleanProvider.display_name,
      model_label: primaryProviderFirstModel.display_name
    };
    llmNode.config.llm_parameters = {
      schema_version: '1.0.0',
      items: {
        store: { enabled: false, value: false },
        use_responses_websocket: { enabled: true, value: false },
        enable_search: { enabled: false, value: false },
        temperature: { enabled: false, value: 0.7 }
      }
    };

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

    for (const label of ['存储响应', '使用 Responses WebSocket', '启用搜索']) {
      const row = screen
        .getByText(label)
        .closest('.agent-flow-llm-parameter-form__row');

      expect(row?.querySelectorAll('[role="switch"]')).toHaveLength(1);
      expect(
        row?.querySelector('.agent-flow-llm-parameter-form__row-control')
      ).toBeNull();
    }

    expect(screen.queryByText('始终开启')).not.toBeInTheDocument();

    const temperatureRow = screen
      .getByText('Temperature')
      .closest('.agent-flow-llm-parameter-form__row');
    expect(
      temperatureRow?.querySelector(
        '.agent-flow-llm-parameter-form__row-control'
      )
    ).not.toBeNull();

    const storeSwitch = screen
      .getByText('存储响应')
      .closest('.agent-flow-llm-parameter-form__row')
      ?.querySelector('[role="switch"]');
    const searchSwitch = screen
      .getByText('启用搜索')
      .closest('.agent-flow-llm-parameter-form__row')
      ?.querySelector('[role="switch"]');

    expect(storeSwitch).toHaveAttribute('aria-checked', 'true');
    await waitFor(() => {
      const normalizedNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-llm'
      );

      expect(normalizedNode?.config.llm_parameters).toMatchObject({
        items: {
          store: { enabled: true, value: true }
        }
      });
    });
    fireEvent.click(storeSwitch as HTMLElement);
    fireEvent.click(searchSwitch as HTMLElement);

    await waitFor(() => {
      const updatedNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-llm'
      );

      expect(updatedNode?.config.llm_parameters).toMatchObject({
        items: {
          store: { enabled: true, value: false },
          use_responses_websocket: { enabled: true, value: false },
          enable_search: { enabled: true, value: true }
        }
      });
    });
  });

  test('renders effective context and optional max output in the model selector options', async () => {
    const duplicatedModelContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;

    duplicatedModelContract.providers[0].model_groups[0].model.context_window = 256000;
    duplicatedModelContract.providers[0].model_groups[0].model.max_output_tokens = 8192;
    duplicatedModelContract.providers[0].model_groups[1].model.context_window = 64000;
    duplicatedModelContract.providers[0].model_groups[1].model.max_output_tokens =
      null;
    fetchModelProviderOptionsSpy.mockResolvedValueOnce(duplicatedModelContract);

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();
    await openModelDropdown();

    expect(await screen.findByLabelText('上下文 256K')).toBeInTheDocument();
    expect(screen.getAllByText('输出 8.2K').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByLabelText('上下文 64K')).toBeInTheDocument();
  });

  test('shows a formal error state when the current provider is unavailable', async () => {
    const state = createInitialState();
    const llmNode = state.draft.document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!llmNode) {
      throw new Error('expected default LLM node');
    }

    llmNode.config.model_provider = {
      provider_code: 'provider_stale',
      model_id: 'gpt-4o-mini'
    };

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={state}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();
    await openModelDropdown();

    expect(
      await screen.findByText('当前节点引用的模型供应商不可用。')
    ).toBeInTheDocument();
  });

  test('switches provider by choosing a model from another provider group', async () => {
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

    await openModelSettings();
    await openModelDropdown();
    await clickModelOption(secondaryProviderFirstModel.display_name);

    await waitFor(() => {
      const llmNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-llm'
      );
      expect(llmNode?.config.model_provider).toEqual({
        provider_code: secondaryProviderOption.provider_code,
        model_id: secondaryProviderFirstModel.model_id
      });
    });
    await waitFor(() => {
      expect(
        screen.queryByRole('button', {
          name: `${secondaryProviderOption.display_name} ${secondaryProviderFirstGroup.model.display_name} ${secondaryProviderFirstModel.display_name}`
        })
      ).not.toBeInTheDocument();
    });
  });

  test('falls back to the left margin when there is insufficient space left of the node detail panel', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <div className="agent-flow-node-detail">
          <AgentFlowEditorStoreProvider initialState={createInitialState()}>
            <SelectionSeed nodeId="node-llm" />
            <NodeConfigTab />
          </AgentFlowEditorStoreProvider>
        </div>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const nodeDetail = container.querySelector('.agent-flow-node-detail');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody || !nodeDetail) {
      throw new Error('expected editor body and node detail container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 800,
      height: 800
    });
    mockElementRect(nodeDetail, {
      left: 300,
      top: 40,
      width: 500,
      height: 720
    });
    mockElementRect(trigger, {
      left: 600,
      top: 180,
      width: 180,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });

    expect(dialog).toHaveStyle({
      left: '16px',
      top: '180px'
    });
  });
});
