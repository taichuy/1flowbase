import type { ConsoleWorkflowTriggerType } from '@1flowbase/api-client';

import type {
  ApplicationApiMapping,
  WorkflowScheduleTrigger
} from '../../applications/api/public-api';

export interface WorkflowTriggerContext {
  applicationId: string;
  triggerType: ConsoleWorkflowTriggerType | null;
  mapping: ApplicationApiMapping | null | undefined;
  schedule: WorkflowScheduleTrigger | null | undefined;
}
