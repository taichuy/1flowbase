import type { FlowNodeType } from '@1flowbase/flow-schema';
import type { NodeFlowRole } from './types';

import { nodeDefinitionMeta } from './meta';
import { getRegisteredNodeDefinition } from './registry';
import { answerNodeDefinition } from './nodes/answer';
import { codeNodeDefinition } from './nodes/code';
import { dataModelNodeDefinitions } from './nodes/data-model';
import { humanInputNodeDefinition } from './nodes/human-input';
import { httpRequestNodeDefinition } from './nodes/http-request';
import { ifElseNodeDefinition } from './nodes/if-else';
import { iterationNodeDefinition } from './nodes/iteration';
import { knowledgeRetrievalNodeDefinition } from './nodes/knowledge-retrieval';
import { llmNodeDefinition } from './nodes/llm';
import { loopNodeDefinition } from './nodes/loop';
import { parameterExtractorNodeDefinition } from './nodes/parameter-extractor';
import { questionClassifierNodeDefinition } from './nodes/question-classifier';
import { startNodeDefinition } from './nodes/start';
import { templateTransformNodeDefinition } from './nodes/template-transform';
import { toolNodeDefinition } from './nodes/tool';
import { toolResultNodeDefinition } from './nodes/tool-result';
import { variableAssignerNodeDefinition } from './nodes/variable-assigner';
import { pluginNodeDefinition } from '../plugin-node-definitions';
import type { InspectorSectionKey, NodeDefinitionMap } from './types';

export type {
  InspectorSectionKey,
  NodeDefinition,
  NodeDefinitionField,
  NodeDefinitionMeta,
  NodeDefinitionSection,
  NodeEditorKind,
  NodeFlowRole
} from './types';

export const nodeDefinitions: NodeDefinitionMap = {
  start: startNodeDefinition,
  answer: answerNodeDefinition,
  llm: llmNodeDefinition,
  knowledge_retrieval: knowledgeRetrievalNodeDefinition,
  question_classifier: questionClassifierNodeDefinition,
  if_else: ifElseNodeDefinition,
  code: codeNodeDefinition,
  template_transform: templateTransformNodeDefinition,
  http_request: httpRequestNodeDefinition,
  tool: toolNodeDefinition,
  tool_result: toolResultNodeDefinition,
  ...dataModelNodeDefinitions,
  variable_assigner: variableAssignerNodeDefinition,
  parameter_extractor: parameterExtractorNodeDefinition,
  iteration: iterationNodeDefinition,
  loop: loopNodeDefinition,
  human_input: humanInputNodeDefinition,
  plugin_node: pluginNodeDefinition
};

export function findInspectorSectionKey(
  nodeType: FlowNodeType,
  fieldKey: string
): InspectorSectionKey | null {
  const definition = nodeDefinitions[nodeType];

  if (!definition) {
    return null;
  }

  for (const section of definition.sections) {
    if (section.fields.some((field) => field.key === fieldKey)) {
      return section.key;
    }
  }

  return null;
}

export function getNodeDefinition(nodeType: FlowNodeType) {
  return nodeDefinitions[nodeType] ?? null;
}

export function getSchemaConfigSections(nodeType: FlowNodeType) {
  return getNodeDefinition(nodeType)?.sections.filter(
    (section) => section.key !== 'basics' && section.key !== 'outputs'
  ) ?? [];
}

export function getNodeDefinitionMeta(nodeType: FlowNodeType) {
  return (
    getRegisteredNodeDefinition(nodeType)?.meta ??
    nodeDefinitionMeta[nodeType as keyof typeof nodeDefinitionMeta]
  );
}

export function getNodeFlowRole(nodeType: FlowNodeType): NodeFlowRole {
  return getNodeDefinitionMeta(nodeType)?.flowRole ?? 'processing';
}

export function getNodeDefinitionSections(nodeType: FlowNodeType) {
  return nodeDefinitions[nodeType]?.sections ?? [];
}

export function getNodeDefinitionFields(
  nodeType: FlowNodeType,
  sectionKey?: InspectorSectionKey
) {
  const sections = getNodeDefinitionSections(nodeType);

  if (!sectionKey) {
    return sections.flatMap((section) => section.fields);
  }

  return sections.find((section) => section.key === sectionKey)?.fields ?? [];
}
