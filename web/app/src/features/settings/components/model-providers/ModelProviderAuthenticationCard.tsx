import { Alert, Button, Input, Space, Typography } from 'antd';
import { CheckCircleOutlined } from '@ant-design/icons';

import { i18nText } from '../../../../shared/i18n/text';
import type {
  SettingsAuthenticateModelProviderInstanceResult,
  SettingsModelProviderAuthOperation,
  SettingsModelProviderCatalogEntry
} from '../../api/model-providers';

type AuthenticationResult = SettingsAuthenticateModelProviderInstanceResult;
type ModelProviderAuthProjection = NonNullable<
  SettingsModelProviderCatalogEntry['auth']
>;

export function ModelProviderAuthenticationCard({
  auth,
  authenticationResult,
  authenticationError,
  authenticationRequestPending,
  callbackValue,
  onCallbackValueChange,
  onRunAuthenticationOperation
}: {
  auth: ModelProviderAuthProjection;
  authenticationResult: AuthenticationResult | null;
  authenticationError: string | null;
  authenticationRequestPending: boolean;
  callbackValue: string;
  onCallbackValueChange: (value: string) => void;
  onRunAuthenticationOperation: (
    operation: SettingsModelProviderAuthOperation
  ) => Promise<AuthenticationResult>;
}) {
  const userAction = authenticationResult?.user_action;
  const isPending = authenticationResult?.status === 'pending';
  const authenticationAlert = authenticationError
    ? {
        type: 'error' as const,
        message: i18nText('settings', 'auto.provider_authentication_failed'),
        description: authenticationError
      }
    : authenticationResult
      ? {
          type:
            authenticationResult.status === 'authorized'
              ? ('success' as const)
              : authenticationResult.status === 'failed'
                ? ('error' as const)
                : ('info' as const),
          message:
            authenticationResult.status === 'authorized'
              ? i18nText('settings', 'auto.provider_authenticated')
              : authenticationResult.status === 'cancelled'
                ? i18nText('settings', 'auto.provider_authentication_cancelled')
                : authenticationResult.status === 'failed'
                  ? i18nText('settings', 'auto.provider_authentication_failed')
                  : i18nText(
                      'settings',
                      'auto.provider_authentication_pending'
                    ),
          description: authenticationResult.message
        }
      : null;

  return (
    <div className="model-provider-drawer__card">
      <div className="model-provider-drawer__card-title">
        <CheckCircleOutlined />
        <span>{i18nText('settings', 'auto.provider_authentication')}</span>
      </div>
      <div className="model-provider-drawer__card-body">
        <Space orientation="vertical" size={12} style={{ width: '100%' }}>
          <Typography.Text type="secondary">
            {i18nText('settings', 'auto.provider_authentication_description')}
          </Typography.Text>
          <Space wrap>
            {auth.actions.map((action) => (
              <Button
                key={action.code}
                disabled={isPending || authenticationRequestPending}
                loading={authenticationRequestPending}
                onClick={() => {
                  void onRunAuthenticationOperation({
                    type: 'begin',
                    action: action.code
                  }).catch(() => undefined);
                }}
              >
                {action.label}
              </Button>
            ))}
          </Space>
          {authenticationAlert ? (
            <Alert
              showIcon
              type={authenticationAlert.type}
              message={authenticationAlert.message}
              description={authenticationAlert.description ?? undefined}
            />
          ) : null}
          {userAction ? (
            <Space orientation="vertical" size={8} style={{ width: '100%' }}>
              {userAction.prompt ? (
                <Typography.Text>{userAction.prompt}</Typography.Text>
              ) : null}
              {userAction.user_code ? (
                <Typography.Text code>{userAction.user_code}</Typography.Text>
              ) : null}
              {userAction.open_url ? (
                <Button
                  href={userAction.open_url}
                  target="_blank"
                  rel="noreferrer"
                >
                  {i18nText(
                    'settings',
                    'auto.provider_authentication_open_url'
                  )}
                </Button>
              ) : null}
              {userAction.expires_at ? (
                <Typography.Text type="secondary">
                  {i18nText(
                    'settings',
                    'auto.provider_authentication_expires_at',
                    {
                      value1: userAction.expires_at
                    }
                  )}
                </Typography.Text>
              ) : null}
              {userAction.kind === 'paste_callback_url' ? (
                <Space.Compact block>
                  <Input
                    aria-label={i18nText(
                      'settings',
                      'auto.provider_authentication_callback_value'
                    )}
                    autoComplete="off"
                    value={callbackValue}
                    onChange={(event) =>
                      onCallbackValueChange(event.target.value)
                    }
                    placeholder={i18nText(
                      'settings',
                      'auto.provider_authentication_callback_value'
                    )}
                  />
                  <Button
                    type="primary"
                    disabled={
                      callbackValue.trim().length === 0 ||
                      authenticationRequestPending
                    }
                    loading={authenticationRequestPending}
                    onClick={() => {
                      void onRunAuthenticationOperation({
                        type: 'submit',
                        value: callbackValue.trim()
                      }).catch(() => undefined);
                    }}
                  >
                    {i18nText(
                      'settings',
                      'auto.provider_authentication_submit'
                    )}
                  </Button>
                </Space.Compact>
              ) : null}
            </Space>
          ) : null}
          {isPending ? (
            <Button
              danger
              disabled={authenticationRequestPending}
              loading={authenticationRequestPending}
              onClick={() => {
                void onRunAuthenticationOperation({ type: 'cancel' }).catch(
                  () => undefined
                );
              }}
            >
              {i18nText('settings', 'auto.provider_authentication_cancel')}
            </Button>
          ) : null}
        </Space>
      </div>
    </div>
  );
}
