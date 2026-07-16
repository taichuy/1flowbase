import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Space, Switch, Typography } from 'antd';

import type { ApplicationApiPublication } from '../../api/public-api';

export function ApplicationApiStatusBar({
  publication,
  loading,
  onTogglePublished,
  toolbar,
  children
}: {
  publication: ApplicationApiPublication | null;
  loading?: boolean;
  onTogglePublished?: (published: boolean) => void;
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
          <Typography.Text strong>{t('auto.public_api')}</Typography.Text>
          <Switch
            checked={published}
            loading={loading}
            checkedChildren={t('auto.publication_published')}
            unCheckedChildren={t('auto.publication_draft')}
            onChange={onTogglePublished}
          />
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
