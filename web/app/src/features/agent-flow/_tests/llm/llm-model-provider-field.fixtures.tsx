window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');

import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import {
  modelProviderOptionsProviders,
  modelProviderOptionsContract
} from '../../../../test/model-provider-contract-fixtures';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { AppProviders } from '../../../../app/AppProviders';
import * as modelProviderOptionsApi from '../../api/model-provider-options';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
// Resolve the real LLM schema's lazy field modules during suite collection.
// Cold module transforms must complete before the inspector's readiness assertions.
import '../../components/detail/fields/LlmModelField';
import '../../components/detail/fields/LlmMcpInstancesField';
import '../../components/detail/fields/LlmToolRegistrationsField';
import '../../components/detail/fields/LlmPromptMessagesField';
import '../../components/detail/fields/LlmResponseFormatField';
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

beforeEach(async () => {
  window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
  await appI18n.changeLanguage('zh_Hans');
  fetchModelProviderOptionsSpy.mockReset();
  fetchModelProviderOptionsSpy.mockResolvedValue(modelProviderOptionsContract);
});

export {
  AgentFlowEditorStoreProvider,
  DocumentObserver,
  NodeConfigTab,
  SelectionSeed,
  act,
  clickModelOption,
  createDefaultAgentFlowDocument,
  createInitialState,
  expect,
  fetchModelProviderOptionsSpy,
  fireEvent,
  listLlmProviderOptions,
  mockElementRect,
  modelProviderOptionsContract,
  openModelDropdown,
  openModelSettings,
  primaryProviderFirstGroup,
  primaryProviderFirstModel,
  primaryProviderOption,
  primaryProviderSecondGroup,
  primaryProviderSecondModel,
  renderWithProviders,
  screen,
  secondaryProviderFirstGroup,
  secondaryProviderFirstModel,
  secondaryProviderOption,
  test,
  vi,
  waitFor
};
