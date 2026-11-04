import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App } from 'antd';

import { i18nText } from '../../../shared/i18n/text';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { useConsoleOperationAccess } from '../../../state/console-operation-access';
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
  unpublishApplicationApiVersion,
  updateApplicationApiStatus
} from '../api/public-api';
import { ApplicationApiDocsPanel } from '../components/api/ApplicationApiDocsPanel';
import { ApplicationApiKeysPanel } from '../components/api/ApplicationApiKeysPanel';
import { ApplicationApiStatusBar } from '../components/api/ApplicationApiStatusBar';
import './application-api-page.css';

export function ApplicationApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { modal, message } = App.useApp();
  const can = useConsoleOperationAccess();
  const canSetApiStatus = can('applications.api.set_enabled');
  const isLogCollection = application.application_type === 'agent_logs';
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const queryClient = useQueryClient();
  const docsToolbarId = `application-api-docs-toolbar-${application.id}`;
  const publicationQuery = useQuery({
    queryKey: applicationApiPublicationQueryKey(application.id),
    queryFn: () => fetchApplicationApiPublication(application.id),
    retry: false,
    enabled: !isLogCollection
  });
  const mappingQuery = useQuery({
    queryKey: applicationApiMappingQueryKey(application.id),
    queryFn: () => fetchApplicationApiMapping(application.id),
    enabled: !isLogCollection
  });
  const publication = isLogCollection ? null : (publicationQuery.data ?? null);
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
      if (!can('publish_application_api')) throw new Error('permission denied');
      const mapping =
        mappingQuery.data ?? (await fetchApplicationApiMapping(application.id));
      return publishApplicationApiVersion(application.id, mapping, csrfToken);
    },
    onSuccess: invalidatePublication
  });
  const revertToDraftMutation = useMutation({
    mutationFn: () => {
      if (!can('unpublish_application_api'))
        throw new Error('permission denied');
      return unpublishApplicationApiVersion(application.id, csrfToken);
    },
    onSuccess: invalidatePublication
  });

  const receptionMutation = useMutation({
    mutationFn: (api_enabled: boolean) => {
      if (!canSetApiStatus) throw new Error('permission denied');
      return updateApplicationApiStatus(application.id, api_enabled, csrfToken);
    },
    onSuccess: () => {
      invalidatePublication();
      void queryClient.invalidateQueries({
        queryKey: ['settings', 'applications']
      });
    },
    onError: () =>
      message.error(
        i18nText('applications', 'agent_logs.reception_update_failed')
      )
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

  if (!isLogCollection && !publication && publicationQuery.isLoading) {
    return <LoadingState compact />;
  }

  return (
    <div className="application-api-page">
      <ApplicationApiStatusBar
        publication={publication}
        apiStatus={isLogCollection ? application.sections.api : undefined}
        loading={
          publishMutation.isPending ||
          revertToDraftMutation.isPending ||
          receptionMutation.isPending
        }
        apiToggleDisabled={!canSetApiStatus}
        publishToggleDisabled={
          !can(
            publication
              ? 'unpublish_application_api'
              : 'publish_application_api'
          )
        }
        onToggleApiEnabled={(api_enabled) =>
          receptionMutation.mutate(api_enabled)
        }
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
