window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');

import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import {
  modelProviderOptionsProviders,
  modelProviderOptionsContract
} from '../../../../test/model-provider-contract-fixtures';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { AppProviders } from '../../../../app/AppProviders';
import * as modelProviderOptionsApi from '../../api/model-provider-options';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { listLlmProviderOptions } from '../../lib/model-options';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';
import { appI18n } from '../../../../shared/i18n/app-i18n';

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

function createInitialState() {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-18T10:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
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

function mockElementRect(
  element: Element,
  rect: Partial<DOMRect> & Pick<DOMRect, 'width' | 'height' | 'left' | 'top'>
) {
  const resolvedRect = {
    x: rect.left,
    y: rect.top,
    width: rect.width,
    height: rect.height,
    top: rect.top,
    left: rect.left,
    right: rect.right ?? rect.left + rect.width,
    bottom: rect.bottom ?? rect.top + rect.height,
    toJSON: () => ''
  } satisfies DOMRect;

  vi.spyOn(element, 'getBoundingClientRect').mockReturnValue(resolvedRect);
}

async function openModelSettings() {
  fireEvent.click(await screen.findByRole('button', { name: '模型' }));
  expect(
    await screen.findByRole('heading', { name: '模型设置' })
  ).toBeInTheDocument();
}

async function openModelDropdown() {
  const combobox = await screen.findByRole('combobox', {
    name: '选择供应商和模型'
  });

  fireEvent.mouseDown(combobox.closest('.ant-select-selector') ?? combobox);
  fireEvent.keyDown(combobox, { key: 'ArrowDown' });
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

describe('LlmModelField', () => {
  beforeEach(async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    fetchModelProviderOptionsSpy.mockReset();
    fetchModelProviderOptionsSpy.mockResolvedValue(
      modelProviderOptionsContract
    );
  });

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

  test('renders the model settings inside the canvas body with minimum width and half-height', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });

    expect(dialog).toHaveClass('agent-flow-model-settings__panel');
    expect(editorBody.contains(dialog)).toBe(true);
    expect(dialog).toHaveStyle({
      width: '320px',
      height: '400px'
    });
  });

  test('opens the model settings panel in the blank area left of the node detail panel', async () => {
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
      width: 1200,
      height: 800
    });
    mockElementRect(nodeDetail, {
      left: 760,
      top: 40,
      width: 420,
      height: 720
    });
    mockElementRect(trigger, {
      left: 980,
      top: 180,
      width: 180,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });

    expect(dialog).toHaveStyle({
      left: '416px',
      top: '180px'
    });
  });

  test('closes the model settings panel two seconds after the mouse leaves it', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();

    vi.useFakeTimers();
    try {
      const dialog = screen.getByRole('dialog', { name: '模型设置' });

      fireEvent.mouseLeave(dialog);
      act(() => {
        vi.advanceTimersByTime(1999);
      });

      expect(
        screen.getByRole('dialog', { name: '模型设置' })
      ).toBeInTheDocument();

      act(() => {
        vi.advanceTimersByTime(1);
      });

      expect(
        screen.queryByRole('dialog', { name: '模型设置' })
      ).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('keeps the model settings panel open when the mouse returns before auto close', async () => {
    renderWithProviders(
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <SelectionSeed nodeId="node-llm" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openModelSettings();

    vi.useFakeTimers();
    try {
      const dialog = screen.getByRole('dialog', { name: '模型设置' });

      fireEvent.mouseLeave(dialog);
      act(() => {
        vi.advanceTimersByTime(1500);
      });
      fireEvent.mouseEnter(dialog);
      act(() => {
        vi.advanceTimersByTime(500);
      });

      expect(
        screen.getByRole('dialog', { name: '模型设置' })
      ).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('resizes the model settings panel width from both sides while keeping the 320px minimum', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const leftResizeHandle = screen.getByTestId(
      'agent-flow-model-settings-resize-handle-left'
    );
    const rightResizeHandle = screen.getByTestId(
      'agent-flow-model-settings-resize-handle'
    );

    fireEvent.mouseDown(rightResizeHandle, {
      clientX: 320,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 460,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        width: '460px'
      });
    });

    fireEvent.mouseDown(rightResizeHandle, {
      clientX: 460,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 120,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        width: '320px'
      });
    });

    fireEvent.mouseDown(leftResizeHandle, {
      clientX: 636,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 496,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '496px',
        width: '460px'
      });
    });

    fireEvent.mouseDown(leftResizeHandle, {
      clientX: 496,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: 760,
      clientY: 180
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '636px',
        width: '320px'
      });
    });
  });

  test('resizes the model settings panel height from the bottom while keeping the minimum height', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const bottomResizeHandle = screen.getByRole('separator', {
      name: '向下调整模型设置高度'
    });

    expect(dialog).toHaveStyle({
      height: '400px'
    });

    fireEvent.mouseDown(bottomResizeHandle, {
      clientX: 520,
      clientY: 560
    });
    fireEvent.mouseMove(window, {
      clientX: 520,
      clientY: 660
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        height: '500px'
      });
    });

    fireEvent.mouseDown(bottomResizeHandle, {
      clientX: 520,
      clientY: 660
    });
    fireEvent.mouseMove(window, {
      clientX: 520,
      clientY: 260
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        height: '240px'
      });
    });
  });

  test('supports dragging the model settings panel within the canvas bounds', async () => {
    const { container } = renderWithProviders(
      <div className="agent-flow-editor__body">
        <AgentFlowEditorStoreProvider initialState={createInitialState()}>
          <SelectionSeed nodeId="node-llm" />
          <NodeConfigTab />
        </AgentFlowEditorStoreProvider>
      </div>
    );

    const editorBody = container.querySelector('.agent-flow-editor__body');
    const trigger = await screen.findByRole('button', { name: '模型' });

    if (!editorBody) {
      throw new Error('expected editor body container');
    }

    mockElementRect(editorBody, {
      left: 0,
      top: 0,
      width: 1200,
      height: 800
    });
    mockElementRect(trigger, {
      left: 980,
      top: 160,
      width: 240,
      height: 40
    });

    fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: '模型设置' });
    const dragHandle = screen.getByTestId(
      'agent-flow-model-settings-drag-handle'
    );

    fireEvent.mouseDown(dragHandle, {
      clientX: 680,
      clientY: 180
    });
    fireEvent.mouseMove(window, {
      clientX: -120,
      clientY: -80
    });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      expect(dialog).toHaveStyle({
        left: '16px',
        top: '16px'
      });
    });
  });

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

    for (const label of [
      '存储响应',
      '使用 Responses WebSocket',
      '启用搜索'
    ]) {
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
