import { useCallback, useState } from 'react';
import BellOutlined from '@ant-design/icons/es/icons/BellOutlined';
import {
  Alert,
  Badge,
  Button,
  Divider,
  Empty,
  Flex,
  Grid,
  Menu,
  Space,
  Spin,
  Splitter,
  Tabs,
  Typography
} from 'antd';
import { useTranslation } from 'react-i18next';
import type { NotificationReadStatus } from '@1flowbase/api-client';
import { formatDateTime } from '../../../shared/i18n/format';
import { ResizableDrawer } from '../../../shared/ui/resizable-drawer/ResizableDrawer';
import { useNotificationInbox } from '../hooks/use-notification-inbox';
import {
  persistInboxSplitRatio,
  readInboxSplitRatio
} from '../lib/inbox-split-ratio';
import './notification-bell.css';

function useNotificationBellController() {
  const { t } = useTranslation('notifications');
  const [open, setOpen] = useState(false);
  const [workspaceWidth, setWorkspaceWidth] = useState(0);
  const [splitRatio, setSplitRatio] = useState(readInboxSplitRatio);
  const screens = Grid.useBreakpoint();
  const compact = workspaceWidth <= 640 || screens.md === false;
  const channelWidth = Math.min(
    workspaceWidth - 320,
    Math.max(280, splitRatio === null ? 350 : workspaceWidth * splitRatio)
  );
  const inbox = useNotificationInbox(open);
  const observeWorkspace = useCallback((node: HTMLDivElement | null) => {
    if (!node) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry.contentRect.width > 0) {
        setWorkspaceWidth(entry.contentRect.width);
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  return {
    inbox, open, setOpen, t, observeWorkspace, compact, setSplitRatio, channelWidth,
    workspaceWidth
  };
}

export function NotificationBell() {
  const {
    inbox, open, setOpen, t, observeWorkspace, compact, setSplitRatio, channelWidth,
    workspaceWidth
  } = useNotificationBellController();
  if (!inbox.authenticated) return null;
  const failed =
    inbox.channels.isError ||
    inbox.messages.isError ||
    inbox.unread.isError ||
    inbox.write.isError;
  return (
    <>
      <Menu
        className="notification-bell-menu"
        mode="horizontal"
        selectedKeys={open ? ['notifications'] : []}
        disabledOverflow
        onClick={() => setOpen(true)}
        items={[
          {
            key: 'notifications',
            label: (
              <span
                className="notification-bell-menu__icon"
                aria-label={t('inbox')}
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
      <ResizableDrawer
        title={t('inbox')}
        open={open}
        onClose={() => setOpen(false)}
        defaultWidth={900}
        minWidth={480}
        maxWidth={1200}
        resizeLabel={t('resize_drawer')}
        rootClassName="notification-inbox"
        bodyClassName="notification-inbox__body"
      >
        {!inbox.connected && <Alert type="info" title={t('reconnecting')} />}
        {failed && <Alert type="error" showIcon title={t('inbox_failed')} />}
        <div
          ref={observeWorkspace}
          className={`notification-inbox__workspace${compact ? ' notification-inbox__workspace--compact' : ''}`}
        >
          <Splitter
            classNames={{
              dragger: { default: 'notification-inbox__split-dragger' }
            }}
            onResize={(sizes) => {
              if (!compact) setSplitRatio(sizes[0] / (sizes[0] + sizes[1]));
            }}
            onResizeEnd={(sizes) => {
              if (!compact)
                persistInboxSplitRatio(sizes[0] / (sizes[0] + sizes[1]));
            }}
          >
            <Splitter.Panel
              min={compact ? 0 : 280}
              size={compact ? (inbox.selected ? '0%' : '100%') : channelWidth}
              resizable={!compact}
              style={{ overflow: 'hidden' }}
            >
              <aside
                className="notification-inbox__channels"
                aria-label={t('channels')}
              >
                <Tabs
                  activeKey={inbox.read_status}
                  onChange={(value) =>
                    inbox.setReadStatus(value as NotificationReadStatus)
                  }
                  items={[
                    { key: 'all', label: t('all_messages') },
                    { key: 'unread', label: t('unread') },
                    { key: 'read', label: t('read') }
                  ]}
                />
                <div className="notification-inbox__channel-list">
                  {inbox.channels.isPending ? (
                    <Spin />
                  ) : inbox.channels.data?.items.length === 0 ? (
                    <Empty description={t('no_channels')} />
                  ) : (
                    inbox.channels.data?.items.map((channel) => (
                      <button
                        type="button"
                        key={channel.instance_id}
                        className="notification-inbox__channel"
                        aria-pressed={
                          inbox.selected?.instance_id === channel.instance_id
                        }
                        onClick={() => inbox.select(channel)}
                      >
                        <Flex justify="space-between" gap="small">
                          <span className="notification-inbox__channel-name">
                            {channel.display_name}
                          </span>
                          <Badge count={channel.unread_count} size="small" />
                        </Flex>
                        {channel.deleted && (
                          <span className="notification-inbox__muted">
                            {t('deleted_channel')}
                          </span>
                        )}
                        <span className="notification-inbox__summary">
                          {channel.latest_title}
                        </span>
                        <span className="notification-inbox__summary notification-inbox__muted">
                          {channel.latest_content}
                        </span>
                        <time className="notification-inbox__muted">
                          {formatDateTime(channel.latest_created_at)}
                        </time>
                      </button>
                    ))
                  )}
                </div>
                <Flex
                  gap="small"
                  justify="end"
                  className="notification-inbox__pagination"
                >
                  <Button
                    disabled={
                      !inbox.channelsHasPrevious || inbox.channels.isFetching
                    }
                    onClick={inbox.previousChannels}
                  >
                    {t('previous')}
                  </Button>
                  <Button
                    disabled={
                      !inbox.channels.data?.next_cursor ||
                      inbox.channels.isFetching
                    }
                    onClick={inbox.nextChannels}
                  >
                    {t('next')}
                  </Button>
                </Flex>
              </aside>
            </Splitter.Panel>
            <Splitter.Panel
              min={compact ? 0 : 320}
              size={
                compact
                  ? inbox.selected
                    ? '100%'
                    : '0%'
                  : workspaceWidth - channelWidth
              }
              resizable={!compact}
              style={{ overflow: 'hidden' }}
            >
              <section
                className="notification-inbox__messages"
                aria-label={t('channel_messages')}
              >
                <Flex
                  gap="small"
                  justify="space-between"
                  align="center"
                  wrap
                  className="notification-inbox__toolbar"
                >
                  {inbox.selected && (
                    <Typography.Text
                      strong
                      className="notification-inbox__channel-heading"
                    >
                      {inbox.selected.display_name}
                    </Typography.Text>
                  )}
                  <Space wrap>
                    <Button
                      type="text"
                      className="notification-inbox__back"
                      onClick={() => inbox.select(undefined)}
                    >
                      {t('back_channels')}
                    </Button>
                    <Button onClick={() => void inbox.refresh()}>
                      {t('refresh')}
                    </Button>
                    <Button
                      disabled={
                        !inbox.selected ||
                        !inbox.messages.data?.unread_count ||
                        inbox.write.isPending
                      }
                      loading={
                        inbox.write.isPending &&
                        inbox.write.variables?.operation === 'channel'
                      }
                      onClick={inbox.readAll}
                    >
                      {t('read_all')}
                    </Button>
                  </Space>
                </Flex>
                <div className="notification-inbox__message-list">
                  {!inbox.selected ? (
                    <Empty description={t('select_channel')} />
                  ) : inbox.messages.isPending ? (
                    <Spin />
                  ) : inbox.messages.data?.items.length === 0 ? (
                    <Empty description={t('empty')} />
                  ) : (
                    inbox.messages.data?.items.map((item) => (
                      <article key={item.id}>
                        <Flex justify="space-between" align="start" gap="small">
                          <Button
                            type="link"
                            className="notification-inbox__message-title"
                            disabled={inbox.write.isPending}
                            onClick={() => {
                              if (!item.read_at) inbox.markRead(item.id);
                            }}
                          >
                            {item.title}
                          </Button>
                          <Badge
                            status={item.read_at ? 'default' : 'processing'}
                            text={item.read_at ? t('read') : t('unread')}
                          />
                        </Flex>
                        <Typography.Paragraph className="notification-inbox__content">
                          {item.content}
                        </Typography.Paragraph>
                        <Flex
                          justify="space-between"
                          align="center"
                          gap="small"
                        >
                          <Typography.Text type="secondary">
                            {formatDateTime(item.created_at)}
                          </Typography.Text>
                          <Button
                            type="text"
                            size="small"
                            disabled={inbox.write.isPending}
                            loading={
                              inbox.write.isPending &&
                              inbox.write.variables?.id === item.id
                            }
                            onClick={() =>
                              item.read_at
                                ? inbox.markUnread(item.id)
                                : inbox.markRead(item.id)
                            }
                          >
                            {item.read_at ? t('mark_unread') : t('mark_read')}
                          </Button>
                        </Flex>
                        <Divider />
                      </article>
                    ))
                  )}
                </div>
                {inbox.selected && (
                  <Flex
                    gap="small"
                    justify="end"
                    className="notification-inbox__pagination"
                  >
                    <Button
                      disabled={
                        !inbox.messagesHasPrevious || inbox.messages.isFetching
                      }
                      onClick={inbox.previousMessages}
                    >
                      {t('previous')}
                    </Button>
                    <Button
                      disabled={
                        !inbox.messages.data?.next_cursor ||
                        inbox.messages.isFetching
                      }
                      onClick={inbox.nextMessages}
                    >
                      {t('next')}
                    </Button>
                  </Flex>
                )}
              </section>
            </Splitter.Panel>
          </Splitter>
        </div>
      </ResizableDrawer>
    </>
  );
}
