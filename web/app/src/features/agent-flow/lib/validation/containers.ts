import type {
  FlowAuthoringDocument,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import { i18nText } from '../../../../shared/i18n/text';
import { pushFieldIssue, type AgentFlowIssue } from './issues';
import { listVisibleSelectorOptions } from '../selector-options';
import {
  loopVariableDefinitions,
  type LoopVariableDefinition
} from '../variables/container-variables';
import { getNamedBindingExpression } from '../named-binding-expressions';
import type { AgentFlowEnvironmentVariable } from '../variables/application-environment-variables';

function matchesConstantType(value: unknown, type: string) {
  if (type === 'array') return Array.isArray(value);
  if (type === 'object')
    return typeof value === 'object' && value !== null && !Array.isArray(value);
  return typeof value === type && (type !== 'number' || Number.isFinite(value));
}
export function validateContainerNode(
  issues: AgentFlowIssue[],
  node: FlowNodeDocument,
  document: FlowAuthoringDocument,
  environmentVariables: AgentFlowEnvironmentVariable[],
  workflowTriggerContext: unknown
) {
  if (node.type !== 'loop' && node.type !== 'iteration') return;
  const report = (path: string, message: string) =>
    pushFieldIssue(
      issues,
      node,
      path,
      i18nText('agentFlow', 'containers.invalid_settings'),
      message,
      path.startsWith('config.') && path !== 'config.loop_variables'
        ? 'policy'
        : 'inputs'
    );
  for (const key of [
    'round_timeout_ms',
    ...(node.type === 'loop' ? ['max_rounds'] : ['parallelism'])
  ]) {
    const value = node.config[key];
    if (key === 'round_timeout_ms' && value == null) continue;
    if (!Number.isSafeInteger(value) || Number(value) < 1)
      report(
        `config.${key}`,
        i18nText('agentFlow', 'containers.positive_integer_required')
      );
  }
  if (!['fail', 'break', 'continue'].includes(String(node.config.error_policy)))
    report(
      'config.error_policy',
      i18nText('agentFlow', 'containers.invalid_error_policy')
    );
  if (
    node.type === 'iteration' &&
    typeof node.config.flatten_output !== 'boolean'
  )
    report(
      'config.flatten_output',
      i18nText('agentFlow', 'containers.boolean_required')
    );
  for (const key of node.type === 'iteration'
    ? ['items', 'output']
    : ['output']) {
    const binding = node.bindings[key];
    if (node.type === 'loop' && !binding) continue;
    if (binding?.kind !== 'selector' || binding.value.length < 2) {
      report(
        `bindings.${key}`,
        i18nText('agentFlow', 'containers.selector_required')
      );
      continue;
    }
    const selectorKey = JSON.stringify(binding.value);
    const option = listVisibleSelectorOptions(
      document,
      node.id,
      environmentVariables,
      workflowTriggerContext,
      `bindings.${key}`
    ).find((option) => JSON.stringify(option.value) === selectorKey);
    if (
      (key === 'items' || node.config.flatten_output === true) &&
      option &&
      !option.valueType.startsWith('array') &&
      option.valueType !== 'json'
    )
      report(
        `bindings.${key}`,
        i18nText('agentFlow', 'containers.array_required')
      );
  }
  if (node.type !== 'loop') return;
  const variables = loopVariableDefinitions(node);
  const keys = new Set<string>();
  const variablesByKey = new Map<string, LoopVariableDefinition>();
  for (const variable of variables) {
    if (!variablesByKey.has(variable.key))
      variablesByKey.set(variable.key, variable);
    if (
      !variable.key.trim() ||
      variable.key.startsWith('__') ||
      keys.has(variable.key) ||
      !['string', 'number', 'boolean', 'object', 'array'].includes(
        variable.value_type
      )
    )
      report(
        'config.loop_variables',
        i18nText('agentFlow', 'containers.invalid_variable')
      );
    keys.add(variable.key);
  }
  for (const key of ['variables', 'variable_updates']) {
    const binding = node.bindings[key];
    const values = binding?.kind === 'named_bindings' ? binding.value : [];
    const names = new Set<string>();
    for (const entry of values) {
      const variable = variablesByKey.get(entry.name);
      const expression = getNamedBindingExpression(entry);
      if (
        !variable ||
        names.has(entry.name) ||
        !expression ||
        (expression.kind === 'constant' &&
          !matchesConstantType(expression.value, variable.value_type))
      )
        report(
          `bindings.${key}`,
          i18nText('agentFlow', 'containers.invalid_variable_value')
        );
      names.add(entry.name);
    }
    if (
      key === 'variables' &&
      variables.some((variable) => !names.has(variable.key))
    )
      report(
        'bindings.variables',
        i18nText('agentFlow', 'containers.initial_value_required')
      );
  }
}
