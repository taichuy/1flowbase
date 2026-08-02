import { useQuery } from '@tanstack/react-query';
import { Result } from 'antd';
import { useMemo } from 'react';

import {
  ApiClientError,
  type ConsoleWorkflowTriggerType
} from '@1flowbase/api-client';

import '../register';
import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import {
  applicationEnvironmentVariablesQueryKey,
  fetchApplicationEnvironmentVariables
} from '../../applications/api/applications';
import {
  applicationApiMappingQueryKey,
  fetchApplicationApiMapping,
  fetchWorkflowScheduleTrigger,
  workflowScheduleTriggerQueryKey
} from '../../applications/api/public-api';
import {
  applicationNodeCatalogQueryKey,
  fetchApplicationNodeCatalog
} from '../../agent-flow/api/node-contributions';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../../agent-flow/api/orchestration';
import { WorkflowEditorAssembly } from '../components/WorkflowEditorAssembly';
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
  const orchestrationQuery = useQuery({
    queryKey: orchestrationQueryKey(applicationId),
    queryFn: () => fetchOrchestrationState(applicationId)
  });
  const nodeCatalogQuery = useQuery({
    queryKey: applicationNodeCatalogQueryKey(applicationId),
    queryFn: () => fetchApplicationNodeCatalog(applicationId)
  });
  const environmentVariablesQuery = useQuery({
    queryKey: applicationEnvironmentVariablesQueryKey(applicationId),
    queryFn: () => fetchApplicationEnvironmentVariables(applicationId)
  });
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
      schedule: scheduleQuery.data ?? null,
      workflowStartFieldContract: nodeCatalogQuery.data?.nodes.find(
        (node) => node.node_type === 'workflow_start'
      )?.field_contract
    }),
    [
      applicationId,
      mappingQuery.data,
      nodeCatalogQuery.data?.nodes,
      scheduleQuery.data,
      workflowTriggerType
    ]
  );

  if (
    orchestrationQuery.isPending ||
    nodeCatalogQuery.isPending ||
    environmentVariablesQuery.isPending
  ) {
    return <LoadingState compact />;
  }

  if (
    orchestrationQuery.isError ||
    nodeCatalogQuery.isError ||
    environmentVariablesQuery.isError
  ) {
    const error = orchestrationQuery.isError
      ? orchestrationQuery.error
      : nodeCatalogQuery.isError
        ? nodeCatalogQuery.error
        : environmentVariablesQuery.error;

    if (error instanceof ApiClientError && error.status === 403) {
      return <PermissionDeniedState />;
    }

    if (error instanceof ApiClientError && error.status === 404) {
      return (
        <Result
          status="404"
          title={i18nText('agentFlow', 'auto.orchestration_not_found')}
        />
      );
    }

    return (
      <Result
        status="error"
        title={i18nText('agentFlow', 'auto.orchestration_load_failed')}
      />
    );
  }

  return (
    <WorkflowEditorAssembly
      applicationId={applicationId}
      applicationName={applicationName}
      workflowTriggerContext={workflowTriggerContext}
      initialState={orchestrationQuery.data}
      initialEnvironmentVariables={environmentVariablesQuery.data}
      nodeCatalog={nodeCatalogQuery.data}
    />
  );
}
