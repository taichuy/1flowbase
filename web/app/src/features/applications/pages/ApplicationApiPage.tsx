import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App, Result } from 'antd';
import { useTranslation } from 'react-i18next';

import { i18nText } from '../../../shared/i18n/text';
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

export function ApplicationApiPage({
  application
}: {
  application: ApplicationDetail;
}) {
  const { t } = useTranslation('applications');
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
    return <Result status="info" title={t('auto.loading_public_api_status')} />;
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
