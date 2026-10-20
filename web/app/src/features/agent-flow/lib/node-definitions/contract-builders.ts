import type {
  FlowBinding,
  FlowNodeOutputDocument,
  NodeRuntimeSideEffectPolicy,
  NodeRuntimePortDocument,
  NodeRuntimePanelFieldDocument,
  NodeRuntimePanelSectionDocument,
  NodeRuntimeUiContract
} from '@1flowbase/flow-schema';
import { NODE_CONTRIBUTION_SCHEMA_VERSION } from '@1flowbase/flow-schema';

import { createDefaultIfElseBranches } from '../if-else-branches';

import { i18nText } from '../../../../shared/i18n/text';

type ContractCategory = 'io' | 'generation' | 'control' | 'data' | 'external';

const DEFAULT_RUNTIME_POLICY = {
  sideEffect: 'none' as const
};

const COMMON_RUNTIME_INPUTS = [
  { key: 'input_payload', title: 'Input Payload', valueType: 'json' }
];

export function cloneJsonValue<T>(value: T): T {
  return structuredClone(value);
}

function duplicateOutput(
  output: FlowNodeOutputDocument
): FlowNodeOutputDocument {
  return { ...output };
}

function duplicateOutputs(outputs: FlowNodeOutputDocument[]) {
  return outputs.map(duplicateOutput);
}

function duplicateContractDefaults(
  contract: NodeRuntimeUiContract['defaults']
) {
  return {
    ...contract,
    config: cloneJsonValue(contract.config),
    bindings: cloneJsonValue(contract.bindings),
    outputs: duplicateOutputs(contract.outputs)
  };
}

export function duplicateContract(
  contract: NodeRuntimeUiContract
): NodeRuntimeUiContract {
  return {
    ...contract,
    meta: { ...contract.meta },
    defaults: duplicateContractDefaults(contract.defaults),
    ports: {
      inputs: contract.ports.inputs.map((port) => ({ ...port })),
      outputs: contract.ports.outputs.map((port) => ({ ...port }))
    },
    card: { ...contract.card },
    panel: {
      sections: cloneJsonValue(contract.panel.sections)
    },
    runtime: {
      inputs: contract.runtime.inputs?.map((item) => ({ ...item })),
      processData: contract.runtime.processData?.map((item) => ({ ...item })),
      outputs: duplicateOutputs(contract.runtime.outputs)
    },
    policies: cloneJsonValue(contract.policies)
  };
}

function createContractDefaults({
  alias,
  description,
  config,
  bindings,
  outputs
}: {
  alias: string;
  description: string;
  config: Record<string, unknown>;
  bindings: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
}) {
  return {
    alias,
    description,
    configVersion: 1,
    config,
    bindings,
    outputs
  };
}

function createContractPorts(outputs: FlowNodeOutputDocument[]) {
  return {
    inputs: [],
    outputs: outputs.map((output) => ({
      key: output.key,
      title: output.title,
      valueType: output.valueType
    }))
  };
}

export function branchPorts(
  branches: ReturnType<typeof createDefaultIfElseBranches>
): NodeRuntimePortDocument[] {
  return branches.map((branch) => ({
    key: branch.sourceHandle,
    title: branch.title
  }));
}

export function panelField({
  key,
  title,
  renderer,
  valueType = 'string',
  required,
  description,
  options,
  min,
  max,
  step,
  numberFormat
}: Omit<NodeRuntimePanelFieldDocument, 'valueType'> & {
  valueType?: string;
}) {
  return {
    key,
    title,
    renderer,
    valueType,
    required,
    description,
    options,
    min,
    max,
    step,
    numberFormat
  } satisfies NodeRuntimePanelFieldDocument;
}

export function panelSection(
  key: string,
  title: string,
  fields: NodeRuntimePanelFieldDocument[]
): NodeRuntimePanelSectionDocument {
  return {
    key,
    title,
    fields
  };
}

export const basicsPanelSection = panelSection('basics', 'Basics', [
  panelField({
    key: 'alias',
    title: i18nText('agentFlow', 'auto.node_alias'),
    renderer: 'text',
    required: true
  }),
  panelField({
    key: 'description',
    title: i18nText('agentFlow', 'auto.node_introduction'),
    renderer: 'text'
  })
]);

export function outputsPanelSection(outputs: FlowNodeOutputDocument[]) {
  return panelSection(
    'outputs',
    'Outputs',
    outputs.map((output) =>
      panelField({
        key: `outputs.${output.key}`,
        title: output.title,
        renderer: 'text',
        valueType: output.valueType,
        required: true
      })
    )
  );
}

export function createNodeRuntimeContract({
  type,
  title,
  alias,
  description,
  category,
  config,
  bindings = {},
  outputs,
  outputPorts,
  panelSections,
  runtimeOutputs,
  sideEffect = DEFAULT_RUNTIME_POLICY.sideEffect
}: {
  type: BuiltinNodeRuntimeContractType;
  title: string;
  alias?: string;
  description: string;
  category: ContractCategory;
  config: Record<string, unknown>;
  bindings?: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
  outputPorts?: NodeRuntimePortDocument[];
  panelSections: NodeRuntimePanelSectionDocument[];
  runtimeOutputs?: FlowNodeOutputDocument[];
  sideEffect?: NodeRuntimeSideEffectPolicy;
}): NodeRuntimeUiContract {
  return {
    meta: {
      type,
      title,
      schemaVersion: NODE_CONTRIBUTION_SCHEMA_VERSION
    },
    defaults: createContractDefaults({
      alias: alias ?? title,
      description,
      config,
      bindings,
      outputs: duplicateOutputs(outputs)
    }),
    ports: outputPorts
      ? { inputs: [], outputs: outputPorts }
      : createContractPorts(outputs),
    card: {
      title,
      description,
      category
    },
    panel: {
      sections: cloneJsonValue(panelSections)
    },
    runtime: {
      inputs: COMMON_RUNTIME_INPUTS.map((item) => ({ ...item })),
      outputs: duplicateOutputs(runtimeOutputs ?? outputs)
    },
    policies: { sideEffect }
  };
}
import { type BuiltinNodeRuntimeContractType } from './contracts';
