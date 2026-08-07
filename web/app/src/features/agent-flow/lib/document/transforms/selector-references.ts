import type {
  FlowAuthoringDocument,
  FlowBinding,
  FlowConditionExpressionDocument,
  FlowConditionGroupDocument,
  FlowConditionRuleDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import { remapDataModelQueryBinding } from '../../data-model-query-binding';
import { isConditionGroup, isConditionRule } from '../../if-else-branches';
import { remapNamedBindingEntry } from '../../named-binding-expressions';
import {
  remapTemplateSelectorTokens,
  type SelectorReferenceTransform
} from '../../template-binding';

function transformConditionRuleSelectorReferences(
  rule: FlowConditionRuleDocument,
  transformSelector: SelectorReferenceTransform
): FlowConditionRuleDocument {
  return {
    ...rule,
    left: transformSelector(rule.left),
    right:
      rule.right?.kind === 'selector'
        ? {
            ...rule.right,
            selector: transformSelector(rule.right.selector)
          }
        : rule.right
  };
}

function transformConditionExpressionSelectorReferences(
  condition: FlowConditionExpressionDocument,
  transformSelector: SelectorReferenceTransform
): FlowConditionExpressionDocument {
  if (isConditionGroup(condition)) {
    return transformConditionGroupSelectorReferences(
      condition,
      transformSelector
    );
  }

  return isConditionRule(condition)
    ? transformConditionRuleSelectorReferences(condition, transformSelector)
    : condition;
}

function transformConditionGroupSelectorReferences(
  group: FlowConditionGroupDocument,
  transformSelector: SelectorReferenceTransform
): FlowConditionGroupDocument {
  return {
    ...group,
    conditions: group.conditions.map((condition) =>
      transformConditionExpressionSelectorReferences(
        condition,
        transformSelector
      )
    )
  };
}

function transformStateWriteValueSelectorReferences(
  value: Extract<
    FlowBinding,
    { kind: 'state_write' }
  >['value'][number]['value'],
  transformSelector: SelectorReferenceTransform
) {
  if (!value) {
    return value;
  }

  if (value.kind === 'selector') {
    return {
      ...value,
      selector: transformSelector(value.selector)
    };
  }

  if (value.kind === 'templated_text') {
    return {
      ...value,
      value: remapTemplateSelectorTokens(value.value, transformSelector)
    };
  }

  return value;
}

export function transformFlowBindingSelectorReferences(
  binding: FlowBinding,
  transformSelector: SelectorReferenceTransform
): FlowBinding {
  switch (binding.kind) {
    case 'templated_text':
      return {
        ...binding,
        value: remapTemplateSelectorTokens(binding.value, transformSelector)
      };
    case 'i18n_text':
      return { ...binding, value: { ...binding.value } };
    case 'selector':
      return {
        ...binding,
        value: transformSelector(binding.value)
      };
    case 'selector_list':
      return {
        ...binding,
        value: binding.value.map(transformSelector)
      };
    case 'variable_groups':
      return {
        ...binding,
        value: binding.value.map((group) => ({
          ...group,
          candidates: group.candidates.map(transformSelector)
        }))
      };
    case 'prompt_messages':
      return {
        ...binding,
        value: binding.value.map((message) => ({
          ...message,
          content: {
            ...message.content,
            value: remapTemplateSelectorTokens(
              message.content.value,
              transformSelector
            )
          }
        }))
      };
    case 'named_bindings':
      return {
        ...binding,
        value: binding.value.map((entry) =>
          remapNamedBindingEntry(entry, transformSelector)
        )
      };
    case 'condition_group':
      return {
        ...binding,
        value: transformConditionGroupSelectorReferences(
          binding.value,
          transformSelector
        )
      };
    case 'if_else_branches':
      return {
        ...binding,
        value: {
          branches: binding.value.branches.map((branch) => ({
            ...branch,
            condition: branch.condition
              ? transformConditionGroupSelectorReferences(
                  branch.condition,
                  transformSelector
                )
              : undefined
          }))
        }
      };
    case 'state_write':
      return {
        ...binding,
        value: binding.value.map((entry) => ({
          ...entry,
          source: entry.source ? transformSelector(entry.source) : entry.source,
          value: transformStateWriteValueSelectorReferences(
            entry.value,
            transformSelector
          )
        }))
      };
    case 'data_model_query':
      return remapDataModelQueryBinding(binding, transformSelector);
  }
}

function transformProtocolContextSelectorReference(
  config: FlowNodeDocument['config'],
  transformSelector: SelectorReferenceTransform
) {
  const protocolContext = config.protocol_context;

  if (
    typeof protocolContext !== 'object' ||
    protocolContext === null ||
    Array.isArray(protocolContext) ||
    (protocolContext as { kind?: unknown }).kind !== 'selector' ||
    !Array.isArray((protocolContext as { value?: unknown }).value) ||
    !(protocolContext as { value: unknown[] }).value.every(
      (segment) => typeof segment === 'string'
    )
  ) {
    return config;
  }

  return {
    ...config,
    protocol_context: {
      ...(protocolContext as { kind: 'selector'; value: string[] }),
      value: transformSelector(
        (protocolContext as { kind: 'selector'; value: string[] }).value
      )
    }
  };
}

export function transformNodeSelectorReferences(
  node: FlowNodeDocument,
  transformSelector: SelectorReferenceTransform
): FlowNodeDocument {
  return {
    ...node,
    config: transformProtocolContextSelectorReference(
      node.config,
      transformSelector
    ),
    bindings: Object.fromEntries(
      Object.entries(node.bindings).map(([key, binding]) => [
        key,
        transformFlowBindingSelectorReferences(binding, transformSelector)
      ])
    )
  };
}

export function transformDocumentSelectorReferences(
  document: FlowAuthoringDocument,
  transformSelector: SelectorReferenceTransform
): FlowAuthoringDocument {
  return {
    ...document,
    graph: {
      ...document.graph,
      nodes: document.graph.nodes.map((node) =>
        transformNodeSelectorReferences(node, transformSelector)
      )
    }
  };
}
