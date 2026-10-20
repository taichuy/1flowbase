import { useLayoutEffect, useRef } from 'react';
import { NotificationSceneEventSource } from './notifications/data';
import { NotificationBell } from '../../features/notifications/components/NotificationBell';
import { NotificationLogs } from '../../features/settings/pages/notifications/NotificationLogs';
import { NotificationsPage } from '../../features/settings/pages/notifications/NotificationsPage';

export function NotificationSettingsStyleBoundaryScene() {
  return <NotificationsPage />;
}

export function NotificationLogsStyleBoundaryScene() {
  return <NotificationLogs />;
}

export function NotificationInboxStyleBoundaryScene({
  selectChannel = true,
  width = 900
}: { selectChannel?: boolean; width?: number } = {}) {
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const originalEventSource = globalThis.EventSource;
    globalThis.EventSource =
      NotificationSceneEventSource as unknown as typeof EventSource;
    localStorage.setItem(
      `resizable-drawer:width:${window.location.pathname}`,
      String(width)
    );
    root.current
      ?.querySelector<HTMLElement>('.notification-bell-menu [role="menuitem"]')
      ?.click();
    const observer = new MutationObserver(() => {
      const channel = document.querySelector<HTMLButtonElement>(
        '.notification-inbox__channel'
      );
      if (selectChannel && channel) {
        channel.click();
        observer.disconnect();
      }
    });
    if (selectChannel)
      observer.observe(document.body, { childList: true, subtree: true });
    return () => {
      observer.disconnect();
      globalThis.EventSource = originalEventSource;
    };
  }, [selectChannel, width]);
  return (
    <div ref={root} data-testid="notification-bell-scene">
      <NotificationBell />
    </div>
  );
}
