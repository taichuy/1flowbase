import { useQuery } from '@tanstack/react-query';
import { Result } from 'antd';
import type { ReactNode } from 'react';

import {
  ApiClientError,
  type ConsoleApplicationType
} from '@1flowbase/api-client';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import {
  applicationEnvironmentVariablesQueryKey,
  fetchApplicationEnvironmentVariables
} from '../../applications/api/applications';
import {
  fetchNodeContributions,
  nodeContributionsQueryKey
} from '../api/node-contributions';
import {
  fetchOrchestrationState,
  orchestrationQueryKey
} from '../api/orchestration';
import { AgentFlowEditorShell } from '../components/editor/AgentFlowEditorShell';
import { i18nText } from '../../../shared/i18n/text';

export function AgentFlowEditorPage({
  applicationId,
  applicationName,
  applicationType = 'agent_flow',
  topSlot
}: {
  applicationId: string;
  applicationName: string;
  applicationType?: ConsoleApplicationType;
  topSlot?: ReactNode;
}) {
  const orchestrationQuery = useQuery({
    queryKey: orchestrationQueryKey(applicationId),
    queryFn: () => fetchOrchestrationState(applicationId)
  });
  const nodeContributionsQuery = useQuery({
    queryKey: nodeContributionsQueryKey(applicationId),
    queryFn: () => fetchNodeContributions(applicationId)
  });
  const environmentVariablesQuery = useQuery({
    queryKey: applicationEnvironmentVariablesQueryKey(applicationId),
    queryFn: () => fetchApplicationEnvironmentVariables(applicationId)
  });

  if (
    orchestrationQuery.isPending ||
    nodeContributionsQuery.isPending ||
    environmentVariablesQuery.isPending
  ) {
    return (
      <Result
        status="info"
        title={i18nText('agentFlow', 'auto.orchestration_loading')}
      />
    );
  }

  if (
    orchestrationQuery.isError ||
    nodeContributionsQuery.isError ||
    environmentVariablesQuery.isError
  ) {
    const error = orchestrationQuery.isError
      ? orchestrationQuery.error
      : nodeContributionsQuery.isError
        ? nodeContributionsQuery.error
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

  const state = orchestrationQuery.data;
  const nodeContributions = nodeContributionsQuery.data;

  return (
    <AgentFlowEditorShell
      applicationId={applicationId}
      applicationName={applicationName}
      applicationType={applicationType}
      initialState={state}
      initialEnvironmentVariables={environmentVariablesQuery.data}
      nodeContributions={nodeContributions}
      topSlot={topSlot}
    />
  );
}
