import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';

import { i18nText } from '../../../../shared/i18n/text';
import {
  getNamedBindingExpression,
  isNamedBindingNameAllowed
} from '../named-binding-expressions';
import {
  listVisibleSelectorOptions,
  type FlowSelectorOption
} from '../selector-options';
import { parseTemplateSelectorTokens } from '../template-binding';
import type { AgentFlowEnvironmentVariable } from '../variables/application-environment-variables';
import { pushFieldIssue, type AgentFlowIssue } from './issues';

function isCodeNamedBindingValueCompatible(
  valueType: string | undefined,
  value: unknown
) {
  if (!valueType || valueType === 'unknown') {
    return true;
  }

  if (valueType === 'string') {
    return typeof value === 'string';
  }

  if (valueType === 'number') {
    return typeof value === 'number' && Number.isFinite(value);
  }

  if (valueType === 'boolean') {
    return typeof value === 'boolean';
  }

  return false;
}

function normalizeCodeBindingValueType(valueType: string | undefined) {
  return valueType?.startsWith('array') ? 'array' : valueType;
}

function isCodeSelectorCompatible(
  selectorOptions: FlowSelectorOption[],
  selector: string[],
  valueType: string | undefined
) {
  const normalizedValueType = normalizeCodeBindingValueType(valueType);

  if (
    !normalizedValueType ||
    normalizedValueType === 'string' ||
    normalizedValueType === 'unknown'
  ) {
    return true;
  }

  const option = selectorOptions.find(
    (candidate) =>
      candidate.value.length === selector.length &&
      candidate.value.every((segment, index) => selector[index] === segment)
  );

  if (!option) {
    return true;
  }

  const optionValueType = normalizeCodeBindingValueType(option.valueType);

  if (optionValueType === 'json') {
    return (
      normalizedValueType === 'json' ||
      normalizedValueType === 'object' ||
      normalizedValueType === 'array'
    );
  }

  return optionValueType === normalizedValueType;
}

function pushCodeNamedBindingValueTypeIssue(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument
) {
  pushFieldIssue(
    issues,
    node,
    'bindings.named_bindings',
    i18nText('agentFlow', 'auto.variable_value_match_type'),
    i18nText('agentFlow', 'auto.variable_value_match_type')
  );
}

export function validateCodeNamedBindings(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument,
  document: FlowAuthoringDocument,
  environmentVariables: AgentFlowEnvironmentVariable[],
  workflowTriggerContext: unknown = null
) {
  const binding = node.bindings.named_bindings;

  if (!binding || binding.kind !== 'named_bindings') {
    return;
  }

  const seenNames = new Set<string>();
  const selectorOptions = listVisibleSelectorOptions(
    document,
    node.id,
    environmentVariables,
    workflowTriggerContext
  );

  for (const entry of binding.value) {
    const parameterName = entry.name.trim();

    if (parameterName.length === 0) {
      pushFieldIssue(
        issues,
        node,
        'bindings.named_bindings',
        i18nText('agentFlow', 'auto.code_input_variable_name_empty'),
        i18nText('agentFlow', 'auto.enter_variable_name')
      );
      continue;
    }

    if (!isNamedBindingNameAllowed(parameterName)) {
      pushFieldIssue(
        issues,
        node,
        'bindings.named_bindings',
        i18nText('agentFlow', 'auto.code_input_variable_name_format_invalid'),
        i18nText('agentFlow', 'auto.code_input_variable_name_format_message')
      );
      continue;
    }

    if (seenNames.has(parameterName)) {
      pushFieldIssue(
        issues,
        node,
        'bindings.named_bindings',
        i18nText('agentFlow', 'auto.code_input_variable_name_duplicate'),
        i18nText('agentFlow', 'auto.code_input_variable_name_duplicate_message')
      );
      continue;
    }

    seenNames.add(parameterName);

    const expression = getNamedBindingExpression(entry);

    if (
      expression?.kind === 'selector' &&
      !isCodeSelectorCompatible(
        selectorOptions,
        expression.selector,
        entry.valueType
      )
    ) {
      pushCodeNamedBindingValueTypeIssue(issues, node);
    }

    if (
      expression?.kind === 'templated_text' &&
      entry.valueType !== undefined &&
      entry.valueType !== 'string' &&
      entry.valueType !== 'number'
    ) {
      pushCodeNamedBindingValueTypeIssue(issues, node);
    }

    if (expression?.kind === 'templated_text' && entry.valueType === 'number') {
      for (const selector of parseTemplateSelectorTokens(expression.value)) {
        if (!isCodeSelectorCompatible(selectorOptions, selector, 'number')) {
          pushCodeNamedBindingValueTypeIssue(issues, node);
          break;
        }
      }
    }

    if (
      expression?.kind === 'constant' &&
      !isCodeNamedBindingValueCompatible(entry.valueType, expression.value)
    ) {
      pushCodeNamedBindingValueTypeIssue(issues, node);
    }
  }
}
