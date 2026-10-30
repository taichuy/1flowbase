import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import { listVisibleSelectorOptions } from '../../agent-flow/lib/selector-options';

/** Both editors use the same lexical scopes; workflow-specific globals are selected by document type. */
export function listWorkflowVariableOptions(
  document: FlowAuthoringDocument,
  nodeId: string,
  workflowTriggerContext: unknown,
  fieldKey = ''
) {
  return listVisibleSelectorOptions(
    document,
    nodeId,
    [],
    workflowTriggerContext,
    fieldKey
  );
}
