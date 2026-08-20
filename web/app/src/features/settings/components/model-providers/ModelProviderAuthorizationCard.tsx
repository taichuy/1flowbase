import { CheckCircleOutlined } from '@ant-design/icons';
import { Alert, Button, Input, Space, Typography } from 'antd';

import type {
  SettingsAuthenticateModelProviderInstanceResult,
  SettingsModelProviderCatalogEntry
} from '../../api/model-providers';
import { i18nText } from '../../../../shared/i18n/text';

type ModelProviderAuthorizationCardProps = {
  catalogEntry: SettingsModelProviderCatalogEntry;
  result: SettingsAuthenticateModelProviderInstanceResult | null;
  errorMessage: string | null;
  pending: boolean;
  callbackValue: string;
  onCallbackValueChange: (value: string) => void;
  onBegin: (action: string) => void;
  onSubmit: (value: string) => void;
  onCancel: () => void;
};

export function ModelProviderAuthorizationCard({
  catalogEntry,
  result,
  errorMessage,
  pending,
  callbackValue,
  onCallbackValueChange,
  onBegin,
  onSubmit,
  onCancel
}: ModelProviderAuthorizationCardProps) {
  const userAction = result?.user_action;
  const isPending = result?.status === 'pending';
  const authenticationAlert = errorMessage
    ? {
        type: 'error' as const,
        message: i18nText('settings', 'auto.provider_authentication_failed'),
        description: errorMessage
      }
    : result
      ? {
          type:
            result.status === 'authorized'
              ? ('success' as const)
              : result.status === 'failed'
                ? ('error' as const)
                : ('info' as const),
          message:
            result.status === 'authorized'
              ? i18nText('settings', 'auto.provider_authenticated')
              : result.status === 'cancelled'
                ? i18nText('settings', 'auto.provider_authentication_cancelled')
                : result.status === 'failed'
                  ? i18nText('settings', 'auto.provider_authentication_failed')
                  : i18nText(
                      'settings',
                      'auto.provider_authentication_pending'
                    ),
          description: result.message
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
            {catalogEntry.auth?.actions.map((action) => (
              <Button
                key={action.code}
                disabled={isPending || pending}
                loading={pending}
                onClick={() => onBegin(action.code)}
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
                    { value1: userAction.expires_at }
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
                    disabled={callbackValue.trim().length === 0 || pending}
                    loading={pending}
                    onClick={() => onSubmit(callbackValue.trim())}
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
              disabled={pending}
              loading={pending}
              onClick={onCancel}
            >
              {i18nText('settings', 'auto.provider_authentication_cancel')}
            </Button>
          ) : null}
        </Space>
      </div>
    </div>
  );
}
