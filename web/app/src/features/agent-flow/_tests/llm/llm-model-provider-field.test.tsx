import { describe, test, expect } from 'vitest';
import {
  DocumentObserver,
  SelectionSeed,
  clickModelOption,
  createInitialState,
  openModelDropdown,
  openModelSettings,
  primaryProviderFirstGroup,
  primaryProviderFirstModel,
  primaryProviderOption,
  primaryProviderSecondGroup,
  primaryProviderSecondModel,
  renderWithProviders
} from './llm-model-provider-field.fixtures';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import { listLlmProviderOptions } from '../../lib/model-options';
import { modelProviderOptionsContract } from '../../../../test/model-provider-contract-fixtures';

describe('LlmModelField', () => {
  test('maps provider-level parameter schema and effective model limits from provider options', () => {
    const providerOptions = listLlmProviderOptions(
      modelProviderOptionsContract
    );
    const openaiProvider = providerOptions.find(
      (option) => option.value === primaryProviderOption.provider_code
    );

    expect(openaiProvider?.parameterForm?.fields[0]?.key).toBe('temperature');
    expect(openaiProvider?.icon).toBe(
      'https://cdn.example.com/openai-compatible.svg'
    );
    expect(openaiProvider?.models[0]).toMatchObject({
      contextWindow: primaryProviderFirstModel.context_window,
      effectiveContextWindow: primaryProviderFirstModel.context_window,
      maxOutputTokens: primaryProviderFirstModel.max_output_tokens
    });
  });

  test('renders the configured provider svg in the selected model chip', async () => {
    const initialState = createInitialState();
    const llmNode = initialState.draft.document.graph.nodes.find(
      (node) => node.id === 'node-llm'
    );

    if (!llmNode) {
      throw new Error('expected llm node');
    }

    llmNode.config.model_provider = {
      provider_code: primaryProviderOption.provider_code,
      model_id: primaryProviderFirstModel.model_id,
      protocol: primaryProviderOption.protocol,
      provider_label: primaryProviderOption.display_name,
      model_label: primaryProviderFirstModel.display_name,
      schema_fetched_at: '2026-04-25T10:00:00Z'
    };

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={initialState}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    const trigger = await screen.findByRole('button', { name: '模型' });

    await waitFor(() =>
      expect(
        trigger.querySelector('.agent-flow-model-chip__provider-image')
      ).toHaveAttribute('src', 'https://cdn.example.com/openai-compatible.svg')
    );
  });

  test('localizes provider parameter form fields and reasoning effort options from i18n catalog', () => {
    const localizedContract = JSON.parse(
      JSON.stringify(modelProviderOptionsContract)
    ) as typeof modelProviderOptionsContract;
    const localizedProvider = localizedContract.providers[0];

    localizedContract.locale_meta = {
      requested_locale: 'zh_Hans',
      resolved_locale: 'zh_Hans',
      user_preferred_locale: 'zh_Hans',
      accept_language: 'zh-Hans-CN,zh;q=0.9,en;q=0.8',
      fallback_locale: 'en_US',
      supported_locales: ['zh_Hans', 'en_US']
    };
    localizedContract.i18n_catalog = {
      [localizedProvider.namespace]: {
        zh_Hans: {
          parameters: {
            reasoning_effort: {
              label: '推理强度',
              description: '控制推理模型投入的推理量。',
              options: {
                xhigh: {
                  label: '极高'
                }
              }
            }
          }
        }
      }
    };
    localizedProvider.parameter_form = {
      schema_version: '1.0.0',
      fields: [
        {
          key: 'reasoning_effort',
          label: 'parameters.reasoning_effort.label',
          description: 'parameters.reasoning_effort.description',
          type: 'enum',
          control: 'select',
          send_mode: 'optional',
          enabled_by_default: false,
          options: [
            {
              label: 'parameters.reasoning_effort.options.xhigh.label',
              value: 'xhigh'
            }
          ],
          visible_when: [],
          disabled_when: []
        }
      ]
    };

    const providerOptions = listLlmProviderOptions(localizedContract);
    const openaiProvider = providerOptions.find(
      (option) => option.value === localizedProvider.provider_code
    );
    const reasoningField = openaiProvider?.parameterForm?.fields[0];

    expect(reasoningField).toMatchObject({
      key: 'reasoning_effort',
      label: '推理强度',
      description: '控制推理模型投入的推理量。',
      control: 'select'
    });
    expect(reasoningField?.options[0]).toMatchObject({
      label: '极高',
      value: 'xhigh'
    });
  });

  test('AC-002 AC-005 AC-007 opens a provider-model dialog without exposing source instances and saves only the aggregate model identity', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const { container } = renderWithProviders(
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

    expect(container.querySelector('.agent-flow-model-field')).toBeNull();

    await openModelSettings();
    expect(
      screen.getByRole('combobox', { name: '选择供应商和模型' })
    ).toBeInTheDocument();
    expect(
      screen.queryByText(primaryProviderOption.display_name)
    ).not.toBeInTheDocument();
    await openModelDropdown();
    expect(
      await screen.findByText(primaryProviderOption.display_name)
    ).toBeInTheDocument();
    expect(
      screen.queryByText(
        primaryProviderFirstGroup.targets
          .map((target) => target.source_instance_display_name)
          .join(' -> ')
      )
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        primaryProviderSecondGroup.targets
          .map((target) => target.source_instance_display_name)
          .join(' -> ')
      )
    ).not.toBeInTheDocument();
    expect(screen.queryByText('个来源实例 ·')).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '模型供应商设置' })
    ).toBeInTheDocument();

    await clickModelOption(primaryProviderSecondModel.display_name);

    await waitFor(() => {
      const llmNode = latestDocument.graph.nodes.find(
        (node) => node.id === 'node-llm'
      );

      expect(llmNode?.config.model_provider).toEqual({
        provider_code: 'openai_compatible',
        model_id: primaryProviderSecondModel.model_id
      });
      expect(llmNode?.config).toMatchObject({
        llm_parameters: {
          schema_version: '1.0.0',
          items: {
            temperature: {
              enabled: false,
              value: 0.7
            }
          }
        }
      });
    });
  }, 10_000);

  test('AC-005 does not match llm model search by source instance label or source_instance_id', async () => {
    const sourceInstanceLabel =
      primaryProviderFirstGroup.targets[0].source_instance_display_name;
    const sourceInstanceId =
      primaryProviderFirstGroup.targets[0].source_instance_id;

    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();
    await openModelDropdown();

    const combobox = screen.getByRole('combobox', {
      name: '选择供应商和模型'
    });
    fireEvent.change(combobox, { target: { value: sourceInstanceLabel } });

    expect(await screen.findByText('没有匹配的模型结果')).toBeInTheDocument();

    fireEvent.change(combobox, { target: { value: sourceInstanceId } });

    expect(await screen.findByText('没有匹配的模型结果')).toBeInTheDocument();
  });
});
