import type {
  ConsoleApplicationNodeFieldContract,
  ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';

import type {
  ApplicationApiMapping,
  WorkflowScheduleTrigger
} from '../../applications/api/public-api';

export interface WorkflowTriggerContext {
  applicationId: string;
  triggerType: ConsoleWorkflowTriggerType | null;
  mapping: ApplicationApiMapping | null | undefined;
  schedule: WorkflowScheduleTrigger | null | undefined;
  workflowStartFieldContract: ConsoleApplicationNodeFieldContract | undefined;
}

export function asWorkflowTriggerContext(
  value: unknown
): WorkflowTriggerContext | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  return value as WorkflowTriggerContext;
}
