import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Space, Switch, Tag, Typography } from 'antd';

import type { ApplicationDetail } from '../../api/applications';

import type { ApplicationApiPublication } from '../../api/public-api';

export function ApplicationApiStatusBar({
  publication,
  apiStatus,
  loading,
  onTogglePublished,
  onToggleApiEnabled,
  apiToggleDisabled,
  publishToggleDisabled,
  toolbar,
  children
}: {
  publication: ApplicationApiPublication | null;
  apiStatus?: ApplicationDetail['sections']['api'];
  loading?: boolean;
  onTogglePublished?: (published: boolean) => void;
  onToggleApiEnabled?: (api_enabled: boolean) => void;
  apiToggleDisabled?: boolean;
  publishToggleDisabled?: boolean;
  toolbar?: ReactNode;
  children?: ReactNode;
}) {
  const { t } = useTranslation('applications');
  const published = Boolean(publication);

  return (
    <section
      aria-label={t('auto.public_api_status')}
      className="application-api-status"
    >
      <div className="application-api-status__header">
        <Space className="application-api-status__summary" align="center" wrap>
          <Typography.Text strong>
            {apiStatus ? t('agent_logs.reception') : t('auto.public_api')}
          </Typography.Text>
          {apiStatus ? (
            <>
              <Switch
                aria-label={t('agent_logs.reception')}
                checked={apiStatus.api_capability_status === 'enabled'}
                disabled={apiToggleDisabled}
                loading={loading}
                onChange={onToggleApiEnabled}
              />
              <Tag>
                {apiStatus.api_capability_status === 'enabled'
                  ? t('agent_logs.reception_enabled')
                  : t('agent_logs.reception_disabled')}
              </Tag>
              <Typography.Text type="secondary">
                {t('agent_logs.reception_help')}
              </Typography.Text>
            </>
          ) : (
            <Switch
              disabled={publishToggleDisabled}
              checked={published}
              loading={loading}
              checkedChildren={t('auto.publication_published')}
              unCheckedChildren={t('auto.publication_draft')}
              onChange={onTogglePublished}
            />
          )}
          {publication ? (
            <Typography.Text type="secondary">
              active publication v{publication.version_sequence}
            </Typography.Text>
          ) : null}
        </Space>
        <div className="application-api-status__docs-toolbar">{toolbar}</div>
        <div className="application-api-status__actions">{children}</div>
      </div>
    </section>
  );
}
