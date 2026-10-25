import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import type {
  CommunicationMessageDetail,
  CommunicationMessageListItem,
  CommunicationConversationListItem
} from '@1flowbase/api-client';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { NotificationsPage } from '../NotificationsPage';
import { CommunicationHistory } from '../communications/CommunicationHistory';

const summary: CommunicationMessageListItem = {
  id: 'message-1',
  conversation_id: 'conversation-1',
  instance_id: 'channel-1',
  instance_display_name: 'Chat channel',
  external_identity: 'alice',
  sender_display_name: 'Alice',
  received_at_ms: 1792960000000,
  content_preview: 'First request',
  processing_status: 'succeeded',
  send_status: 'failed',
  failure_code: 'send_interrupted',
  can_retry: true,
  run_id: 'original-run',
  application_id: 'original-app'
};
const conversation: CommunicationConversationListItem = {
  id: 'conversation-1',
  workspace_id: 'workspace-1',
  instance_id: 'channel-1',
  external_identity: 'alice',
  external_conversation_key: 'thread-1',
  sender_display_name: 'Alice',
  instance_display_name: 'Chat channel',
  latest_message: summary
};
let detail: CommunicationMessageDetail;
let client: QueryClient;
let calls: URL[];
beforeEach(async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=conversations'
  );
  await appI18n.changeLanguage('en_US');
  calls = [];
  detail = {
    ...summary,
    content: '<script>Full request</script>',
    external_message_key: 'external-1',
    final_reply: 'Final reply',
    attachments: []
  };
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  useAuthStore.setState({
    csrfToken: 'csrf',
    sessionStatus: 'authenticated',
    actor: {
      id: 'user-1',
      current_workspace_id: 'workspace-1',
      effective_display_role: 'root'
    } as NonNullable<ReturnType<typeof useAuthStore.getState>['actor']>
  });
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string) => {
      const url = new URL(input, 'http://localhost');
      calls.push(url);
      const page = Number(url.searchParams.get('page') ?? 1);
      let data: unknown;
      if (url.pathname.endsWith('/instances'))
        data = [{ id: 'channel-1', display_name: 'Chat channel' }];
      else if (url.pathname.endsWith('/types')) data = [];
      else if (url.pathname.endsWith('/messages/message-1')) data = detail;
      else if (url.pathname.endsWith('/retry')) data = detail;
      else if (url.pathname.endsWith('/conversations'))
        data = {
          items: [
            page === 2
              ? {
                  ...conversation,
                  id: 'conversation-2',
                  sender_display_name: 'Bob'
                }
              : conversation
          ],
          total: 21,
          page,
          page_size: 20
        };
      else if (url.pathname.endsWith('/messages'))
        data = {
          items: [
            page === 2
              ? {
                  ...summary,
                  id: 'message-2',
                  content_preview: 'Earlier request'
                }
              : summary
          ],
          total: 21,
          page,
          page_size: 20
        };
      else if (url.pathname.endsWith('/logs'))
        data = { items: [], total: 0, page: 1, page_size: 20 };
      else throw new Error(`Unexpected request: ${url.pathname}`);
      return new Response(JSON.stringify({ data, meta: null }), {
        headers: { 'content-type': 'application/json' }
      });
    })
  );
});
afterEach(async () => {
  await waitFor(() => {
    expect(client.isFetching()).toBe(0);
    expect(client.isMutating()).toBe(0);
  });
  cleanup();
  client.clear();
  resetAuthStore();
  vi.unstubAllGlobals();
  window.history.replaceState({}, '', '/');
  await appI18n.changeLanguage('zh_Hans');
});
async function show(component = <CommunicationHistory />) {
  let view!: ReturnType<typeof render>;
  await act(async () => {
    view = render(
      <QueryClientProvider client={client}>{component}</QueryClientProvider>
    );
  });
  return view;
}
async function click(element: HTMLElement) {
  await act(async () => {
    fireEvent.click(element);
  });
}

test('lists conversations across channels immediately with shared table, statuses, links and server pagination', async () => {
  await show();
  const alice = await screen.findByRole('link', { name: 'Alice' });
  expect(alice.closest('.data-table')).not.toBeNull();
  expect(alice).toHaveAttribute(
    'href',
    expect.stringContaining('conversation_id=conversation-1')
  );
  expect(screen.getByText('Processed')).toBeInTheDocument();
  expect(screen.getByText('Delivery failed')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'View run logs' })).toHaveAttribute(
    'href',
    '/applications/original-app/logs?run_id=original-run&view=trace'
  );
  const request = calls.find((url) => url.pathname.endsWith('/conversations'))!;
  expect(request.searchParams.has('instance_id')).toBe(false);
  await click(screen.getByTitle('2'));
  await screen.findByRole('link', { name: 'Bob' });
  expect(window.location.search).toContain('page=2');
  expect(
    calls.some(
      (url) =>
        url.pathname.endsWith('/conversations') &&
        url.searchParams.get('page') === '2'
    )
  ).toBe(true);
});

test('received records are messages with URL filters, exact detail, plain content and authoritative retry', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=logs&record_tab=received&processing_status=succeeded&send_status=failed'
  );
  await show(<CommunicationHistory records />);
  await click(await screen.findByRole('link', { name: 'First request' }));
  const drawer = within(await screen.findByRole('dialog'));
  await drawer.findByText('<script>Full request</script>');
  expect(drawer.getByText('Final reply')).toBeInTheDocument();
  expect(document.querySelector('.notification-log-detail script')).toBeNull();
  expect(window.location.search).toContain('message_id=message-1');
  expect(
    calls.some(
      (url) =>
        url.pathname.endsWith('/messages') &&
        url.searchParams.get('processing_status') === 'succeeded' &&
        url.searchParams.get('send_status') === 'failed' &&
        !url.searchParams.has('instance_id')
    )
  ).toBe(true);
  await click(drawer.getByRole('button', { name: 'Retry delivery' }));
  await waitFor(() =>
    expect(
      calls.some((url) =>
        url.pathname.endsWith('/instances/channel-1/messages/message-1/retry')
      )
    ).toBe(true)
  );
  detail = { ...detail, can_retry: false };
  await click(drawer.getByRole('button', { name: 'Refresh' }));
  await waitFor(() =>
    expect(
      drawer.queryByRole('button', { name: 'Retry delivery' })
    ).not.toBeInTheDocument()
  );
});

test('conversation deep link opens its message table and preserves the parent when opening a message', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=conversations&conversation_id=conversation-1&conversation_page=2'
  );
  await show();
  const drawer = within(await screen.findByRole('dialog'));
  await drawer.findByRole('link', { name: 'Earlier request' });
  expect(
    calls.some(
      (url) =>
        url.pathname.endsWith('/messages') &&
        url.searchParams.get('conversation_id') === 'conversation-1' &&
        url.searchParams.get('page') === '2'
    )
  ).toBe(true);
  await click(drawer.getByTitle('1'));
  await click(await drawer.findByRole('link', { name: 'First request' }));
  await drawer.findByText('Final reply');
  await click(drawer.getByRole('link', { name: 'Back to conversation' }));
  await drawer.findByRole('link', { name: 'First request' });
  expect(window.location.search).toContain('conversation_id=conversation-1');
  expect(window.location.search).not.toContain('message_id');
});

test('tabs and received selection restore from URL after remount and browser history navigation', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=logs&record_tab=received&message_id=message-1'
  );
  const view = await show(<NotificationsPage />);
  await screen.findByText('Final reply');
  const recordTab = screen.getByRole('tab', {
    name: appI18n.t('notifications:send_receive_records')
  });
  expect(recordTab).toHaveAttribute('aria-selected', 'true');
  await click(screen.getByRole('button', { name: 'Close' }));
  await click(
    screen.getByRole('tab', { name: appI18n.t('notifications:logs') })
  );
  expect(window.location.search).toContain('record_tab=sent');
  view.unmount();
  await show(<NotificationsPage />);
  expect(
    screen.getByRole('tab', { name: appI18n.t('notifications:logs') })
  ).toHaveAttribute('aria-selected', 'true');
  await act(async () => {
    window.history.back();
  });
  await waitFor(() =>
    expect(
      screen.getByRole('tab', {
        name: appI18n.t('notifications:received_records')
      })
    ).toHaveAttribute('aria-selected', 'true')
  );
  await act(async () => {
    window.history.forward();
  });
  await waitFor(() =>
    expect(
      screen.getByRole('tab', { name: appI18n.t('notifications:logs') })
    ).toHaveAttribute('aria-selected', 'true')
  );
});

test('opening a conversation from a later received page resets the destination list page', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=logs&record_tab=received&page=2'
  );
  await show(<NotificationsPage />);
  await screen.findByRole('link', { name: 'Earlier request' });
  await click(screen.getByRole('link', { name: 'Alice' }));
  await screen.findByRole('dialog');
  expect(new URLSearchParams(window.location.search).get('tab')).toBe(
    'conversations'
  );
  expect(new URLSearchParams(window.location.search).get('page')).toBe('1');
  await waitFor(() =>
    expect(
      calls.some(
        (url) =>
          url.pathname.endsWith('/conversations') &&
          url.searchParams.get('page') === '1'
      )
    ).toBe(true)
  );
});
