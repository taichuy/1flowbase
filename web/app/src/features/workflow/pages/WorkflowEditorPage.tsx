import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import type { ConsoleWorkflowTriggerType } from '@1flowbase/api-client';

import '../register';
import { AgentFlowEditorPage } from '../../agent-flow/pages/AgentFlowEditorPage';
import {
  applicationApiMappingQueryKey,
  fetchApplicationApiMapping,
  fetchWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../../applications/api/public-api';
import { WORKFLOW_EDITOR_CAPABILITIES } from '../lib/editor-capabilities';
import { buildWorkflowNodePickerOptions } from '../lib/picker-options';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

export function WorkflowEditorPage({
  applicationId,
  applicationName,
  workflowTriggerType
}: {
  applicationId: string;
  applicationName: string;
  workflowTriggerType: ConsoleWorkflowTriggerType | null;
}) {
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(applicationId),
    queryFn: () => fetchApplicationApiMapping(applicationId)
  });
  const scheduleQuery = useQuery({
    queryKey: workflowScheduleTriggerQueryKey(applicationId),
    queryFn: () => fetchWorkflowScheduleTrigger(applicationId),
    retry: false
  });
  const workflowTriggerContext = useMemo<WorkflowTriggerContext>(
    () => ({
      applicationId,
      triggerType: workflowTriggerType,
      mapping: mappingQuery.data,
      schedule: scheduleQuery.data ?? null
    }),
    [applicationId, mappingQuery.data, scheduleQuery.data, workflowTriggerType]
  );

  return (
    <AgentFlowEditorPage
      applicationId={applicationId}
      applicationName={applicationName}
      workflowTriggerContext={workflowTriggerContext}
      capabilities={WORKFLOW_EDITOR_CAPABILITIES}
      nodePickerOptionsBuilder={buildWorkflowNodePickerOptions}
    />
  );
}
