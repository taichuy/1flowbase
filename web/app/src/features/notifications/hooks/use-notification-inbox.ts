import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  NotificationChannel,
  NotificationReadStatus
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import {
  markNotificationChannelRead,
  markNotificationRead,
  markNotificationUnread,
  notificationChannelsOptions,
  notificationMessagesOptions,
  notificationQueryKey,
  notificationUnreadOptions,
  subscribeNotificationChanges
} from '../api/queries';

type CursorState = { cursor?: string; previous: Array<string | undefined> };
const firstPage = (): CursorState => ({ previous: [] });
type InboxState = {
  scope: string;
  selected?: NotificationChannel;
  read_status: NotificationReadStatus;
  channels: CursorState;
  messages: CursorState;
};
const initialState = (scope: string): InboxState => ({
  scope,
  read_status: 'all',
  channels: firstPage(),
  messages: firstPage()
});

export function useNotificationInbox(open: boolean) {
  const scope = useAuthStore(selectNavigationQueryScope);
  const authenticated = useAuthStore(
    (state) => state.sessionStatus === 'authenticated'
  );
  const csrfToken = useAuthStore((state) => state.csrfToken) ?? '';
  const client = useQueryClient();
  const [stored, setStored] = useState(() => initialState(scope));
  // Gate selection synchronously: no render may request a previous workspace channel.
  const state = stored.scope === scope ? stored : initialState(scope);
  const update = (change: (current: InboxState) => InboxState) =>
    setStored((current) =>
      change(current.scope === scope ? current : initialState(scope))
    );
  const [connection, setConnection] = useState({ scope, connected: false });
  const refresh = () =>
    client.invalidateQueries({ queryKey: notificationQueryKey(scope) });
  const unread = useQuery({
    ...notificationUnreadOptions(scope),
    enabled: authenticated
  });
  const channels = useQuery({
    ...notificationChannelsOptions(scope, {
      cursor: state.channels.cursor,
      page_size: 20,
      read_status: state.read_status
    }),
    enabled: authenticated && open
  });
  const messages = useQuery({
    ...notificationMessagesOptions(scope, {
      cursor: state.messages.cursor,
      page_size: 20,
      read_status: state.read_status,
      instance_id: state.selected?.instance_id
    }),
    enabled: authenticated && open && !!state.selected
  });
  const channelsData = useSnapshot(
    channels.data,
    JSON.stringify([scope, state.read_status])
  );
  const messagesData = useSnapshot(
    messages.data,
    JSON.stringify([scope, state.read_status, state.selected?.instance_id])
  );
  const selected =
    channelsData?.items.find(
      (item) => item.instance_id === state.selected?.instance_id
    ) ?? state.selected;
  const write = useMutation({
    mutationFn: async ({
      id,
      operation,
      requestScope
    }: {
      id: string;
      operation: 'read' | 'unread' | 'channel';
      requestScope: string;
    }) => {
      if (operation === 'read') await markNotificationRead(id, csrfToken);
      else if (operation === 'unread')
        await markNotificationUnread(id, csrfToken);
      else await markNotificationChannelRead(id, csrfToken);
      return requestScope;
    },
    onSuccess: (requestScope) =>
      client.invalidateQueries({ queryKey: notificationQueryKey(requestScope) })
  });
  const resetWrite = write.reset;
  useEffect(() => {
    setStored(initialState(scope));
    setConnection({ scope, connected: false });
    resetWrite();
    if (!authenticated) return;
    return subscribeNotificationChanges(
      () =>
        void client.invalidateQueries({
          queryKey: notificationQueryKey(scope)
        }),
      (connected) => setConnection({ scope, connected })
    );
  }, [authenticated, client, resetWrite, scope]);
  const paginate = (
    section: 'channels' | 'messages',
    direction: 'next' | 'previous'
  ) =>
    update((current) => {
      const pagination = current[section];
      const next_cursor =
        section === 'channels'
          ? channelsData?.next_cursor
          : messagesData?.next_cursor;
      if (direction === 'next' && !next_cursor) return current;
      return {
        ...current,
        [section]:
          direction === 'next'
            ? {
                cursor: next_cursor,
                previous: [...pagination.previous, pagination.cursor]
              }
            : {
                cursor: pagination.previous.at(-1),
                previous: pagination.previous.slice(0, -1)
              }
      };
    });
  return {
    authenticated,
    unread,
    channels: {
      ...channels,
      data: channelsData,
      isPending: channels.isPending && !channelsData
    },
    messages: {
      ...messages,
      data: messagesData,
      isPending: messages.isPending && !messagesData
    },
    selected,
    write,
    refresh,
    connected: connection.scope === scope && connection.connected,
    read_status: state.read_status,
    setReadStatus: (read_status: NotificationReadStatus) => {
      resetWrite();
      update((current) => ({
        ...current,
        read_status,
        channels: firstPage(),
        messages: firstPage()
      }));
    },
    select: (selected?: NotificationChannel) => {
      resetWrite();
      update((current) => ({ ...current, selected, messages: firstPage() }));
    },
    markRead: (id: string) =>
      write.mutate({ id, operation: 'read', requestScope: scope }),
    markUnread: (id: string) =>
      write.mutate({ id, operation: 'unread', requestScope: scope }),
    readAll: () => {
      if (selected)
        write.mutate({
          id: selected.instance_id,
          operation: 'channel',
          requestScope: scope
        });
    },
    channelsHasPrevious: state.channels.previous.length > 0,
    messagesHasPrevious: state.messages.previous.length > 0,
    previousChannels: () => paginate('channels', 'previous'),
    nextChannels: () => paginate('channels', 'next'),
    previousMessages: () => paginate('messages', 'previous'),
    nextMessages: () => paginate('messages', 'next')
  };
}

// Retain the last HTTP snapshot on a failed refresh or page fetch. The identity
// excludes the cursor, but includes scope and filters so data never crosses them.
function useSnapshot<T>(data: T | undefined, identity: string): T | undefined {
  const [last, setLast] = useState<{ identity: string; data: T }>();
  useEffect(() => {
    if (data !== undefined) setLast({ identity, data });
  }, [data, identity]);
  return data ?? (last?.identity === identity ? last.data : undefined);
}
