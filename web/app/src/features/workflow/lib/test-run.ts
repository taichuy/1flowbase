import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { getStartInputFields } from '../../flow-editor/authoring/start-input-fields';

export function buildWorkflowTestRunInput({
  document,
  inputs
}: {
  document: FlowAuthoringDocument;
  inputs: Record<string, unknown>;
}) {
  const startNode = document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  );

  if (!startNode) {
    throw new Error('Workflow document requires a workflow_start node');
  }

  const values = Object.fromEntries(
    getStartInputFields(startNode).flatMap((field) => {
      const value = inputs[field.key] ?? field.defaultValue;
      return value === undefined ? [] : [[field.key, value]];
    })
  );

  return {
    input_payload: {
      [startNode.id]: values
    }
  };
}
