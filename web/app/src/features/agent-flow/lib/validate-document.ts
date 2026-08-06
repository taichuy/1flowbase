import {
  getLlmNodeOutputs,
  validateI18nTextRef,
  validatePublicOutputKey,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';
import type {
  FlowBinding,
  FlowConditionGroupDocument,
  FlowConditionRuleDocument,
  FlowEdgeDocument,
  FlowNodeDocument,
  IfElseBranchDocument
} from '@1flowbase/flow-schema';

import { evaluateSchemaRule } from '../../../shared/schema-ui/v1/runtime/rule-evaluator';
import type { AgentFlowModelProviderOptions } from '../api/model-provider-options';
import {
  extractDataModelQuerySelectors,
  getActiveNodeBindings
} from './data-model-query-binding';
import {
  createLlmToolSourceHandleId,
  getLlmToolInternalLlmNodePolicy,
  getLlmModelProvider,
  getLlmVisibleInternalTools,
  getLlmVisibleInternalToolsEnabled,
  isLlmToolSourceHandle
} from './llm-node-config';
import {
  ERROR_BRANCH_SOURCE_HANDLE,
  nodeUsesErrorBranch
} from './policy/node-error-policy';
import { getBuiltinNodeRuntimeContract } from './node-definitions/contracts';
import { extractNamedBindingSelectors } from './named-binding-expressions';
import type { NodeDefinitionField } from './node-definitions';
import { nodeDefinitions } from './node-definitions';
import { outputTypeSupportsJsonSchema } from './output-contract/schema';
import { isOutputVariableKeyAllowed } from './output-contract/variable-key';
import { hasPluginContributionRef } from './plugin-node-definitions';
import {
  collectConditionSelectors,
  collectIfElseBranchSelectors,
  getIfElseBranchesFromBindings,
  isConditionGroup,
  isConditionRule
} from './if-else-branches';
import {
  isSelectorVisible,
  listVisibleSelectorOptions
} from './selector-options';
import { workflowTriggerVariableNodeId } from './variables/workflow-trigger-variables';
import { parseTemplateSelectorTokens } from './template-binding';
import {
  environmentVariableNodeId,
  type AgentFlowEnvironmentVariable
} from './variables/application-environment-variables';
import {
  conversationVariableNodeId,
  listConversationVariables
} from './variables/conversation-variables';
import { systemVariableNodeId } from './variables/system-variables';
import { i18nText } from '../../../shared/i18n/text';
import { validateCodeNamedBindings } from './validation/code-named-bindings';
import { pushFieldIssue, type AgentFlowIssue } from './validation/issues';
import { validateAuthoringDocument } from '../../flow-editor/authoring/validation';

export type { AgentFlowIssue } from './validation/issues';

export const VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE =
  'variable_aggregator_candidate_type_mismatch';

export function isVariableAggregatorCandidateTypeMismatchIssue(
  issue: AgentFlowIssue
) {
  return issue.id.startsWith(
    `${VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE}:`
  );
}

function isMissingRequiredField(
  node: FlowNodeDocument,
  fieldKey: string
): boolean {
  if (fieldKey === 'alias') {
    return node.alias.trim().length === 0;
  }

  if (fieldKey.startsWith('config.')) {
    if (fieldKey === 'config.model_provider') {
      const modelProvider = getLlmModelProvider(node.config);
      return (
        modelProvider.provider_code.trim().length === 0 ||
        modelProvider.model_id.trim().length === 0
      );
    }

    const configValue = node.config[fieldKey.slice('config.'.length)];

    if (typeof configValue === 'string') {
      return configValue.trim().length === 0;
    }

    return configValue === undefined || configValue === null;
  }

  if (fieldKey.startsWith('outputs.')) {
    const outputKey = fieldKey.slice('outputs.'.length);
    const output = node.outputs.find((item) => item.key === outputKey);

    return !output || output.title.trim().length === 0;
  }

  if (!fieldKey.startsWith('bindings.')) {
    return false;
  }

  const binding = node.bindings[fieldKey.slice('bindings.'.length)];

  if (!binding) {
    return true;
  }

  switch (binding.kind) {
    case 'templated_text':
      return binding.value.trim().length === 0;
    case 'i18n_text':
      return !validateI18nTextRef(binding.value).ok;
    case 'selector':
      return binding.value.length === 0;
    case 'selector_list':
      return (
        binding.value.length === 0 ||
        binding.value.some((selector) => !selectorHasRequiredInput(selector))
      );
    case 'variable_groups':
      return (
        binding.value.length === 0 ||
        binding.value.some(
          (group) =>
            group.key.trim().length === 0 ||
            group.candidates.length === 0 ||
            group.candidates.some(
              (selector) => !selectorHasRequiredInput(selector)
            )
        )
      );
    case 'data_model_query':
      return false;
    case 'prompt_messages':
      return binding.value.length === 0;
    case 'named_bindings':
      return binding.value.length === 0;
    case 'condition_group':
      return binding.value.conditions.length === 0;
    case 'if_else_branches':
      return ifElseBranchesMissingRequiredInput(binding.value.branches);
    case 'state_write':
      if (node.type === 'variable_assigner') {
        return (
          binding.value.length === 0 ||
          binding.value.some(
            (entry) =>
              entry.path.length < 2 ||
              entry.path.some((segment) => segment.trim().length === 0) ||
              !entry.value
          )
        );
      }

      return (
        binding.value.length === 0 ||
        binding.value.some(
          (entry) =>
            entry.path.length < 2 ||
            entry.path.some((segment) => segment.trim().length === 0) ||
            !entry.source ||
            entry.source.length < 2 ||
            entry.source.some((segment) => segment.trim().length === 0)
        )
      );
  }
}

function conditionGroupHasCompleteRules(
  group: FlowConditionGroupDocument
): boolean {
  return (
    group.conditions.length > 0 &&
    group.conditions.every((condition) => {
      if (isConditionGroup(condition)) {
        return conditionGroupHasCompleteRules(condition);
      }

      return (
        isConditionRule(condition) && conditionRuleHasRequiredInput(condition)
      );
    })
  );
}

function selectorHasRequiredInput(selector: string[]): boolean {
  return (
    selector.length >= 2 &&
    selector.every((segment) => segment.trim().length > 0)
  );
}

function conditionRuleHasRequiredInput(
  rule: FlowConditionRuleDocument
): boolean {
  if (!selectorHasRequiredInput(rule.left)) {
    return false;
  }

  if (rule.comparator === 'exists' || rule.comparator === 'empty') {
    return true;
  }

  if (!rule.right) {
    return false;
  }

  return (
    rule.right.kind !== 'selector' ||
    selectorHasRequiredInput(rule.right.selector)
  );
}

function ifElseBranchesMissingRequiredInput(
  branches: IfElseBranchDocument[]
): boolean {
  return (
    !branches.some((branch) => branch.kind === 'if') ||
    !branches.some((branch) => branch.kind === 'else') ||
    branches.some(
      (branch) =>
        branch.kind !== 'else' &&
        (!branch.condition || !conditionGroupHasCompleteRules(branch.condition))
    )
  );
}

function createNodeRuleValues(node: FlowNodeDocument): Record<string, unknown> {
  return {
    ...node,
    config: {
      ...node.config,
      output_contract: node.outputs
    }
  };
}

function isFieldVisibleForNode(
  node: FlowNodeDocument,
  field: NodeDefinitionField
) {
  return evaluateSchemaRule(field.visibleWhen, {
    values: createNodeRuleValues(node),
    capabilities: []
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function getAllowedPluginNodeOutputKeys(
  node: FlowNodeDocument
): Set<string> | null {
  const rawSnapshot = node.output_schema_snapshot;

  if (!isRecord(rawSnapshot) || !Array.isArray(rawSnapshot.outputs)) {
    return null;
  }

  const allowed = rawSnapshot.outputs
    .map((entry) => {
      if (!isRecord(entry)) {
        return null;
      }

      if (typeof entry.key !== 'string') {
        return null;
      }

      const outputKey = entry.key.trim();

      return outputKey.length > 0 ? outputKey : null;
    })
    .filter((key): key is string => key !== null);

  return new Set(allowed);
}

function collectBindingSelectors(binding: FlowBinding): string[][] {
  switch (binding.kind) {
    case 'templated_text':
      return parseTemplateSelectorTokens(binding.value);
    case 'i18n_text':
      return [];
    case 'selector':
      return [binding.value];
    case 'selector_list':
      return binding.value;
    case 'variable_groups':
      return binding.value.flatMap((group) => group.candidates);
    case 'prompt_messages':
      return binding.value.flatMap((message) =>
        parseTemplateSelectorTokens(message.content.value)
      );
    case 'named_bindings':
      return extractNamedBindingSelectors(binding.value);
    case 'condition_group':
      return collectConditionSelectors(binding.value);
    case 'if_else_branches':
      return collectIfElseBranchSelectors(binding.value.branches);
    case 'state_write':
      return binding.value.flatMap((entry) => [
        ...(entry.source ? [entry.source] : []),
        ...collectStateWriteValueSelectors(entry.value)
      ]);
    case 'data_model_query':
      return extractDataModelQuerySelectors(binding.value);
  }
}

function validateVariableAggregatorGroups(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument,
  document: FlowAuthoringDocument,
  environmentVariables: AgentFlowEnvironmentVariable[],
  workflowTriggerContext: unknown
) {
  const binding = node.bindings.groups;

  if (
    node.type !== 'variable_aggregator' ||
    binding?.kind !== 'variable_groups'
  ) {
    return;
  }

  const options = listVisibleSelectorOptions(
    document,
    node.id,
    environmentVariables,
    workflowTriggerContext
  );
  const seenGroupKeys = new Set<string>();
  const outputsMatchGroups =
    node.outputs.length === binding.value.length &&
    binding.value.every((group, index) => {
      const output = node.outputs[index];

      return (
        output?.key === group.key &&
        output.title === group.key &&
        output.valueType === group.valueType
      );
    });

  if (!outputsMatchGroups) {
    pushFieldIssue(
      issues,
      node,
      'bindings.groups',
      i18nText('agentFlow', 'auto.variable_group_outputs_out_of_sync'),
      i18nText('agentFlow', 'auto.variable_group_outputs_must_match_groups')
    );
  }

  for (const [groupIndex, group] of binding.value.entries()) {
    if (seenGroupKeys.has(group.key)) {
      pushFieldIssue(
        issues,
        node,
        'bindings.groups',
        i18nText('agentFlow', 'auto.duplicate_variable_group'),
        i18nText('agentFlow', 'auto.variable_group_keys_must_unique')
      );
    }
    seenGroupKeys.add(group.key);

    for (const [candidateIndex, selector] of group.candidates.entries()) {
      if (!selectorHasRequiredInput(selector)) {
        continue;
      }

      const selectedOption = options.find(
        (option) =>
          option.value.length === selector.length &&
          option.value.every((segment, index) => segment === selector[index])
      );
      const declaredType = selectedOption?.valueType.startsWith('array[')
        ? 'array'
        : selectedOption?.valueType;

      if (selectedOption && declaredType !== group.valueType) {
        issues.push({
          id: `${VARIABLE_AGGREGATOR_CANDIDATE_TYPE_MISMATCH_ISSUE_CODE}:${node.id}:${groupIndex}:${candidateIndex}`,
          scope: 'field',
          level: 'error',
          nodeId: node.id,
          sectionKey: 'inputs',
          fieldKey: 'bindings.groups',
          title: i18nText(
            'agentFlow',
            'auto.variable_group_candidate_incompatible'
          ),
          message: i18nText(
            'agentFlow',
            'auto.variable_group_candidate_type_mismatch',
            { value1: group.valueType }
          )
        });
      }
    }
  }
}

function collectStateWriteValueSelectors(
  value: Extract<FlowBinding, { kind: 'state_write' }>['value'][number]['value']
): string[][] {
  if (!value) {
    return [];
  }

  if (value.kind === 'selector') {
    return [value.selector];
  }

  if (value.kind === 'templated_text') {
    return parseTemplateSelectorTokens(value.value);
  }

  return [];
}

function collectVisibleInternalToolBranchNodeIds(
  startNodeId: string,
  nodeById: Map<string, FlowNodeDocument>,
  edges: FlowEdgeDocument[]
): Set<string> {
  const visited = new Set<string>();
  const queue = [startNodeId];

  while (queue.length > 0) {
    const nodeId = queue.shift();

    if (!nodeId || visited.has(nodeId)) {
      continue;
    }

    visited.add(nodeId);

    const node = nodeById.get(nodeId);
    if (node?.type === 'tool_result') {
      continue;
    }

    for (const edge of edges) {
      if (edge.source !== nodeId) {
        continue;
      }

      if (
        edge.sourceHandle === ERROR_BRANCH_SOURCE_HANDLE ||
        isLlmToolSourceHandle(edge.sourceHandle)
      ) {
        continue;
      }

      queue.push(edge.target);
    }
  }

  return visited;
}

function validateVisibleInternalLlmToolBranches(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument,
  nodeById: Map<string, FlowNodeDocument>,
  edges: FlowEdgeDocument[]
) {
  if (node.type !== 'llm' || !getLlmVisibleInternalToolsEnabled(node.config)) {
    return;
  }

  for (const [toolIndex, tool] of getLlmVisibleInternalTools(
    node.config
  ).entries()) {
    const allowInternalLlm =
      getLlmToolInternalLlmNodePolicy(tool) === 'allowed';
    const connectorId = tool.connector_id || tool.tool_name;
    const sourceHandle = createLlmToolSourceHandleId(connectorId);
    const connectorEdges = edges.filter(
      (edge) => edge.source === node.id && edge.sourceHandle === sourceHandle
    );

    for (const edge of connectorEdges) {
      const reachableNodeIds = collectVisibleInternalToolBranchNodeIds(
        edge.target,
        nodeById,
        edges
      );
      const toolResultNodeIds = [...reachableNodeIds].filter(
        (nodeId) => nodeById.get(nodeId)?.type === 'tool_result'
      );

      if (toolResultNodeIds.length === 0) {
        pushFieldIssue(
          issues,
          node,
          'config.visible_internal_llm_tools_enabled',
          i18nText('agentFlow', 'auto.tool_result_required'),
          i18nText('agentFlow', 'auto.tool_result_required'),
          'inputs'
        );
      } else if (toolResultNodeIds.length > 1) {
        pushFieldIssue(
          issues,
          node,
          'config.visible_internal_llm_tools_enabled',
          i18nText('agentFlow', 'auto.tool_result_duplicate'),
          i18nText('agentFlow', 'auto.tool_result_duplicate'),
          'inputs'
        );
      }

      if (!allowInternalLlm) {
        const reachesLlmNode = [...reachableNodeIds].some(
          (nodeId) => nodeId !== node.id && nodeById.get(nodeId)?.type === 'llm'
        );

        if (reachesLlmNode) {
          pushFieldIssue(
            issues,
            node,
            `config.visible_internal_llm_tools.${toolIndex}.internal_llm_node_policy`,
            i18nText('agentFlow', 'auto.internal_llm_node_policy'),
            i18nText('agentFlow', 'auto.internal_llm_node_forbidden_message'),
            'inputs'
          );
        }
      }
    }
  }
}

function isRuntimeSelectorSource(source: string) {
  return (
    source === systemVariableNodeId ||
    source === environmentVariableNodeId ||
    source === conversationVariableNodeId ||
    source === workflowTriggerVariableNodeId
  );
}

function validateVariableAssignmentOperations(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument,
  document: FlowAuthoringDocument
) {
  const binding = node.bindings.operations;

  if (node.type !== 'variable_assigner' || binding?.kind !== 'state_write') {
    return;
  }

  const conversationVariableNames = new Set(
    listConversationVariables(document).map((variable) => variable.name)
  );

  for (const operation of binding.value) {
    const [namespace, targetName] = operation.path;

    if (
      namespace !== conversationVariableNodeId ||
      !targetName ||
      !conversationVariableNames.has(targetName)
    ) {
      pushFieldIssue(
        issues,
        node,
        'bindings.operations',
        i18nText('agentFlow', 'auto.variable_assignment_target_invalid'),
        i18nText(
          'agentFlow',
          'auto.variable_assignment_target_must_be_defined_conversation'
        )
      );
      return;
    }
  }
}

function getAllowedPublicOutputKeysForNode(
  node: FlowNodeDocument
): Set<string> | null {
  if (node.type === 'plugin_node' && hasPluginContributionRef(node)) {
    return getAllowedPluginNodeOutputKeys(node);
  }

  if (node.type === 'llm') {
    return new Set(getLlmNodeOutputs(node.config).map((output) => output.key));
  }

  if (node.type === 'code') {
    return new Set(node.outputs.map((output) => output.key));
  }

  if (node.type === 'variable_aggregator') {
    return new Set(node.outputs.map((output) => output.key));
  }

  const contract = getBuiltinNodeRuntimeContract(node.type);

  if (!contract) {
    return null;
  }

  return new Set(contract.defaults.outputs.map((output) => output.key));
}

interface AnswerPresentationReference {
  nodeId: string;
  outputKey: string;
}

function collectAnswerPresentationReferences(
  node: FlowNodeDocument
): AnswerPresentationReference[] {
  const binding = node.bindings.answer_template;

  if (!binding) {
    return [];
  }

  const selectors =
    binding.kind === 'selector'
      ? [binding.value]
      : binding.kind === 'templated_text'
        ? parseTemplateSelectorTokens(binding.value)
        : [];

  return selectors
    .filter((selector) => selector.length >= 2)
    .map((selector) => ({
      nodeId: selector[0],
      outputKey: selector.slice(1).join('.')
    }));
}

function buildNodeDependencyMap(
  document: FlowAuthoringDocument,
  nodeIds: Set<string>
): Map<string, Set<string>> {
  const dependencies = new Map<string, Set<string>>();

  for (const node of document.graph.nodes) {
    dependencies.set(node.id, new Set());
  }

  for (const edge of document.graph.edges) {
    if (!nodeIds.has(edge.source) || !nodeIds.has(edge.target)) {
      continue;
    }
    dependencies.get(edge.target)?.add(edge.source);
  }

  for (const node of document.graph.nodes) {
    const nodeDependencies = dependencies.get(node.id);
    if (!nodeDependencies) {
      continue;
    }

    for (const [, binding] of getActiveNodeBindings(node)) {
      for (const selector of collectBindingSelectors(binding)) {
        const sourceNodeId = selector[0] ?? '';
        if (
          selector.length >= 2 &&
          !isRuntimeSelectorSource(sourceNodeId) &&
          nodeIds.has(sourceNodeId) &&
          sourceNodeId !== node.id
        ) {
          nodeDependencies.add(sourceNodeId);
        }
      }
    }
  }

  return dependencies;
}

function nodeDependsOn(
  dependencies: Map<string, Set<string>>,
  nodeId: string,
  dependencyNodeId: string
): boolean {
  const stack = [nodeId];
  const visited = new Set<string>();

  while (stack.length > 0) {
    const current = stack.pop();
    if (!current || visited.has(current)) {
      continue;
    }
    visited.add(current);

    for (const dependency of dependencies.get(current) ?? []) {
      if (dependency === dependencyNodeId) {
        return true;
      }
      stack.push(dependency);
    }
  }

  return false;
}

function formatAnswerPresentationReference(
  reference: AnswerPresentationReference,
  nodeById: Map<string, FlowNodeDocument>
): string {
  const node = nodeById.get(reference.nodeId);
  const nodeLabel = node?.alias.trim() || reference.nodeId;

  return `${nodeLabel}.${reference.outputKey}`;
}

function validateAnswerPresentationReferences(
  issues: AgentFlowIssue[],
  answerNode: FlowNodeDocument,
  nodeById: Map<string, FlowNodeDocument>,
  dependencies: Map<string, Set<string>>
) {
  const references = collectAnswerPresentationReferences(answerNode);
  const seen = new Set<string>();

  for (const reference of references) {
    const key = `${reference.nodeId}.${reference.outputKey}`;
    if (!seen.has(key)) {
      seen.add(key);
      continue;
    }

    pushFieldIssue(
      issues,
      answerNode,
      'bindings.answer_template',
      i18nText('agentFlow', 'auto.answer_repeated_reference_output_variable'),
      i18nText(
        'agentFlow',
        'auto.same_output_variable_referenced_answer_template',
        { value1: formatAnswerPresentationReference(reference, nodeById) }
      )
    );
  }

  for (let index = 0; index < references.length; index += 1) {
    const current = references[index];
    for (const later of references.slice(index + 1)) {
      if (!nodeDependsOn(dependencies, current.nodeId, later.nodeId)) {
        continue;
      }

      pushFieldIssue(
        issues,
        answerNode,
        'bindings.answer_template',
        i18nText(
          'agentFlow',
          'auto.answer_display_order_violates_execution_dependencies'
        ),
        i18nText(
          'agentFlow',
          'auto.template_puts_front_former_depends_execution_result_latter_adjust_answer',
          {
            value1: formatAnswerPresentationReference(current, nodeById),
            value2: formatAnswerPresentationReference(later, nodeById)
          }
        )
      );
      break;
    }
  }
}

function validateTerminalTopology(
  issues: AgentFlowIssue[],
  document: FlowAuthoringDocument
) {
  for (const node of document.graph.nodes.filter(
    (candidate) => candidate.type === 'answer'
  )) {
    if (document.graph.edges.some((edge) => edge.source === node.id)) {
      issues.push({
        id: `${node.id}-terminal-outgoing-edge`,
        scope: 'node',
        level: 'error',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText('agentFlow', 'auto.terminal_connection_invalid'),
        message: i18nText(
          'agentFlow',
          'auto.terminal_connection_invalid_message'
        )
      });
    }
  }
}

export function validateDocument(
  document: FlowAuthoringDocument,
  providerOptions?: AgentFlowModelProviderOptions | null,
  environmentVariables: AgentFlowEnvironmentVariable[] = [],
  workflowTriggerContext: unknown = null
): AgentFlowIssue[] {
  const issues: AgentFlowIssue[] = validateAuthoringDocument(document);
  const nodeIds = new Set(document.graph.nodes.map((node) => node.id));
  const nodeById = new Map(document.graph.nodes.map((node) => [node.id, node]));
  const dependencies = buildNodeDependencyMap(document, nodeIds);
  const startNodes = document.graph.nodes.filter(
    (node) => node.type === 'start'
  );
  const answerNodes = document.graph.nodes.filter(
    (node) => node.type === 'answer'
  );
  const providerMap = new Map(
    (providerOptions?.providers ?? []).map((provider) => [
      provider.provider_code,
      provider
    ])
  );

  if (startNodes.length !== 1) {
    issues.push({
      id: 'global-start-count',
      scope: 'global',
      level: 'error',
      nodeId: null,
      sectionKey: null,
      fieldKey: null,
      title: i18nText('agentFlow', 'auto.number_start_nodes_illegal'),
      message: i18nText(
        'agentFlow',
        'auto.each_draft_must_retain_exactly_one_start_node'
      )
    });
  }

  if (answerNodes.length === 0) {
    issues.push({
      id: 'global-answer-missing',
      scope: 'global',
      level: 'error',
      nodeId: null,
      sectionKey: null,
      fieldKey: null,
      title: i18nText('agentFlow', 'auto.missing_answer_node'),
      message: i18nText(
        'agentFlow',
        'auto.first_version_agentflow_requires_least_one_answer_node_dialogue_output'
      )
    });
  }

  validateTerminalTopology(issues, document);

  for (const edge of document.graph.edges) {
    if (nodeIds.has(edge.source) && nodeIds.has(edge.target)) {
      const sourceNode = nodeById.get(edge.source);
      const sourceBranches =
        sourceNode?.type === 'if_else'
          ? getIfElseBranchesFromBindings(sourceNode.bindings)
          : null;
      const usesErrorBranchSourceHandle =
        edge.sourceHandle === ERROR_BRANCH_SOURCE_HANDLE;

      if (
        usesErrorBranchSourceHandle &&
        (!sourceNode || !nodeUsesErrorBranch(sourceNode))
      ) {
        issues.push({
          id: `${edge.id}-invalid-source-handle`,
          scope: 'node',
          level: 'error',
          nodeId: edge.source,
          sectionKey: 'policy',
          fieldKey: 'config.error_policy',
          title: i18nText('agentFlow', 'auto.branch_connection_invalid'),
          message: i18nText(
            'agentFlow',
            'auto.branch_connection_invalid_message'
          )
        });
      }

      if (sourceBranches && !usesErrorBranchSourceHandle) {
        const branchHandles = new Set(
          sourceBranches.map((branch) => branch.sourceHandle)
        );

        if (!edge.sourceHandle || !branchHandles.has(edge.sourceHandle)) {
          issues.push({
            id: `${edge.id}-invalid-source-handle`,
            scope: 'node',
            level: 'error',
            nodeId: edge.source,
            sectionKey: 'inputs',
            fieldKey: 'bindings.branches',
            title: i18nText('agentFlow', 'auto.branch_connection_invalid'),
            message: i18nText(
              'agentFlow',
              'auto.branch_connection_invalid_message'
            )
          });
        }
      }

      continue;
    }

    if (nodeIds.has(edge.source)) {
      issues.push({
        id: `${edge.id}-dangling`,
        scope: 'node',
        level: 'warning',
        nodeId: edge.source,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText(
          'agentFlow',
          'auto.node_connection_points_invalid_target'
        ),
        message: i18nText('agentFlow', 'auto.connection_node_deleted_node')
      });
    }
  }

  for (const node of document.graph.nodes) {
    const definition = nodeDefinitions[node.type];

    if (definition) {
      for (const section of definition.sections) {
        for (const field of section.fields) {
          if (
            field.required &&
            isFieldVisibleForNode(node, field) &&
            isMissingRequiredField(node, field.key)
          ) {
            if (node.type === 'llm' && field.key === 'config.model_provider') {
              const modelProvider = getLlmModelProvider(node.config);
              const providerMissing =
                modelProvider.provider_code.trim().length === 0;

              pushFieldIssue(
                issues,
                node,
                field.key,
                providerMissing
                  ? i18nText('agentFlow', 'auto.llm_missing_model_supplier')
                  : i18nText('agentFlow', 'auto.llm_missing_model'),
                providerMissing
                  ? i18nText('agentFlow', 'auto.select_model_supplier_first')
                  : i18nText('agentFlow', 'auto.select_model_first')
              );
              continue;
            }

            pushFieldIssue(
              issues,
              node,
              field.key,
              i18nText('agentFlow', 'auto.not_configured', {
                value1: field.label
              }),
              i18nText('agentFlow', 'auto.please_complete_first', {
                value1: field.label
              })
            );
          }
        }
      }
    }

    if (node.type === 'llm') {
      validateVisibleInternalLlmToolBranches(
        issues,
        node,
        nodeById,
        document.graph.edges
      );

      const modelProvider = getLlmModelProvider(node.config);
      const providerCode = modelProvider.provider_code.trim();
      const model = modelProvider.model_id.trim();

      if (providerOptions && providerCode.length > 0) {
        const provider = providerMap.get(providerCode);

        if (!provider) {
          pushFieldIssue(
            issues,
            node,
            'config.model_provider',
            i18nText('agentFlow', 'auto.llm_model_provider_unavailable'),
            i18nText('agentFlow', 'auto.model_provider_exist_ready_access'),
            'inputs'
          );
        } else if (model.length > 0) {
          const matchingModelGroups = provider.model_groups.filter(
            (group) => group.model_id === model
          );

          if (matchingModelGroups.length === 0) {
            pushFieldIssue(
              issues,
              node,
              'config.model_provider',
              i18nText('agentFlow', 'auto.llm_model_available'),
              i18nText(
                'agentFlow',
                'auto.model_belong_selected_supplier_s_list_active_models'
              )
            );
          }
        }
      }
    }

    if (node.type === 'unresolved_node') {
      issues.push({
        id: `${node.id}-unresolved-node`,
        scope: 'node',
        level: 'error',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText('agentFlow', 'auto.unresolved_node'),
        message: i18nText(
          'agentFlow',
          'auto.unresolved_node_definition_summary'
        )
      });
    }

    if (node.type === 'plugin_node' && !hasPluginContributionRef(node)) {
      issues.push({
        id: `${node.id}-plugin-ref-missing`,
        scope: 'node',
        level: 'error',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText(
          'agentFlow',
          'auto.plugin_node_missing_contribution_identity'
        ),
        message: i18nText(
          'agentFlow',
          'auto.plugin_node_missing_plugin_id_plugin_version_contribution_code_node'
        )
      });
    }

    validateVariableAssignmentOperations(issues, node, document);
    validateVariableAggregatorGroups(
      issues,
      node,
      document,
      environmentVariables,
      workflowTriggerContext
    );

    if (node.type === 'answer') {
      validateAnswerPresentationReferences(
        issues,
        node,
        nodeById,
        dependencies
      );
    }

    for (const [bindingKey, bindingValue] of getActiveNodeBindings(node)) {
      const selectors = collectBindingSelectors(bindingValue);

      for (const selector of selectors) {
        const sourceNodeId = selector[0] ?? '';

        if (
          selector.length === 0 ||
          isSelectorVisible(
            document,
            node.id,
            selector,
            environmentVariables,
            workflowTriggerContext
          )
        ) {
          continue;
        }

        if (
          selector.length >= 2 &&
          !isRuntimeSelectorSource(sourceNodeId) &&
          !nodeIds.has(sourceNodeId)
        ) {
          pushFieldIssue(
            issues,
            node,
            `bindings.${bindingKey}`,
            i18nText('agentFlow', 'auto.binding_reference_node_exist'),
            i18nText('agentFlow', 'auto.binding_refers_output_deleted_node', {
              value1: sourceNodeId
            })
          );
          continue;
        }

        pushFieldIssue(
          issues,
          node,
          `bindings.${bindingKey}`,
          i18nText('agentFlow', 'auto.binding_reference_visible'),
          i18nText(
            'agentFlow',
            'auto.binding_refers_output_connected_upstream_link'
          )
        );
      }
    }

    const seenOutputKeys = new Set<string>();
    const allowedPublicOutputKeys = getAllowedPublicOutputKeysForNode(node);

    if (node.type === 'code') {
      const language =
        typeof node.config.language === 'string'
          ? node.config.language.trim()
          : '';

      if (language.length > 0 && language !== 'javascript') {
        pushFieldIssue(
          issues,
          node,
          'config.language',
          i18nText('agentFlow', 'auto.unsupported_runtime_language'),
          i18nText('agentFlow', 'auto.version_supports_javascript')
        );
      }

      validateCodeNamedBindings(
        issues,
        node,
        document,
        environmentVariables,
        workflowTriggerContext
      );
    }

    if (node.type === 'code' && node.outputs.length === 0) {
      pushFieldIssue(
        issues,
        node,
        'config.output_contract',
        i18nText('agentFlow', 'auto.code_output_contract_empty'),
        i18nText(
          'agentFlow',
          'auto.code_node_needs_retain_least_one_output_variable_downstream_reference'
        )
      );
    }

    for (const output of node.outputs) {
      const outputKey = output.key.trim();
      const outputValueType = output.valueType.trim();

      if (outputKey.length === 0) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.output_variable_name_configured'),
          i18nText('agentFlow', 'auto.variable_names_output_contracts_empty')
        );
        continue;
      }

      if (seenOutputKeys.has(outputKey)) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.duplicate_output_contract'),
          i18nText(
            'agentFlow',
            'auto.variable_names_output_contracts_must_unique'
          )
        );
        continue;
      }

      seenOutputKeys.add(outputKey);

      const publicOutputKeyValidation = validatePublicOutputKey(outputKey);

      if (!publicOutputKeyValidation.ok) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.output_variable_names_reserved'),
          i18nText(
            'agentFlow',
            'auto.variable_names_output_contract_system_reserved_fields_use_business_field'
          )
        );
        continue;
      }

      if (node.type === 'code' && !isOutputVariableKeyAllowed(outputKey)) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.output_variable_name_format_invalid'),
          i18nText('agentFlow', 'auto.output_variable_name_format_message')
        );
      }

      if (node.type === 'code' && output.title.trim() !== outputKey) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.output_variable_title_mismatch'),
          i18nText(
            'agentFlow',
            'auto.code_output_variable_title_must_match_name'
          )
        );
      }

      if (allowedPublicOutputKeys && !allowedPublicOutputKeys.has(outputKey)) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          i18nText('agentFlow', 'auto.output_variable_name_unknown'),
          i18nText(
            'agentFlow',
            'auto.variable_name_output_contract_belong_node_runtime_contract'
          )
        );
      }

      if (
        output.jsonSchema !== undefined &&
        !outputTypeSupportsJsonSchema(outputValueType)
      ) {
        pushFieldIssue(
          issues,
          node,
          'config.output_contract',
          'JSON Schema 类型不匹配',
          '只有 Object 和 Array 输出变量可以启用 JSON Schema 校验。'
        );
      }
    }

    if (
      node.type !== 'start' &&
      node.type !== 'workflow_start' &&
      !document.graph.edges.some(
        (edge) => edge.target === node.id && nodeIds.has(edge.source)
      )
    ) {
      issues.push({
        id: `${node.id}-orphan`,
        scope: 'node',
        level: 'warning',
        nodeId: node.id,
        sectionKey: 'basics',
        fieldKey: null,
        title: i18nText('agentFlow', 'auto.yet_connected_main_link', {
          value1: node.alias
        }),
        message: i18nText('agentFlow', 'auto.node_any_valid_incoming_edges')
      });
    }
  }

  return [...new Map(issues.map((issue) => [issue.id, issue])).values()];
}
