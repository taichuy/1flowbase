import { Suspense, lazy, useState } from 'react';
import BellOutlined from '@ant-design/icons/es/icons/BellOutlined';
import { Badge, Menu } from 'antd';
import { useTranslation } from 'react-i18next';
import { appI18n, applicationI18nLoader } from '../../../shared/i18n/app-i18n';
import { resolveAppLocale } from '../../../shared/i18n/locales';
import { useNotificationInbox } from '../hooks/use-notification-inbox';
import './notification-bell.css';

const NotificationInboxDrawer = lazy(async () => {
  const locale = resolveAppLocale(appI18n.resolvedLanguage ?? appI18n.language);
  const [module] = await Promise.all([
    import('./NotificationInboxDrawer'),
    applicationI18nLoader.activatePrimary(locale, ['notifications'])
  ]);
  return { default: module.NotificationInboxDrawer };
});

export function NotificationBell() {
  const { t, i18n } = useTranslation('notifications');
  const [open, setOpen] = useState(false);
  const [hasOpened, setHasOpened] = useState(false);
  const inbox = useNotificationInbox(open);
  if (!inbox.authenticated) return null;

  const fallbackInboxLabel = (i18n.resolvedLanguage ?? i18n.language)?.startsWith(
    'zh'
  )
    ? '通知消息'
    : 'Notifications';

  return (
    <>
      <Menu
        className="notification-bell-menu"
        mode="horizontal"
        selectedKeys={open ? ['notifications'] : []}
        disabledOverflow
        onClick={() => {
          setHasOpened(true);
          setOpen(true);
        }}
        items={[
          {
            key: 'notifications',
            label: (
              <span
                className="notification-bell-menu__icon"
                aria-label={t('inbox', { defaultValue: fallbackInboxLabel })}
              >
                <Badge
                  count={inbox.unread.data?.unread_count}
                  size="small"
                  style={{ color: 'inherit' }}
                >
                  <BellOutlined />
                </Badge>
              </span>
            )
          }
        ]}
      />
      {hasOpened ? (
        <Suspense fallback={null}>
          <NotificationInboxDrawer
            inbox={inbox}
            open={open}
            onClose={() => setOpen(false)}
            t={t}
          />
        </Suspense>
      ) : null}
    </>
  );
}
