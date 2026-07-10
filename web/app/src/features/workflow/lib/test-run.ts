import type {
  ConsoleWorkflowExtensionParameterMapping,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

type WorkflowInputValues = Record<string, unknown>;
type WorkflowExtensionInputs = Record<
  ConsoleWorkflowExtensionParameterMapping['source'],
  WorkflowInputValues
>;

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
  extensionParameters: ConsoleWorkflowExtensionParameterMapping[];
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

function workflowInputKeys(document: FlowAuthoringDocument) {
  const startNode = workflowStartNode(document);
  const rawFields = startNode.config.input_fields;

  if (!Array.isArray(rawFields)) {
    return new Set<string>();
  }

  return new Set(
    rawFields.flatMap((field) => {
      if (typeof field !== 'object' || field === null) {
        return [];
      }
      const key = (field as Record<string, unknown>).key;
      return typeof key === 'string' && key.length > 0 ? [key] : [];
    })
  );
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
  startNodeId: string,
  allowedKeys: Set<string>,
  parameters: ConsoleWorkflowExtensionParameterMapping[],
  inputs: WorkflowExtensionInputs
) {
  const values: WorkflowInputValues = {};

  for (const parameter of parameters) {
    const selector = parameter.target.split('.');
    if (
      selector.length !== 2 ||
      selector[0] !== startNodeId ||
      !allowedKeys.has(selector[1])
    ) {
      continue;
    }

    const sourceValues = inputs[parameter.source];
    if (Object.prototype.hasOwnProperty.call(sourceValues, parameter.name)) {
      values[selector[1]] = sourceValues[parameter.name];
    }
  }

  return values;
}

export function buildWorkflowTestRunInput(input: WorkflowTestRunInput) {
  const startNode = workflowStartNode(input.document);
  const allowedKeys = workflowInputKeys(input.document);
  let values: WorkflowInputValues;

  switch (input.triggerType) {
    case 'schedule':
      values = selectWorkflowInputs(asRecord(input.schedulePayload), allowedKeys);
      break;
    case 'extension':
      values = extensionWorkflowInputs(
        startNode.id,
        allowedKeys,
        input.extensionParameters,
        input.extensionInputs
      );
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
