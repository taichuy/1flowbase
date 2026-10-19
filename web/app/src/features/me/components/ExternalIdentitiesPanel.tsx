import { useMutation, useQuery } from '@tanstack/react-query';
import { Alert, Button, Space, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { fetchLoginEntries } from '../../auth/api/session';
import { authorizeMyExternalIdentity } from '../api/me';

export function ExternalIdentitiesPanel({
  csrfToken
}: {
  csrfToken: string | null;
}) {
  const { t } = useTranslation('me');
  const entries = useQuery({
    queryKey: ['me', 'external-login-entries'],
    queryFn: () => fetchLoginEntries(),
    select: (response) =>
      response.login_entries.filter(
        (entry) => entry.auth_type !== 'password-local'
      )
  });
  const authorize = useMutation({
    mutationFn: async (login_entry_id: string) => {
      if (!csrfToken)
        throw new Error(t('external_identities.session_required'));
      return authorizeMyExternalIdentity(login_entry_id, csrfToken);
    },
    onSuccess: ({ authorization_url }) =>
      window.location.assign(authorization_url)
  });
  if (!entries.data?.length) return null;
  const bound =
    new URLSearchParams(window.location.search).get('auth_bound') === 'true';
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Title level={3}>
        {t('external_identities.title')}
      </Typography.Title>
      <Typography.Paragraph type="secondary">
        {t('external_identities.description')}
      </Typography.Paragraph>
      {bound ? (
        <Alert showIcon type="success" title={t('external_identities.bound')} />
      ) : null}
      {authorize.isError ? (
        <Alert showIcon type="error" title={t('external_identities.failed')} />
      ) : null}
      {entries.data.map((entry) => (
        <Button
          key={entry.id}
          disabled={!csrfToken}
          loading={authorize.isPending && authorize.variables === entry.id}
          onClick={() => authorize.mutate(entry.id)}
        >
          {t('external_identities.bind', { provider: entry.title })}
        </Button>
      ))}
    </Space>
  );
}
