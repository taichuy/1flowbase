import { useQuery } from '@tanstack/react-query';
import { Result } from 'antd';
import type { ReactNode } from 'react';

import { ApiClientError } from '@1flowbase/api-client';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
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
import { AgentFlowEditorAssembly } from '../components/editor/AgentFlowEditorAssembly';
import { i18nText } from '../../../shared/i18n/text';

export function AgentFlowEditorPage({
  applicationId,
  applicationName,
  topSlot
}: {
  applicationId: string;
  applicationName: string;
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
    return <LoadingState compact />;
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
    <AgentFlowEditorAssembly
      applicationId={applicationId}
      applicationName={applicationName}
      initialState={state}
      initialEnvironmentVariables={environmentVariablesQuery.data}
      nodeContributions={nodeContributions}
      topSlot={topSlot}
    />
  );
}
