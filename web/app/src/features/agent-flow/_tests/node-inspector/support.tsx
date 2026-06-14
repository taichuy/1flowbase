/* eslint-disable react-refresh/only-export-components */

import { readFileSync } from 'node:fs';

import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { vi } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  type BuiltinFlowNodeType
} from '@1flowbase/flow-schema';
import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import {
  modelProviderOptionsContract,
  modelProviderOptionsProviders
} from '../../../../test/model-provider-contract-fixtures';

import { createNodeDocument } from '../../lib/document/node-factory';
import * as dataModelOptionsApi from '../../api/data-model-options';
import * as modelProviderOptionsApi from '../../api/model-provider-options';
import { TemplatedNamedBindingsField } from '../../components/bindings/TemplatedNamedBindingsField';
import { NodeDetailPanel } from '../../components/detail/NodeDetailPanel';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { NodeInspector } from '../../components/inspector/NodeInspector';
import * as nodeSchemaAdapterApi from '../../schema/node-schema-adapter';
import * as nodeSchemaRegistry from '../../schema/node-schema-registry';
import { validateDocument } from '../../lib/validate-document';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { selectWorkingDocument } from '../../store/editor/selectors';

export const SLOW_NODE_INSPECTOR_TEST_TIMEOUT = 20_000;

vi.setConfig({ testTimeout: SLOW_NODE_INSPECTOR_TEST_TIMEOUT });

vi.mock('@monaco-editor/react', () => ({
  default: ({
    'aria-label': ariaLabel,
    options,
    value,
    onChange
  }: {
    'aria-label'?: string;
    options?: { ariaLabel?: string };
    value?: string;
    onChange?: (value?: string) => void;
  }) => (
    <textarea
      aria-label={ariaLabel ?? options?.ariaLabel}
      value={value ?? ''}
      onChange={(event) => onChange?.(event.target.value)}
    />
  )
}));

export const primaryProviderOption = modelProviderOptionsProviders[0];
export const primaryProviderFirstGroup = primaryProviderOption.model_groups[0];
export const primaryProviderFirstModel = primaryProviderFirstGroup.models[0];
export const fetchModelProviderOptionsSpy = vi.spyOn(
  modelProviderOptionsApi,
  'fetchModelProviderOptions'
);
export const fetchDataModelOptionsSpy = vi.spyOn(
  dataModelOptionsApi,
  'fetchDataModelOptions'
);
export const resolveAgentFlowNodeSchemaSpy = vi.spyOn(
  nodeSchemaRegistry,
  'resolveAgentFlowNodeSchema'
);
export const createAgentFlowNodeSchemaAdapterSpy = vi.spyOn(
  nodeSchemaAdapterApi,
  'createAgentFlowNodeSchemaAdapter'
);

export function createInitialState() {
  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function createInitialStateWithCodeNode() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(createNodeDocument('code', 'node-code', 720, 240));

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function createInitialStateWithCustomCodeNode() {
  const state = createInitialStateWithCodeNode();
  const codeNode = state.draft.document.graph.nodes.find(
    (node) => node.id === 'node-code'
  );

  if (!codeNode) {
    throw new Error('expected code node');
  }

  codeNode.config.source = 'return { riskScore: 0.82 };';
  codeNode.bindings.named_bindings = {
    kind: 'named_bindings',
    value: [
      {
        name: 'arg1',
        selector: ['sys', 'conversation_id']
      }
    ]
  };
  codeNode.outputs = [
    {
      key: 'riskScore',
      title: 'Risk Score',
      valueType: 'number'
    }
  ];

  return state;
}

export function createInitialStateWithStructuredCodeNode() {
  const state = createInitialStateWithCodeNode();
  const codeNode = state.draft.document.graph.nodes.find(
    (node) => node.id === 'node-code'
  );

  if (!codeNode) {
    throw new Error('expected code node');
  }

  codeNode.outputs = [
    {
      key: 'chat_history',
      title: 'Chat History',
      valueType: 'array',
      jsonSchema: {
        type: 'array',
        items: {
          type: 'object',
          required: ['role', 'content'],
          properties: {
            role: { type: 'string' },
            content: { type: 'string' }
          }
        }
      }
    }
  ];

  return state;
}

export function createInitialStateWithLoopNode() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(createNodeDocument('loop', 'node-loop', 720, 240));

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function createInitialStateWithIfElseNode() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(
    createNodeDocument('if_else', 'node-if-else', 720, 240)
  );

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function createInitialStateWithDataModelNode(
  nodeType: BuiltinFlowNodeType = 'data_model_list'
) {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(
    createNodeDocument(nodeType, 'node-data-model', 720, 240)
  );
  document.graph.edges.push({
    id: 'edge-start-data-model',
    source: 'node-start',
    target: 'node-data-model',
    sourceHandle: null,
    targetHandle: null,
    containerId: null,
    points: []
  });

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function createInitialStateWithHttpRequestNode() {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

  document.graph.nodes.push(
    createNodeDocument('http_request', 'node-http-request', 720, 240)
  );
  document.graph.edges.push({
    id: 'edge-start-http-request',
    source: 'node-start',
    target: 'node-http-request',
    sourceHandle: null,
    targetHandle: null,
    containerId: null,
    points: []
  });

  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    versions: []
  };
}

export function SelectionSeed({ nodeId }: { nodeId: string }) {
  const setSelection = useAgentFlowEditorStore((state) => state.setSelection);

  useEffect(() => {
    setSelection({
      selectedNodeId: nodeId,
      selectedNodeIds: [nodeId]
    });
  }, [nodeId, setSelection]);

  return null;
}

export function DocumentObserver({
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

export function FocusIssueSeed() {
  const focusIssueField = useAgentFlowEditorStore(
    (state) => state.focusIssueField
  );

  useEffect(() => {
    focusIssueField({
      nodeId: 'node-llm',
      sectionKey: 'inputs',
      fieldKey: 'config.model_provider'
    });
  }, [focusIssueField]);

  return null;
}

export function renderWithProviders(ui: ReactNode) {
  return render(<AppProviders>{ui}</AppProviders>);
}

export function getLlmNodeConfig(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
) {
  const llmNode = document.graph.nodes.find((node) => node.id === 'node-llm');

  if (!llmNode) {
    throw new Error('expected default LLM node');
  }

  return llmNode.config;
}

export function getDataModelNode(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
) {
  const dataModelNode = document.graph.nodes.find(
    (node) => node.id === 'node-data-model'
  );

  if (!dataModelNode) {
    throw new Error('expected data model node');
  }

  return dataModelNode;
}

export function getCodeNode(
  document: ReturnType<typeof createDefaultAgentFlowDocument>
) {
  const codeNode = document.graph.nodes.find((node) => node.id === 'node-code');

  if (!codeNode) {
    throw new Error('expected code node');
  }

  return codeNode;
}

export async function openSelect(label: string) {
  const combobox = await screen.findByRole('combobox', { name: label });

  fireEvent.mouseDown(combobox);
  fireEvent.keyDown(combobox, { key: 'ArrowDown' });

  return combobox;
}

export async function selectOption(label: string) {
  const matches = await screen.findAllByTitle(label);
  const option = matches[matches.length - 1];

  fireEvent.click(option);
}

export async function selectDataModelOption(value: string) {
  const option = await screen.findByTestId(`data-model-option-${value}`);

  fireEvent.click(option);
}


export async function setupNodeInspectorTest() {
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
  fetchDataModelOptionsSpy.mockReset();
  fetchDataModelOptionsSpy.mockResolvedValue([
    {
      value: 'orders',
      label: 'Orders',
      state: 'enabled',
      disabled: false,
      disabledReason: null,
      modelId: 'model-orders',
      modelCode: 'orders',
      fields: [
        {
          code: 'name',
          title: 'Name',
          valueType: 'string',
          required: true,
          writable: true
        },
        {
          code: 'amount',
          title: 'Amount',
          valueType: 'number',
          required: false,
          writable: true
        },
        {
          code: 'status',
          title: 'Status',
          valueType: 'enum',
          required: false,
          writable: true
        },
        {
          code: 'customer',
          title: 'Customer',
          valueType: 'many_to_one',
          required: false,
          writable: true
        },
        {
          code: 'lines',
          title: 'Lines',
          valueType: 'one_to_many',
          required: false,
          writable: true
        },
        {
          code: 'approved',
          title: 'Approved',
          valueType: 'boolean',
          required: false,
          writable: true
        }
      ]
    },
    {
      value: 'empty_orders',
      label: 'Empty Orders',
      state: 'enabled',
      disabled: false,
      disabledReason: null,
      modelId: 'model-empty-orders',
      modelCode: 'empty_orders',
      fields: []
    },
    {
      value: 'draft_orders',
      label: 'Draft Orders',
      state: 'unpublished',
      disabled: true,
      disabledReason: 'Data Model is not published',
      modelId: 'model-draft-orders',
      modelCode: 'draft_orders',
      fields: []
    },
    {
      value: 'disabled_orders',
      label: 'Disabled Orders',
      state: 'disabled',
      disabled: true,
      disabledReason: 'Data Model is disabled',
      modelId: 'model-disabled-orders',
      modelCode: 'disabled_orders',
      fields: []
    },
    {
      value: 'broken_orders',
      label: 'Broken Orders',
      state: 'broken',
      disabled: true,
      disabledReason: 'Data Model is broken',
      modelId: 'model-broken-orders',
      modelCode: 'broken_orders',
      fields: []
    }
  ]);
  resolveAgentFlowNodeSchemaSpy.mockClear();
  createAgentFlowNodeSchemaAdapterSpy.mockClear();
}

export { fireEvent, readFileSync, screen, waitFor, within };
export {
  AgentFlowEditorStoreProvider,
  NodeConfigTab,
  NodeDetailPanel,
  NodeInspector,
  TemplatedNamedBindingsField,
  createDefaultAgentFlowDocument,
  modelProviderOptionsContract,
  nodeSchemaAdapterApi,
  nodeSchemaRegistry,
  validateDocument
};
