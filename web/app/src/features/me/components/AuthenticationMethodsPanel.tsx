import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Space, Tabs, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import {
  authenticationMethodsQueryKey,
  authorizeMyExternalIdentity,
  authorizeMyPasswordSetup,
  fetchMyAuthenticationMethods,
  setMyFirstPassword,
  unlinkMyExternalIdentity,
  type ChangeMyPasswordInput
} from '../api/me';
import { ExternalProviderMethods } from './ExternalProviderMethods';
import { PasswordAuthenticationMethod } from './PasswordAuthenticationMethod';

export function AuthenticationMethodsPanel({
  csrfToken,
  changingPassword,
  changePasswordError,
  onChangePassword,
  onSessionEnded,
  onSwitchAccount
}: {
  csrfToken: string | null;
  changingPassword: boolean;
  changePasswordError: string | null;
  onChangePassword: (input: ChangeMyPasswordInput) => Promise<void>;
  onSessionEnded: () => Promise<void>;
  onSwitchAccount: () => Promise<void>;
}) {
  const { t } = useTranslation('me');
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: authenticationMethodsQueryKey,
    queryFn: fetchMyAuthenticationMethods
  });
  const [cancelledError, setCancelledError] = useState(false);
  const params = new URLSearchParams(window.location.search);
  const callbackError = cancelledError ? null : params.get('auth_error');
  const csrf = () => {
    if (!csrfToken) throw new Error('session required');
    return csrfToken;
  };
  const authorize = useMutation({
    mutationFn: (login_entry_id: string) =>
      authorizeMyExternalIdentity(login_entry_id, csrf()),
    onSuccess: ({ authorization_url }) =>
      window.location.assign(authorization_url)
  });
  const unlink = useMutation({
    mutationFn: (identity_id: string) =>
      unlinkMyExternalIdentity(identity_id, csrf()),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: authenticationMethodsQueryKey
      }),
    onError: () => {
      void queryClient.invalidateQueries({
        queryKey: authenticationMethodsQueryKey
      });
    }
  });
  const verify = useMutation({
    mutationFn: (login_entry_id: string) =>
      authorizeMyPasswordSetup(login_entry_id, csrf()),
    onSuccess: ({ authorization_url }) =>
      window.location.assign(authorization_url)
  });
  const setup = useMutation({
    mutationFn: (input: { new_password: string }) =>
      setMyFirstPassword(input, csrf()),
    onSuccess: onSessionEnded,
    onError: () => {
      void queryClient.invalidateQueries({
        queryKey: authenticationMethodsQueryKey
      });
    }
  });
  const switchAccount = useMutation({ mutationFn: onSwitchAccount });
  const pending =
    authorize.isPending ||
    unlink.isPending ||
    verify.isPending ||
    setup.isPending ||
    switchAccount.isPending;
  const errorMessages: Record<string, string> = {
    verification_failed: t('authentication.verification_failed'),
    provider_unavailable: t('authentication.provider_unavailable'),
    entry_unavailable: t('authentication.provider_unavailable'),
    invalid_state: t('authentication.invalid_state'),
    session_expired: t('authentication.session_expired'),
    cancelled: t('authentication.authorization_cancelled')
  };
  if (query.isPending) return <LoadingState />;
  if (query.isError)
    return (
      <Alert
        showIcon
        type="error"
        title={t('authentication.load_failed')}
        action={
          <Button onClick={() => void query.refetch()}>
            {t('authentication.retry')}
          </Button>
        }
      />
    );
  const methods = query.data;
  return (
    <Space orientation="vertical" size="middle" style={{ width: '100%' }}>
      <Typography.Title level={3}>
        {t('auto.security_settings')}
      </Typography.Title>
      {callbackError === 'identity_exists' ? (
        <Alert
          type="warning"
          showIcon
          title={t('authentication.identity_exists')}
          description={
            <Space orientation="vertical">
              <Typography.Text>
                {t('authentication.migration_description')}
              </Typography.Text>
              <Space wrap>
                <Button
                  disabled={!csrfToken}
                  loading={switchAccount.isPending}
                  onClick={() => switchAccount.mutate()}
                >
                  {t('authentication.switch_account')}
                </Button>
                <Button
                  onClick={() => {
                    setCancelledError(true);
                    const url = new URL(window.location.href);
                    url.searchParams.delete('auth_error');
                    window.history.replaceState(window.history.state, '', url);
                  }}
                >
                  {t('profile.actions.cancel')}
                </Button>
              </Space>
            </Space>
          }
        />
      ) : callbackError ? (
        <Alert
          type="error"
          showIcon
          title={
            errorMessages[callbackError] ?? t('authentication.action_failed')
          }
        />
      ) : null}
      {params.get('auth_bound') === 'true' ? (
        <Alert
          showIcon
          type="success"
          title={t('authentication.binding_succeeded')}
        />
      ) : null}
      {authorize.isError ||
      unlink.isError ||
      verify.isError ||
      setup.isError ||
      switchAccount.isError ? (
        <Alert
          showIcon
          type="error"
          title={t('authentication.action_failed')}
        />
      ) : null}
      <Tabs
        defaultActiveKey="password"
        items={[
          {
            key: 'password',
            label: t('authentication.password_tab'),
            children: (
              <PasswordAuthenticationMethod
                methods={methods}
                disabled={!csrfToken || pending}
                changing={changingPassword}
                setting={setup.isPending}
                authorizingEntryId={
                  verify.isPending ? verify.variables : undefined
                }
                changeError={changePasswordError}
                onChangePassword={onChangePassword}
                onAuthorize={(id) => verify.mutate(id)}
                onSetPassword={(input) => setup.mutate(input)}
              />
            )
          },
          ...methods.providers.map((provider) => ({
            key: provider.auth_type,
            label: provider.display_name,
            children: (
              <ExternalProviderMethods
                provider={provider}
                disabled={!csrfToken || pending}
                authorizingEntryId={
                  authorize.isPending ? authorize.variables : undefined
                }
                unlinkingIdentityId={
                  unlink.isPending ? unlink.variables : undefined
                }
                onAuthorize={(id) => authorize.mutate(id)}
                onUnlink={(id) => unlink.mutate(id)}
              />
            )
          }))
        ]}
      />
    </Space>
  );
}
