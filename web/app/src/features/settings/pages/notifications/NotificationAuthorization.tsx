import { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Button, QRCode, Space, Tag, Typography } from 'antd';
import CheckCircleOutlined from '@ant-design/icons/es/icons/CheckCircleOutlined';
import ClockCircleOutlined from '@ant-design/icons/es/icons/ClockCircleOutlined';
import CloseCircleOutlined from '@ant-design/icons/es/icons/CloseCircleOutlined';
import InfoCircleOutlined from '@ant-design/icons/es/icons/InfoCircleOutlined';
import MessageOutlined from '@ant-design/icons/es/icons/MessageOutlined';
import QrcodeOutlined from '@ant-design/icons/es/icons/QrcodeOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { useTranslation } from 'react-i18next';
import type {
  NotificationAuthorizationDescriptor,
  NotificationAuthorizationOperation,
  NotificationAuthorizationView
} from '@1flowbase/api-client';
import { authorizeNotificationInstance } from '../../../notifications/api/queries';
import { NotificationDeliveryWindow } from './NotificationDeliveryWindow';
import { useAuthStore } from '../../../../state/auth-store';

export function NotificationAuthorization({
  instance_id,
  display_name,
  description,
  descriptor,
  authorization,
  beginOnMount = false,
  onChanged
}: {
  instance_id: string;
  display_name?: string;
  description?: string;
  descriptor: NotificationAuthorizationDescriptor;
  authorization: NotificationAuthorizationView;
  beginOnMount?: boolean;
  onChanged: () => void;
}) {
  const { t } = useTranslation('notifications');
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const [view, setView] = useState(authorization);
  const [busy, setBusy] =
    useState<NotificationAuthorizationOperation['type']>();
  const [failed, setFailed] = useState(false);
  const request = useRef<AbortController | null>(null);
  const changed = useRef(onChanged);
  changed.current = onChanged;
  const command = useCallback(
    async (operation: NotificationAuthorizationOperation) => {
      request.current?.abort();
      const controller = new AbortController();
      request.current = controller;
      setBusy(operation.type);
      setFailed(false);
      try {
        const result = await authorizeNotificationInstance(
          instance_id,
          operation,
          csrfToken,
          controller.signal
        );
        if (controller.signal.aborted) return;
        setView(result);
        changed.current();
      } catch {
        if (!controller.signal.aborted) setFailed(true);
      } finally {
        if (!controller.signal.aborted) setBusy(undefined);
      }
    },
    [instance_id, csrfToken]
  );

  useEffect(() => {
    request.current?.abort();
    setView(authorization);
    setBusy(undefined);
    setFailed(false);
  }, [authorization]);
  useEffect(() => () => request.current?.abort(), []);
  const firstAction = descriptor.actions.find(
    (action) => action.kind === 'qr_code'
  );
  const autoStarted = useRef(false);
  useEffect(() => {
    if (beginOnMount && firstAction && !autoStarted.current) {
      const timer = window.setTimeout(() => {
        autoStarted.current = true;
        void command({ type: 'begin', action: firstAction.code });
      }, 0);
      return () => window.clearTimeout(timer);
    }
  }, [beginOnMount, firstAction, command]);
  useEffect(() => {
    if (view.status !== 'pending' || !view.user_action || failed || busy)
      return;
    const timer = window.setTimeout(() => {
      void command({ type: 'poll' });
    }, view.user_action.poll_interval_seconds * 1000);
    return () => window.clearTimeout(timer);
  }, [view, failed, busy, command]);

  const statusLabel =
    view.status === 'authorized'
      ? t('authorization_authorized')
      : view.status === 'pending'
        ? t('authorization_pending')
        : view.status === 'invalid'
          ? t('authorization_invalid')
          : t('authorization_unauthorized');
  const authorized = view.status === 'authorized';
  const StatusIcon = authorized
    ? CheckCircleOutlined
    : view.status === 'invalid'
      ? CloseCircleOutlined
      : ClockCircleOutlined;
  return (
    <section
      className="notification-authorization"
      aria-label={t('channel_authorization')}
    >
      <div className="notification-authorization__heading">
        <h3>{t('channel_authorization')}</h3>
        <Tag
          className="notification-status"
          color={
            authorized
              ? 'success'
              : view.status === 'invalid'
                ? 'error'
                : view.status === 'pending'
                  ? 'warning'
                  : 'default'
          }
          icon={<StatusIcon />}
        >
          {statusLabel}
        </Tag>
        {view.status !== 'pending' && (
          <div className="notification-authorization__actions">
            {authorized && (
              <Button
                shape="round"
                icon={<ReloadOutlined />}
                disabled={!!busy}
                loading={busy === 'maintain'}
                onClick={() => void command({ type: 'maintain' })}
              >
                {t('delivery_window_refresh')}
              </Button>
            )}
            {descriptor.actions.flatMap((action) => action.kind === 'qr_code' ? [
                <Button
                  key={action.code}
                  type={authorized ? 'text' : 'default'}
                  shape="round"
                  icon={<QrcodeOutlined />}
                  disabled={!!busy}
                  loading={busy === 'begin'}
                  aria-label={
                    view.status === 'invalid' || authorized
                      ? t('authorization_reauthorize', { action: action.label })
                      : action.label
                  }
                  onClick={() =>
                    void command({ type: 'begin', action: action.code })
                  }
                >
                  {view.status === 'invalid' || authorized
                    ? t('reauthorize')
                    : action.label}
                </Button>
              ] : [])}
          </div>
        )}
      </div>
      <div className="notification-authorization__panel">
        <div className="notification-authorization__summary">
          <div className="notification-authorization__icon">
            <MessageOutlined aria-hidden />
          </div>
          <div className="notification-authorization__identity">
            <h4>{display_name ?? t('authorization_status')}</h4>
            <p>
              {authorized
                ? t('authorization_ready')
                : (description ?? statusLabel)}
            </p>
          </div>
          <StatusIcon
            className={`notification-authorization__result${authorized ? ' notification-authorization__result--success' : view.status === 'invalid' ? ' notification-authorization__result--error' : ''}`}
            aria-hidden
          />
        </div>
        {view.delivery_window && (
          <NotificationDeliveryWindow delivery_window={view.delivery_window} />
        )}
        {view.status === 'pending' && view.user_action?.kind === 'qr_code' && (
          <div className="notification-authorization__qr">
            <QRCode value={view.user_action.qr_code_url} />
            <Typography.Text>{t('authorization_waiting')}</Typography.Text>
            <Typography.Text type="secondary">
              {t('authorization_not_delivery')}
            </Typography.Text>
            <Space wrap>
              <Button
                loading={busy === 'cancel'}
                onClick={() => void command({ type: 'cancel' })}
              >
                {t('authorization_cancel')}
              </Button>
              {failed && (
                <Button onClick={() => void command({ type: 'poll' })}>
                  {t('refresh')}
                </Button>
              )}
            </Space>
          </div>
        )}
      </div>
      {authorized && (
        <p className="notification-authorization__hint">
          <InfoCircleOutlined aria-hidden />
          <span>
            {view.delivery_window
              ? t('delivery_window_message_hint')
              : t('authorization_session_hint')}
          </span>
        </p>
      )}
      {failed && (
        <Alert type="error" showIcon title={t('authorization_failed')} />
      )}
    </section>
  );
}
