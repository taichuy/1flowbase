import type { SchemaViewRendererProps } from '../../../shared/schema-ui/v1/registry/create-renderer-registry';
import { createWorkflowStartTriggerSummary } from '../lib/node-definitions';

export function WorkflowStartCardDescription({
  adapter
}: SchemaViewRendererProps) {
  return (
    <div className="agent-flow-node-card__description">
      {createWorkflowStartTriggerSummary(
        adapter.getDerived('workflowTriggerContext')
      )}
    </div>
  );
}
