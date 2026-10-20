import type { FlowBinding } from '@1flowbase/flow-schema';

import { extractDataModelQuerySelectors } from '../data-model-query-binding';

import { extractNamedBindingSelectors } from '../named-binding-expressions';

import {
  collectConditionSelectors,
  collectIfElseBranchSelectors
} from '../if-else-branches';

import { workflowTriggerVariableNodeId } from '../variables/workflow-trigger-variables';
import { parseTemplateSelectorTokens } from '../template-binding';
import { environmentVariableNodeId } from '../variables/application-environment-variables';
import { conversationVariableNodeId } from '../variables/conversation-variables';
import { systemVariableNodeId } from '../variables/system-variables';

export function collectBindingSelectors(binding: FlowBinding): string[][] {
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

export function isRuntimeSelectorSource(source: string) {
  return (
    source === systemVariableNodeId ||
    source === environmentVariableNodeId ||
    source === conversationVariableNodeId ||
    source === workflowTriggerVariableNodeId
  );
}
