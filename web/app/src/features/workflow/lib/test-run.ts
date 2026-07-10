import type { ConsoleWorkflowTriggerType } from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

type WorkflowInputValues = Record<string, unknown>;
type WorkflowInputSource = 'path' | 'query' | 'body' | 'form';
type WorkflowExtensionInputs = Record<WorkflowInputSource, WorkflowInputValues>;

interface WorkflowHttpInputField {
  key: string;
  source?: WorkflowInputSource;
  required: boolean;
  defaultValue?: unknown;
}

interface WorkflowTestRunBaseInput {
  document: FlowAuthoringDocument;
  triggerType: ConsoleWorkflowTriggerType;
}

interface ScheduleWorkflowTestRunInput extends WorkflowTestRunBaseInput {
  triggerType: 'schedule';
  schedulePayload: unknown;
}

interface ExtensionWorkflowTestRunInput extends WorkflowTestRunBaseInput {
  triggerType: 'extension';
  extensionInputs: WorkflowExtensionInputs;
}

export type WorkflowTestRunInput =
  | ScheduleWorkflowTestRunInput
  | ExtensionWorkflowTestRunInput;

function asRecord(value: unknown): WorkflowInputValues {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as WorkflowInputValues)
    : {};
}

function workflowStartNode(document: FlowAuthoringDocument) {
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  );

  if (!startNode) {
    throw new Error('Workflow document requires a workflow_start node');
  }

  return startNode;
}

export function listWorkflowHttpInputFields(document: FlowAuthoringDocument) {
  const startNode = workflowStartNode(document);
  const rawFields = startNode.config.input_fields;

  if (!Array.isArray(rawFields)) {
    return [];
  }

  return rawFields.flatMap((field): WorkflowHttpInputField[] => {
    if (typeof field !== 'object' || field === null) {
      return [];
    }

    const inputField = field as Record<string, unknown>;
    const key = inputField.key;
    const source = inputField.source;
    if (typeof key !== 'string' || key.length === 0) {
      return [];
    }

    const normalizedSource =
      source === 'path' ||
      source === 'query' ||
      source === 'body' ||
      source === 'form'
        ? source
        : undefined;

    return [
      {
        key,
        ...(normalizedSource ? { source: normalizedSource } : {}),
        required: inputField.required === true,
        ...(Object.prototype.hasOwnProperty.call(inputField, 'defaultValue')
          ? { defaultValue: inputField.defaultValue }
          : {})
      }
    ];
  });
}

function selectWorkflowInputs(
  values: WorkflowInputValues,
  allowedKeys: Set<string>
) {
  return Object.fromEntries(
    Object.entries(values).filter(([key]) => allowedKeys.has(key))
  );
}

function extensionWorkflowInputs(
  fields: WorkflowHttpInputField[],
  inputs: WorkflowExtensionInputs
) {
  const values: WorkflowInputValues = {};

  for (const field of fields) {
    if (!field.source) {
      continue;
    }
    const sourceValues = inputs[field.source];
    if (Object.prototype.hasOwnProperty.call(sourceValues, field.key)) {
      values[field.key] = sourceValues[field.key];
    } else if (Object.prototype.hasOwnProperty.call(field, 'defaultValue')) {
      values[field.key] = field.defaultValue;
    } else if (field.required) {
      throw new Error(`Missing required workflow input: ${field.key}`);
    }
  }

  return values;
}

export function buildWorkflowTestRunInput(input: WorkflowTestRunInput) {
  const startNode = workflowStartNode(input.document);
  const inputFields = listWorkflowHttpInputFields(input.document);
  const allowedKeys = new Set(inputFields.map((field) => field.key));
  let values: WorkflowInputValues;

  switch (input.triggerType) {
    case 'schedule':
      values = selectWorkflowInputs(asRecord(input.schedulePayload), allowedKeys);
      break;
    case 'extension':
      values = extensionWorkflowInputs(inputFields, input.extensionInputs);
      break;
  }

  return {
    input_payload: {
      [startNode.id]: values
    }
  };
}

export function readWorkflowResult(detail: {
  flow_run: { output_payload: Record<string, unknown> };
}) {
  return detail.flow_run.output_payload;
}
