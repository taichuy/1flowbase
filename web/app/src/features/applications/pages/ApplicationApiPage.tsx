import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { getApplicationsApiBaseUrl } from '../api/applications';

import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../../../state/auth-store';
import {
  applicationDetailQueryKey,
  type ApplicationDetail
} from '../api/applications';
import {
  applicationApiMappingQueryKey,
  applicationApiPublicationQueryKey,
  fetchApplicationApiMapping,
  fetchApplicationApiPublication,
  publishApplicationApiVersion,
  unpublishApplicationApiVersion
} from '../api/public-api';
import { ApplicationApiDocsPanel } from '../components/api/ApplicationApiDocsPanel';
import { ApplicationApiKeysPanel } from '../components/api/ApplicationApiKeysPanel';
import { ApplicationApiStatusBar } from '../components/api/ApplicationApiStatusBar';
import './application-api-page.css';

function PublishedApplicationApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { modal } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const docsToolbarId = `application-api-docs-toolbar-${application.id}`;
  const publicationQuery = useQuery({
    queryKey: applicationApiPublicationQueryKey(application.id),
    queryFn: () => fetchApplicationApiPublication(application.id),
    retry: false
  });
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(application.id),
    queryFn: () => fetchApplicationApiMapping(application.id)
  });
  const publication = publicationQuery.data ?? null;
  const invalidatePublication = () => {
    void queryClient.invalidateQueries({
      queryKey: applicationApiPublicationQueryKey(application.id)
    });
    void queryClient.invalidateQueries({
      queryKey: applicationDetailQueryKey(application.id)
    });
  };
  const publishMutation = useMutation({
    mutationFn: async () => {
      const mapping =
        mappingQuery.data ?? (await fetchApplicationApiMapping(application.id));
      return publishApplicationApiVersion(application.id, mapping, csrfToken);
    },
    onSuccess: invalidatePublication
  });
  const revertToDraftMutation = useMutation({
    mutationFn: () => unpublishApplicationApiVersion(application.id, csrfToken),
    onSuccess: invalidatePublication
  });

  const confirmRevertToDraft = () => {
    modal.confirm({
      title: i18nText('applications', 'auto.revert_to_draft'),
      content: i18nText('applications', 'auto.revert_to_draft_confirm_content'),
      okText: i18nText('applications', 'auto.revert_to_draft'),
      cancelText: i18nText('applications', 'auto.cancel'),
      onOk: () => revertToDraftMutation.mutateAsync()
    });
  };

  if (!publication && publicationQuery.isLoading) {
    return <LoadingState compact />;
  }

  return (
    <div className="application-api-page">
      <ApplicationApiStatusBar
        publication={publication}
        loading={publishMutation.isPending || revertToDraftMutation.isPending}
        onTogglePublished={(published) => {
          if (published) {
            publishMutation.mutate();
          } else {
            confirmRevertToDraft();
          }
        }}
        toolbar={
          <div
            id={docsToolbarId}
            className="application-api-status__docs-toolbar-target"
          />
        }
      >
        <ApplicationApiKeysPanel
          applicationId={application.id}
          csrfToken={csrfToken}
          onCreatedToken={() => undefined}
          variant="embedded"
        />
      </ApplicationApiStatusBar>
      <ApplicationApiDocsPanel
        applicationId={application.id}
        toolbarPortalId={docsToolbarId}
      />
    </div>
  );
}

export function ApplicationApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { t } = useTranslation('applications');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  if (application.application_type !== 'agent_logs') {
    return <PublishedApplicationApiPage application={application} />;
  }
  const endpoint = `${getApplicationsApiBaseUrl()}/api/logs/v1/events`;
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Title level={4}>
        {t('agent_logs.ingest_api')}
      </Typography.Title>
      <Typography.Paragraph>
        {t('agent_logs.api_description')}
      </Typography.Paragraph>
      <Typography.Text code copyable>{`POST ${endpoint}`}</Typography.Text>
      <Typography.Paragraph>{t('agent_logs.api_auth')}</Typography.Paragraph>
      <ApplicationApiKeysPanel
        applicationId={application.id}
        csrfToken={csrfToken}
        onCreatedToken={() => undefined}
      />
      <Typography.Title level={5}>
        {t('agent_logs.request_body')}
      </Typography.Title>
      <Typography.Paragraph>
        {t('agent_logs.envelope_description')}
      </Typography.Paragraph>
      <pre>
        {JSON.stringify(
          {
            schema_version: '1flowbase.agent-logs/v1',
            source_id: 'collector-installation',
            source_client: 'codex',
            events: [
              {
                event_id: 'event-1',
                source_session_id: 'session-1',
                source_task_id: 'turn-1',
                parent_source_task_id: null,
                sequence: 1,
                occurred_at: '2026-10-07T08:00:00Z',
                kind: 'user',
                content: 'Hello',
                phase: null,
                name: null,
                call_id: null,
                model_id: null,
                provider_code: null,
                usage: null,
                inherited: false,
                raw: {}
              }
            ]
          },
          null,
          2
        )}
      </pre>
      <Typography.Paragraph>
        {t('agent_logs.receipt_description')}
      </Typography.Paragraph>
    </Space>
  );
}
