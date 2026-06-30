import type {
  FlowNodeType,
  NodeRuntimePanelFieldDocument,
  NodeRuntimePanelSectionDocument
} from '@1flowbase/flow-schema';

import type {
  SchemaBlock,
  SchemaFieldBlock,
  SchemaSectionBlock
} from '../../../shared/schema-ui/contracts/canvas-node-schema';
import {
  getNodeDefinitionSections,
  type NodeDefinitionField,
  type NodeEditorKind
} from '../lib/node-definitions';
import { getBuiltinNodeRuntimeContract } from '../lib/node-definitions/contracts';
import { i18nText } from '../../../shared/i18n/text';

const FIELD_RENDERER_BY_EDITOR: Record<NodeEditorKind, string> = {
  text: 'text',
  static_select: 'static_select',
  data_model: 'data_model',
  data_model_query: 'data_model_query',
  llm_model: 'llm_model',
  llm_context_policy: 'llm_context_policy',
  llm_external_reasoning_policy: 'llm_external_reasoning_policy',
  llm_tool_registrations: 'llm_tool_registrations',
  llm_prompt_messages: 'llm_prompt_messages',
  llm_response_format: 'llm_response_format',
  code_source: 'code_source',
  number: 'number',
  selector: 'selector',
  selector_list: 'selector_list',
  templated_text: 'templated_text',
  named_bindings: 'named_bindings',
  templated_named_bindings: 'templated_named_bindings',
  condition_group: 'condition_group',
  if_else_branches: 'if_else_branches',
  state_write: 'state_write',
  variable_assignment: 'variable_assignment',
  output_contract_definition: 'output_contract_definition',
  start_input_fields: 'start_input_fields',
  start_model_list: 'start_model_list'
};

const CONTRACT_FIELD_RENDERER_ALLOWLIST = new Set([
  ...Object.values(FIELD_RENDERER_BY_EDITOR),
  'switch',
  'variable_assignment',
  'http_request_endpoint',
  'http_request_key_values',
  'http_request_body',
  'http_request_curl_import'
]);

function createFieldBlock(field: NodeDefinitionField): SchemaFieldBlock {
  const block: SchemaFieldBlock = {
    kind: 'field',
    renderer: FIELD_RENDERER_BY_EDITOR[field.editor],
    path: field.key,
    label: field.label,
    options: field.options
  };

  if (field.visibleWhen) {
    block.visibleWhen = field.visibleWhen;
  }

  return block;
}

function createSectionBlock(
  title: string,
  fields: NodeDefinitionField[]
): SchemaSectionBlock {
  return {
    kind: 'section',
    title,
    blocks: fields.map(createFieldBlock)
  };
}

function createContractFieldBlock(
  field: NodeRuntimePanelFieldDocument
): SchemaFieldBlock | null {
  if (!CONTRACT_FIELD_RENDERER_ALLOWLIST.has(field.renderer)) {
    return null;
  }

  return {
    kind: 'field',
    renderer: field.renderer,
    path: field.key,
    label: field.title,
    options: field.options as SchemaFieldBlock['options'],
    min: field.min,
    max: field.max,
    step: field.step,
    numberFormat: field.numberFormat
  };
}

function createContractSectionBlock(
  section: NodeRuntimePanelSectionDocument
): SchemaSectionBlock | null {
  const fields = (section.fields ?? [])
    .map(createContractFieldBlock)
    .filter((field): field is SchemaFieldBlock => field !== null);

  if (fields.length === 0) {
    return null;
  }

  return {
    kind: 'section',
    title: section.title ?? section.key ?? 'Config',
    blocks: fields
  };
}

const EDITABLE_OUTPUT_CONTRACT_NODE_TYPES = new Set<FlowNodeType>([
  'code',
  'workflow_end'
]);

function shouldExposeGeneratedOutputVariables(nodeType: FlowNodeType) {
  return (
    nodeType !== 'start' &&
    nodeType !== 'workflow_start' &&
    nodeType !== 'if_else' &&
    !EDITABLE_OUTPUT_CONTRACT_NODE_TYPES.has(nodeType)
  );
}

function buildSharedOutputVariableBlocks(
  nodeType: FlowNodeType
): SchemaBlock[] {
  if (!shouldExposeGeneratedOutputVariables(nodeType)) {
    return [];
  }

  return [
    {
      kind: 'view',
      renderer: 'output_contract',
      title: i18nText('agentFlow', 'auto.output_variable'),
      key: `${nodeType}-generated-outputs`
    }
  ];
}

function splitBlocksByPath(sections: SchemaSectionBlock[], paths: Set<string>) {
  const extractedBlocksByPath = new Map<string, SchemaFieldBlock>();
  const remainingSections = sections
    .map((section) => {
      const remainingBlocks = section.blocks.filter((block) => {
        if (block.kind === 'field' && paths.has(block.path)) {
          extractedBlocksByPath.set(block.path, block);
          return false;
        }

        return true;
      });

      return { ...section, blocks: remainingBlocks };
    })
    .filter((section) => section.blocks.length > 0);

  const extractedBlocks = [...paths]
    .map((path) => extractedBlocksByPath.get(path))
    .filter((block): block is SchemaFieldBlock => block !== undefined);

  return { remainingSections, extractedBlocks };
}

export function buildNodeDetailHeaderBlocks(): SchemaBlock[] {
  return [
    {
      kind: 'field',
      renderer: 'header_alias',
      path: 'alias',
      label: i18nText('agentFlow', 'auto.node_alias')
    },
    {
      kind: 'field',
      renderer: 'header_description',
      path: 'description',
      label: i18nText('agentFlow', 'auto.node_introduction')
    }
  ];
}

export function buildNodeCardBlocks(nodeType: FlowNodeType): SchemaBlock[] {
  const contract = getBuiltinNodeRuntimeContract(nodeType);

  return [
    {
      kind: 'view',
      renderer: 'card_eyebrow',
      key: `${nodeType}-${contract?.card.title ?? 'node'}-eyebrow`,
      title: contract?.card.title
    },
    ...(contract?.meta.type === 'llm'
      ? [
          {
            kind: 'view' as const,
            renderer: 'card_model',
            key: `${nodeType}-model`
          }
        ]
      : []),
    {
      kind: 'view',
      renderer: 'card_description',
      key: `${nodeType}-${contract?.card.title ?? 'node'}-description`,
      title: contract?.card.description
    }
  ];
}

export function buildCommonConfigBlocks(nodeType: FlowNodeType): SchemaBlock[] {
  const contract = getBuiltinNodeRuntimeContract(nodeType);
  const contractSections = (contract?.panel.sections ?? [])
    .filter((section) => {
      if (
        section.key === 'basics' ||
        (section.key === 'outputs' &&
          !EDITABLE_OUTPUT_CONTRACT_NODE_TYPES.has(nodeType))
      ) {
        return false;
      }

      if (nodeType === 'llm' && section.key === 'advanced') {
        return false;
      }

      return true;
    })
    .map(createContractSectionBlock)
    .filter((section): section is SchemaSectionBlock => section !== null);
  const definitionSections = contractSections?.length
    ? contractSections
    : getNodeDefinitionSections(nodeType)
        .filter((section) => {
          if (
            section.key === 'basics' ||
            (section.key === 'outputs' &&
              !EDITABLE_OUTPUT_CONTRACT_NODE_TYPES.has(nodeType))
          ) {
            return false;
          }

          if (nodeType === 'llm' && section.key === 'advanced') {
            return false;
          }

          return true;
        })
        .flatMap((section) => {
          return [createSectionBlock(section.title, section.fields)];
        });
  const { remainingSections, extractedBlocks } =
    nodeType === 'http_request'
      ? splitBlocksByPath(
          definitionSections,
          new Set([
            'config.timeout_ms',
            'config.max_response_bytes',
            'config.curl_import',
            'config.verify_ssl',
            'config.store_response_as_file'
          ])
        )
      : { remainingSections: definitionSections, extractedBlocks: [] };
  const outputVariableBlocks = buildSharedOutputVariableBlocks(nodeType);
  const policyBlocks: SchemaBlock[] =
    nodeType === 'start'
      ? []
      : [
          {
            kind: 'view',
            renderer: 'policy_group',
            title: i18nText('agentFlow', 'auto.strategy')
          }
        ];

  return [
    ...remainingSections,
    ...outputVariableBlocks,
    ...(extractedBlocks.length > 0
      ? [
          {
            kind: 'section' as const,
            title: 'Inputs',
            blocks: extractedBlocks
          }
        ]
      : []),
    ...policyBlocks,
    {
      kind: 'view',
      renderer: 'relations',
      title: i18nText('agentFlow', 'auto.next_step')
    }
  ];
}

export function buildCommonLastRunBlocks(): SchemaBlock[] {
  return [
    {
      kind: 'view',
      renderer: 'runtime_summary',
      title: i18nText('agentFlow', 'auto.running_summary')
    },
    {
      kind: 'view',
      renderer: 'runtime_io',
      title: i18nText('agentFlow', 'auto.run_input_output')
    },
    {
      kind: 'view',
      renderer: 'runtime_metadata',
      title: i18nText('agentFlow', 'auto.run_metadata')
    }
  ];
}

export function buildNodeRuntimeSlots() {
  return {
    summary: 'summary',
    output_contract: 'output_contract',
    policy_group: 'policy_group',
    relations: 'relations',
    runtime_summary: 'runtime_summary',
    runtime_io: 'runtime_io',
    runtime_metadata: 'runtime_metadata'
  } as const;
}
