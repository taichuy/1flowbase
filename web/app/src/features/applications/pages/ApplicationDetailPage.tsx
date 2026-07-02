import { useQuery } from '@tanstack/react-query';
import { Navigate } from '@tanstack/react-router';
import { Result } from 'antd';
import { Suspense, lazy, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ApiClientError } from '@1flowbase/api-client';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import { SectionPageLayout } from '../../../shared/ui/section-page-layout/SectionPageLayout';
import {
  applicationDetailQueryKey,
  fetchApplicationDetail
} from '../api/applications';
import { ApplicationSectionState } from '../components/ApplicationSectionState';
import {
  getApplicationSections,
  type ApplicationSectionKey
} from '../lib/application-sections';
import './workflow-application-page.css';

const AgentFlowEditorPage = lazy(() =>
  import('../../agent-flow/pages/AgentFlowEditorPage').then((module) => ({
    default: module.AgentFlowEditorPage
  }))
);
const ApplicationLogsPage = lazy(() =>
  import('./ApplicationLogsPage').then((module) => ({
    default: module.ApplicationLogsPage
  }))
);
const ApplicationApiPage = lazy(() =>
  import('./ApplicationApiPage').then((module) => ({
    default: module.ApplicationApiPage
  }))
);
const ApplicationMonitoringPage = lazy(() =>
  import('./ApplicationMonitoringPage').then((module) => ({
    default: module.ApplicationMonitoringPage
  }))
);

function ApplicationSectionFallback() {
  return <LoadingState compact />;
}

function ApplicationSectionBoundary({ children }: { children: ReactNode }) {
  return (
    <Suspense fallback={<ApplicationSectionFallback />}>{children}</Suspense>
  );
}

export function ApplicationDetailPage({
  applicationId,
  requestedSectionKey
}: {
  applicationId: string;
  requestedSectionKey: ApplicationSectionKey;
}) {
  const { t } = useTranslation('applications');
  const detailQuery = useQuery({
    queryKey: applicationDetailQueryKey(applicationId),
    queryFn: () => fetchApplicationDetail(applicationId)
  });

  if (detailQuery.isPending) {
    return <LoadingState />;
  }

  if (detailQuery.isError) {
    const error = detailQuery.error;

    if (error instanceof ApiClientError && error.status === 403) {
      return <PermissionDeniedState />;
    }

    if (error instanceof ApiClientError && error.status === 404) {
      return <Result status="404" title={t('auto.application_not_found')} />;
    }

    return <Result status="error" title={t('auto.application_load_failed')} />;
  }

  const application = detailQuery.data;
  const isWorkflow = application.application_type === 'workflow';

  if (isWorkflow && requestedSectionKey === 'api') {
    return (
      <Navigate
        to="/applications/$applicationId/orchestration"
        params={{ applicationId }}
        replace
      />
    );
  }

  const content =
    requestedSectionKey === 'orchestration' ? (
      isWorkflow ? (
        <div className="workflow-orchestration-page">
          <ApplicationSectionBoundary>
            <AgentFlowEditorPage
              applicationId={applicationId}
              applicationName={application.name}
              applicationType={application.application_type}
              workflowTriggerType={application.workflow_trigger_type}
            />
          </ApplicationSectionBoundary>
        </div>
      ) : (
        <ApplicationSectionBoundary>
          <AgentFlowEditorPage
            applicationId={applicationId}
            applicationName={application.name}
            applicationType={application.application_type}
          />
        </ApplicationSectionBoundary>
      )
    ) : requestedSectionKey === 'logs' ? (
      <ApplicationSectionBoundary>
        <ApplicationLogsPage applicationId={applicationId} />
      </ApplicationSectionBoundary>
    ) : requestedSectionKey === 'api' ? (
      <ApplicationSectionBoundary>
        <ApplicationApiPage application={application} />
      </ApplicationSectionBoundary>
    ) : requestedSectionKey === 'monitoring' ? (
      <ApplicationSectionBoundary>
        <ApplicationMonitoringPage applicationId={applicationId} />
      </ApplicationSectionBoundary>
    ) : (
      <ApplicationSectionState
        application={application}
        sectionKey={requestedSectionKey}
      />
    );

  return (
    <SectionPageLayout
      pageTitle={application.name}
      navItems={getApplicationSections(
        applicationId,
        t,
        application.application_type
      )}
      activeKey={requestedSectionKey}
      contentWidth={requestedSectionKey === 'orchestration' ? 'full' : 'wide'}
      heightMode={
        requestedSectionKey === 'logs' || requestedSectionKey === 'api'
          ? 'viewport'
          : 'natural'
      }
    >
      {content}
    </SectionPageLayout>
  );
}
