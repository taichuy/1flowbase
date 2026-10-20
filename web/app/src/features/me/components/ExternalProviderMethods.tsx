import {
  Avatar,
  Button,
  Descriptions,
  Divider,
  Popconfirm,
  Space,
  Typography,
  Alert
} from 'antd';
import { useTranslation } from 'react-i18next';
import type { ConsoleAuthenticationProvider } from '../api/me';

export function ExternalProviderMethods({
  provider,
  disabled,
  authorizingEntryId,
  unlinkingIdentityId,
  onAuthorize,
  onUnlink
}: {
  provider: ConsoleAuthenticationProvider;
  disabled: boolean;
  authorizingEntryId: string | undefined;
  unlinkingIdentityId: string | undefined;
  onAuthorize: (login_entry_id: string) => void;
  onUnlink: (identity_id: string) => void;
}) {
  const { t } = useTranslation('me');
  const blockedReason = (reason: string | null) => {
    if (reason === 'last_usable_authentication_method')
      return t('authentication.last_login_method');
    if (reason === 'provider_unavailable')
      return t('authentication.provider_unavailable');
    return t('authentication.unlink_unavailable');
  };
  const authorizeButtons = (restore: boolean) =>
    provider.login_entries.map((entry) => (
      <Button
        key={entry.id}
        disabled={disabled || !provider.provider_available}
        loading={authorizingEntryId === entry.id}
        onClick={() => onAuthorize(entry.id)}
      >
        {restore
          ? t('authentication.restore', { entry: entry.title })
          : t('authentication.bind', { entry: entry.title })}
      </Button>
    ));

  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Paragraph>
        {t('authentication.binding_description')}
      </Typography.Paragraph>
      {!provider.provider_available ? (
        <Alert
          type="warning"
          showIcon
          title={t('authentication.provider_unavailable')}
        />
      ) : null}
      {provider.bindings.length === 0 ? (
        <>
          <Typography.Text>{t('authentication.not_bound')}</Typography.Text>
          <Space wrap>{authorizeButtons(false)}</Space>
        </>
      ) : null}
      {provider.bindings.map((binding, index) => (
        <div key={binding.identity_id}>
          {index > 0 ? <Divider /> : null}
          <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
            <Space>
              <Avatar src={binding.avatar_url} size={48} />
              <Typography.Text strong>
                {binding.nickname ?? binding.account ?? binding.subject_value}
              </Typography.Text>
            </Space>
            <Descriptions
              column={1}
              size="small"
              items={[
                {
                  key: 'state',
                  label: t('authentication.state'),
                  children:
                    binding.state === 'bound'
                      ? t('authentication.bound')
                      : t('authentication.unlinked')
                },
                {
                  key: 'account',
                  label: t('profile.fields.account'),
                  children: binding.account ?? '—'
                },
                {
                  key: 'nickname',
                  label: t('profile.fields.nickname'),
                  children: binding.nickname ?? '—'
                },
                {
                  key: 'profile_url',
                  label: t('authentication.profile_url'),
                  children: binding.profile_url ? (
                    <Typography.Link
                      href={binding.profile_url}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {binding.profile_url}
                    </Typography.Link>
                  ) : (
                    '—'
                  )
                },
                {
                  key: 'bound_at',
                  label: t('authentication.bound_at'),
                  children: binding.bound_at
                },
                ...(binding.unlinked_at
                  ? [
                      {
                        key: 'unlinked_at',
                        label: t('authentication.unlinked_at'),
                        children: binding.unlinked_at
                      }
                    ]
                  : [])
              ]}
            />
            {binding.state === 'bound' ? (
              <>
                {!binding.can_unlink ? (
                  <Typography.Paragraph type="secondary">
                    {blockedReason(binding.unlink_blocked_reason)}
                  </Typography.Paragraph>
                ) : null}
                <Popconfirm
                  title={t('authentication.unlink_confirm')}
                  description={t('authentication.unlink_description')}
                  okText={t('authentication.unlink')}
                  cancelText={t('profile.actions.cancel')}
                  disabled={disabled || !binding.can_unlink}
                  onConfirm={() => onUnlink(binding.identity_id)}
                >
                  <Button
                    danger
                    disabled={disabled || !binding.can_unlink}
                    loading={unlinkingIdentityId === binding.identity_id}
                  >
                    {t('authentication.unlink')}
                  </Button>
                </Popconfirm>
              </>
            ) : (
              <>
                <Typography.Paragraph type="secondary">
                  {t('authentication.restore_description')}
                </Typography.Paragraph>
                <Space wrap>{authorizeButtons(true)}</Space>
              </>
            )}
          </Space>
        </div>
      ))}
    </Space>
  );
}
