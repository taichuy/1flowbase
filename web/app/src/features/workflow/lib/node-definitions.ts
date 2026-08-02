import type {
  FlowNodeDocument,
  FlowNodeOutputDocument,
  NodeRuntimeUiContract
} from '@1flowbase/flow-schema';
import { DEFAULT_WORKFLOW_START_NODE_CONFIG } from '@1flowbase/flow-schema';

import { i18nText } from '../../../shared/i18n/text';
import {
  basicsPanelSection,
  createNodeRuntimeContract,
  panelField,
  panelSection
} from '../../agent-flow/lib/node-definitions/contracts';
import type { NodeDefinitionMeta } from '../../flow-editor/authoring/node-definition-types';
import { getStartInputFields } from '../../flow-editor/authoring/start-input-fields';
import { asWorkflowTriggerContext } from './trigger-context';

export function createWorkflowStartContract(): NodeRuntimeUiContract {
  return createNodeRuntimeContract({
    type: 'workflow_start',
    title: 'Workflow Start',
    description: i18nText('workflow', 'auto.workflow_start_description'),
    category: 'io',
    config: {
      input_fields: [...DEFAULT_WORKFLOW_START_NODE_CONFIG.input_fields],
      sync_timeout_ms: DEFAULT_WORKFLOW_START_NODE_CONFIG.sync_timeout_ms
    },
    outputs: [],
    panelSections: [
      panelSection('inputs', i18nText('workflow', 'auto.input_parameters'), [
        panelField({
          key: 'config.input_fields',
          title: i18nText('workflow', 'auto.input_parameters'),
          renderer: 'start_input_fields',
          valueType: 'array'
        })
      ])
    ]
  });
}

export function createWorkflowEndContract(): NodeRuntimeUiContract {
  return createNodeRuntimeContract({
    type: 'workflow_end',
    title: 'Workflow End',
    description: i18nText('workflow', 'auto.workflow_end_description'),
    category: 'io',
    config: {},
    outputs: [],
    panelSections: [
      basicsPanelSection,
      panelSection('outputs', i18nText('workflow', 'auto.return_fields'), [
        panelField({
          key: 'config.output_contract',
          title: i18nText('workflow', 'auto.return_fields'),
          renderer: 'output_contract_definition',
          valueType: 'array'
        })
      ])
    ]
  });
}

export const workflowStartNodeMeta: NodeDefinitionMeta = {
  summary: i18nText('workflow', 'auto.workflow_start_description'),
  helpHref: '/docs/workflow/nodes/workflow-start',
  flowRole: 'entry'
};

export const workflowEndNodeMeta: NodeDefinitionMeta = {
  summary: i18nText('workflow', 'auto.workflow_end_description'),
  helpHref: '/docs/workflow/nodes/workflow-end',
  flowRole: 'terminal'
};

export function getWorkflowStartNodeVariableOutputs(
  node: Pick<FlowNodeDocument, 'config' | 'outputs'>
): FlowNodeOutputDocument[] {
  if (node.outputs.length > 0) {
    throw new Error('Workflow start node outputs must be empty');
  }

  return getStartInputFields(node).map((field) => ({
    key: field.key,
    title: `input.${field.key}`,
    valueType: field.valueType
  }));
}

export function createWorkflowStartTriggerSummary(value: unknown) {
  const context = asWorkflowTriggerContext(value);

  if (!context?.triggerType) {
    return i18nText('workflow', 'auto.workflow_trigger_not_configured');
  }

  if (context.triggerType === 'schedule') {
    const schedule = context.schedule;

    if (!schedule) {
      return i18nText('workflow', 'auto.workflow_trigger_not_configured');
    }

    const status = schedule.enabled
      ? i18nText('workflow', 'auto.workflow_schedule_enabled')
      : i18nText('workflow', 'auto.workflow_schedule_disabled');

    return `${status} · ${schedule.cron} · ${schedule.timezone}`;
  }

  const extension = context.mapping?.extension;

  return extension
    ? `${extension.method} /api/ex/${extension.slug}`
    : i18nText('workflow', 'auto.workflow_trigger_not_configured');
}
