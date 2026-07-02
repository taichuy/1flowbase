import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import type {
  ApplicationApiMapping,
  WorkflowScheduleTrigger,
  WorkflowScheduleTriggerInput
} from '../api/public-api';
import { getStartInputFields } from '../../agent-flow/lib/variables/start-node-variables';

export type WorkflowTriggerType = 'extension' | 'schedule' | 'manual';
export type WorkflowExtensionHttpMethod =
  | 'GET'
  | 'POST'
  | 'PUT'
  | 'PATCH'
  | 'DELETE'
  | 'HEAD'
  | 'OPTIONS';
export type WorkflowExtensionResponseMode = 'sync' | 'async';
export type WorkflowExtensionParameterSource =
  | 'path'
  | 'query'
  | 'form'
  | 'body';

export interface WorkflowExtensionParameterFormValue {
  source: WorkflowExtensionParameterSource;
  name: string;
  target: string;
}

export interface WorkflowExtensionTargetOption {
  value: string;
  label: string;
}

export interface WorkflowTriggerFormValues {
  trigger_type: WorkflowTriggerType;
  extension_slug: string;
  extension_method: WorkflowExtensionHttpMethod;
  extension_response_mode: WorkflowExtensionResponseMode;
  extension_parameters: WorkflowExtensionParameterFormValue[];
  schedule_enabled: boolean;
  schedule_cron: string;
  schedule_timezone: string;
  schedule_input_payload: string;
}

export const WORKFLOW_EXTENSION_HTTP_METHODS: WorkflowExtensionHttpMethod[] = [
  'GET',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
  'HEAD',
  'OPTIONS'
];

export const WORKFLOW_EXTENSION_PARAMETER_SOURCES: WorkflowExtensionParameterSource[] =
  ['path', 'query', 'form', 'body'];
export const WORKFLOW_TRIGGER_TYPES: WorkflowTriggerType[] = [
  'extension',
  'schedule',
  'manual'
];

export const DEFAULT_WORKFLOW_TRIGGER_VALUES: WorkflowTriggerFormValues = {
  trigger_type: 'extension',
  extension_slug: '',
  extension_method: 'POST',
  extension_response_mode: 'sync',
  extension_parameters: WORKFLOW_EXTENSION_PARAMETER_SOURCES.map((source) => ({
    source,
    name: '',
    target: ''
  })),
  schedule_enabled: true,
  schedule_cron: '0 9 * * *',
  schedule_timezone: 'UTC',
  schedule_input_payload: '{}'
};

export function createDefaultWorkflowApiMapping(
  values: Pick<
    WorkflowTriggerFormValues,
    | 'extension_slug'
    | 'extension_method'
    | 'extension_response_mode'
    | 'extension_parameters'
  >
): ApplicationApiMapping {
  return {
    input: {
      query_target: 'node-workflow-start.query',
      model_target: null,
      inputs_target: 'node-workflow-start.inputs',
      history_target: null,
      attachments_target: null
    },
    output: {
      answer_selector: null,
      usage_selector: null,
      files_selector: null,
      error_selector: null
    },
    extension: {
      slug: values.extension_slug.trim(),
      method: values.extension_method,
      response_mode: values.extension_response_mode,
      parameters: normalizeWorkflowExtensionParameters(
        values.extension_parameters
      )
    }
  };
}

export function createWorkflowApiMappingWithoutExtension(): ApplicationApiMapping {
  return {
    input: {
      query_target: 'node-workflow-start.query',
      model_target: null,
      inputs_target: 'node-workflow-start.inputs',
      history_target: null,
      attachments_target: null
    },
    output: {
      answer_selector: null,
      usage_selector: null,
      files_selector: null,
      error_selector: null
    }
  };
}

export function createWorkflowScheduleTriggerInput(
  values: Pick<
    WorkflowTriggerFormValues,
    | 'schedule_enabled'
    | 'schedule_cron'
    | 'schedule_timezone'
    | 'schedule_input_payload'
  >
): WorkflowScheduleTriggerInput {
  return {
    enabled: values.schedule_enabled,
    cron: values.schedule_cron.trim(),
    timezone: values.schedule_timezone.trim(),
    input_payload: parseWorkflowScheduleInputPayload(
      values.schedule_input_payload
    )
  };
}

export function createWorkflowTriggerValuesFromMapping(
  mapping: ApplicationApiMapping | null | undefined
): WorkflowTriggerFormValues {
  const extension = mapping?.extension;

  return {
    ...DEFAULT_WORKFLOW_TRIGGER_VALUES,
    trigger_type: extension ? 'extension' : 'schedule',
    extension_slug: extension?.slug ?? '',
    extension_method: extension?.method ?? 'POST',
    extension_response_mode: extension?.response_mode ?? 'sync',
    extension_parameters:
      extension?.parameters && extension.parameters.length > 0
        ? extension.parameters.map((parameter) => ({
            source: parameter.source,
            name: parameter.name,
            target: parameter.target
          }))
        : DEFAULT_WORKFLOW_TRIGGER_VALUES.extension_parameters.map(
            (parameter) => ({ ...parameter })
          )
  };
}

export function createWorkflowTriggerValuesFromContext({
  triggerType,
  mapping,
  schedule
}: {
  triggerType: WorkflowTriggerType;
  mapping: ApplicationApiMapping | null | undefined;
  schedule: WorkflowScheduleTrigger | null | undefined;
}): WorkflowTriggerFormValues {
  const extensionValues = createWorkflowTriggerValuesFromMapping(mapping);

  return {
    ...extensionValues,
    trigger_type: triggerType,
    schedule_enabled:
      schedule?.enabled ?? DEFAULT_WORKFLOW_TRIGGER_VALUES.schedule_enabled,
    schedule_cron:
      schedule?.cron ?? DEFAULT_WORKFLOW_TRIGGER_VALUES.schedule_cron,
    schedule_timezone:
      schedule?.timezone ?? DEFAULT_WORKFLOW_TRIGGER_VALUES.schedule_timezone,
    schedule_input_payload: JSON.stringify(
      schedule?.input_payload ??
        parseWorkflowScheduleInputPayload(
          DEFAULT_WORKFLOW_TRIGGER_VALUES.schedule_input_payload
        ),
      null,
      2
    )
  };
}

export function parseWorkflowScheduleInputPayload(value: string): unknown {
  return JSON.parse(value.trim() || '{}');
}

export function createWorkflowExtensionTargetOptions(
  document: FlowAuthoringDocument
): WorkflowExtensionTargetOption[] {
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  );

  if (!startNode) {
    return [];
  }

  return getStartInputFields(startNode).map((field) => {
    const value = `${startNode.id}.${field.key}`;

    return {
      value,
      label: `${field.label} · ${value}`
    };
  });
}

export function findInvalidWorkflowExtensionParameterTargets(
  mapping: ApplicationApiMapping | null | undefined,
  targetOptions: WorkflowExtensionTargetOption[]
) {
  const allowedTargets = new Set(targetOptions.map((option) => option.value));

  return (
    mapping?.extension?.parameters
      .map((parameter) => parameter.target.trim())
      .filter((target) => target.length > 0 && !allowedTargets.has(target)) ??
    []
  );
}

function normalizeWorkflowExtensionParameters(
  parameters: WorkflowExtensionParameterFormValue[] = []
) {
  return parameters
    .map((parameter) => ({
      source: parameter.source,
      name: parameter.name?.trim() ?? '',
      target: parameter.target?.trim() ?? ''
    }))
    .filter((parameter) => parameter.name !== '' || parameter.target !== '');
}
