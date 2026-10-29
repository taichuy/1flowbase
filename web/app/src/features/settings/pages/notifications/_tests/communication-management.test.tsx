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
  CommunicationMessage,
  CommunicationMessageListItem,
  CommunicationConversationListItem
} from '@1flowbase/api-client';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';
import { NotificationsPage } from '../NotificationsPage';
import { CommunicationHistory } from '../communications/CommunicationHistory';

const conversation: CommunicationConversationListItem = {
  id: 'conversation-1',
  workspace_id: 'workspace-1',
  instance_id: 'channel-1',
  external_identity: 'alice',
  external_conversation_key: 'thread-1',
  sender_display_name: 'Alice',
  instance_display_name: 'Chat channel'
};
function message(index: number): CommunicationMessage {
  return {
    id: `message-${index}`,
    conversation_id: conversation.id,
    input_id: `input-${index}`,
    instance_id: 'channel-1',
    instance_display_name: 'Chat channel',
    external_identity: 'alice',
    sender_display_name: 'Alice',
    type: 'text',
    direction: index % 2 ? 'inbound' : 'outbound',
    occurred_at_ms: 1792960000000 + index * 1000,
    content: `Chat message ${index}`,
    file: null,
    processing_status: 'succeeded',
    send_status: 'failed',
    failure_code: 'send_interrupted',
    can_retry: index === 10,
    run_id: index === 10 ? 'original-run' : null,
    application_id: index === 10 ? 'original-app' : null
  };
}
let messages: CommunicationMessage[];
let client: QueryClient;
let calls: URL[];
const chatRequests = () =>
  calls.filter((url) => /\/conversations\/[^/]+\/messages$/.test(url.pathname));
beforeEach(async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=conversations'
  );
  await appI18n.changeLanguage('en_US');
  calls = [];
  messages = Array.from({ length: 10 }, (_, index) => message(index + 1));
  messages[7] = {
    ...messages[7]!,
    type: 'file',
    content: null,
    file: {
      file_name: 'answer.pdf',
      media_type: 'application/pdf',
      size_bytes: 100,
      status: 'ready',
      file_table_id: 'files',
      file_record_id: 'file-8',
      failure_code: null,
      access_path:
        '/api/console/communications/instances/channel-1/files/message-8'
    }
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
      else if (url.pathname.endsWith('/binding'))
        data = {
          binding: { model: 'deepseek-flash', reasoning_effort: 'high' }
        };
      else if (url.pathname.endsWith('/retry')) data = messages[9];
      else if (url.pathname.endsWith('/files/message-8'))
        data = {
          message: messages[7],
          file_name: 'answer.pdf',
          content_type: 'application/pdf',
          content_length: 100,
          offset: 0,
          next_offset: null,
          eof: true,
          data_base64: '',
          url: 'https://files.example.test/answer.pdf',
          preview_url: null
        };
      else if (/\/conversations\/[^/]+\/messages$/.test(url.pathname)) {
        data = url.searchParams.has('cursor')
          ? { items: messages.slice(0, 5), next_cursor: null }
          : { items: messages.slice(5), next_cursor: 'message-6' };
      } else if (url.pathname.endsWith('/conversations'))
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
      else if (url.pathname.endsWith('/messages')) {
        const { content, ...item } = message(page === 2 ? 3 : 1);
        const summary: CommunicationMessageListItem = {
          ...item,
          content_preview: content
        };
        data = { items: [summary], total: 21, page, page_size: 20 };
      } else if (url.pathname.endsWith('/logs'))
        data = { items: [], total: 0, page: 1, page_size: 20 };
      else throw new Error(`Unexpected request: ${url.pathname}`);
      return new Response(JSON.stringify({ data, meta: null }), {
        headers: { 'content-type': 'application/json' }
      });
    })
  );
});
afterEach(async () => {
  cleanup();
  await client.cancelQueries();
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
async function openChat() {
  await click(await screen.findByRole('button', { name: /^Chat$/ }));
  const dialog = await screen.findByRole('dialog');
  await within(dialog).findByText('Chat message 10');
  return dialog;
}

test('default shared table lists basic conversation fields across channels with a single Chat operation', async () => {
  await show();
  const alice = await screen.findByText('Alice');
  expect(alice.closest('.data-table')).not.toBeNull();
  expect(alice.closest('a')).toBeNull();
  const channel = screen.getByText('Chat channel');
  expect(channel.closest('a')).toBeNull();
  expect(screen.getAllByRole('columnheader')).toHaveLength(3);
  expect(screen.getAllByRole('button', { name: /^Chat$/ })).toHaveLength(1);
  expect(screen.queryByText('Delivery failed')).not.toBeInTheDocument();
  expect(
    screen.queryByRole('link', { name: 'View run logs' })
  ).not.toBeInTheDocument();
  expect(
    calls
      .find((url) => url.pathname.endsWith('/conversations'))!
      .searchParams.has('instance_id')
  ).toBe(false);
  expect(chatRequests()).toHaveLength(0);
  await click(screen.getByTitle('2'));
  await screen.findByText('Bob');
  expect(new URLSearchParams(window.location.search).get('page')).toBe('2');
});

test('Chat opens the modeless run-detail floating shell and reused conversation pane with only the newest five messages and prepends older messages explicitly', async () => {
  await show();
  const drawer = await openChat();
  const shell = screen.getByTestId('communication-chat-window');
  expect(shell).toHaveAttribute('aria-modal', 'false');
  expect(shell).toHaveClass('application-logs-floating-window');
  expect(
    shell.querySelector('.agent-flow-editor__dock-panel-header')
  ).not.toBeNull();
  expect(
    shell.querySelector('.application-run-detail__conversation-pane')
  ).not.toBeNull();
  expect(document.querySelector('.ant-drawer')).toBeNull();
  expect(within(shell).getAllByRole('separator')).toHaveLength(3);
  const pane = within(drawer).getByTestId('debug-conversation-messages');
  expect(within(drawer).getByText('Chat', { exact: true })).toBeInTheDocument();
  expect(within(drawer).getByRole('textbox')).toBeDisabled();
  expect(within(drawer).getByRole('textbox')).toHaveValue('');
  expect(await within(drawer).findByText('deepseek-flash')).toBeInTheDocument();
  expect(within(drawer).getByText('high')).toBeInTheDocument();
  expect(
    within(drawer).queryByRole('button', { name: /^Send$/ })
  ).not.toBeInTheDocument();
  expect(chatRequests()).toHaveLength(1);
  expect(chatRequests()[0]!.searchParams.toString()).toBe('limit=5');
  expect(within(pane).getByText('answer.pdf')).toBeInTheDocument();
  expect(calls.some((url) => url.pathname.includes('/files/'))).toBe(false);
  await click(
    within(drawer).getByRole('button', {
      name: appI18n.t('notifications:file_access')
    })
  );
  expect(
    await within(drawer).findByRole('link', {
      name: appI18n.t('notifications:download_file')
    })
  ).toHaveAttribute('href', 'https://files.example.test/answer.pdf');
  expect(
    calls.filter((url) =>
      url.pathname.endsWith('/instances/channel-1/files/message-8')
    )
  ).toHaveLength(1);
  expect(pane).toHaveTextContent(
    /Chat message 6[\s\S]*Chat message 7[\s\S]*answer.pdf[\s\S]*Chat message 9[\s\S]*Chat message 10/
  );
  expect(
    within(pane)
      .getByText('Chat message 6')
      .closest('.agent-flow-editor__debug-assistant-bubble')
  ).not.toBeNull();
  const inboundMessage = within(pane).getByText('Chat message 7');
  expect(
    inboundMessage.closest('.agent-flow-editor__debug-assistant-bubble')
  ).toBeNull();
  expect(
    within(drawer).queryByText('Chat message 1', { exact: true })
  ).not.toBeInTheDocument();
  await click(
    within(drawer).getByRole('button', { name: 'Load earlier messages' })
  );
  await within(drawer).findByText('Chat message 1', { exact: true });
  expect(chatRequests()).toHaveLength(2);
  expect(chatRequests()[1]!.searchParams.get('limit')).toBe('5');
  expect(chatRequests()[1]!.searchParams.get('cursor')).toBe('message-6');
  for (const index of [1, 2, 3, 4, 5, 6, 7, 9, 10])
    expect(
      within(pane).getAllByText(`Chat message ${index}`, { exact: true })
    ).toHaveLength(1);
  expect(pane).toHaveTextContent(
    /Chat message 1[\s\S]*Chat message 5[\s\S]*Chat message 6[\s\S]*Chat message 10/
  );
  expect(
    within(drawer).queryByRole('button', { name: 'Load earlier messages' })
  ).not.toBeInTheDocument();
  await click(within(drawer).getByRole('button', { name: 'Close chat' }));
  await waitFor(() =>
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  );
  const reopened = await openChat();
  expect(chatRequests()).toHaveLength(3);
  expect(chatRequests()[2]!.searchParams.get('limit')).toBe('5');
  expect(chatRequests()[2]!.searchParams.has('cursor')).toBe(false);
  expect(
    within(reopened).queryByText('Chat message 1', { exact: true })
  ).not.toBeInTheDocument();
  const reopenedPane = within(reopened).getByTestId(
    'debug-conversation-messages'
  );
  expect(within(reopenedPane).getAllByText(/^Chat message \d+$/)).toHaveLength(
    4
  );
  expect(within(reopenedPane).getByText('answer.pdf')).toBeInTheDocument();
  expect(
    within(reopened).getByRole('button', { name: 'Load earlier messages' })
  ).toBeInTheDocument();
});

test('message footer keeps only copy and Aim run-log actions', async () => {
  await show();
  const drawer = await openChat();
  const link = within(drawer).getByRole('link', { name: 'View run logs' });
  expect(link).toHaveAttribute(
    'href',
    '/applications/original-app/logs?run_id=original-run&view=trace'
  );
  expect(link.querySelector('[data-icon="aim"]')).not.toBeNull();
  const actions = link.closest('.agent-flow-editor__debug-message-actions')!;
  expect(actions.querySelectorAll('button,a')).toHaveLength(2);
  expect(actions).toHaveTextContent(/^$/);
  expect(
    within(drawer).queryByRole('button', { name: 'Retry delivery' })
  ).not.toBeInTheDocument();
  expect(document.querySelector('.communication-chat__direction')).toBeNull();
});

test('conversation deep link restores tabs and Chat after remount and browser back/forward; close clears selection', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=logs&record_tab=received&conversation_id=conversation-1'
  );
  const view = await show(<NotificationsPage />);
  await screen.findByText('Chat message 10');
  expect(
    screen.getByRole('tab', {
      name: appI18n.t('notifications:send_receive_records')
    })
  ).toHaveAttribute('aria-selected', 'true');
  expect(
    screen.getByRole('tab', {
      name: appI18n.t('notifications:received_records')
    })
  ).toHaveAttribute('aria-selected', 'true');
  view.unmount();
  await show(<NotificationsPage />);
  await screen.findByText('Chat message 10');
  await click(screen.getByRole('button', { name: 'Close chat' }));
  expect(
    new URLSearchParams(window.location.search).has('conversation_id')
  ).toBe(false);
  await click(
    screen.getByRole('tab', { name: appI18n.t('notifications:logs') })
  );
  expect(new URLSearchParams(window.location.search).get('record_tab')).toBe(
    'sent'
  );
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
    window.history.back();
  });
  await screen.findByText('Chat message 10');
  expect(
    new URLSearchParams(window.location.search).get('conversation_id')
  ).toBe('conversation-1');
  await act(async () => {
    window.history.forward();
  });
  await waitFor(() =>
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  );
  expect(
    new URLSearchParams(window.location.search).has('conversation_id')
  ).toBe(false);
});

test('received page two Chat preserves tabs, filters and pagination without opening a message detail', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/communications?tab=logs&record_tab=received&page=2&processing_status=succeeded&send_status=failed'
  );
  await show(<NotificationsPage />);
  expect(
    (await screen.findByText('Chat message 3')).closest('.data-table')
  ).not.toBeNull();
  await openChat();
  const params = new URLSearchParams(window.location.search);
  expect(params.get('tab')).toBe('logs');
  expect(params.get('record_tab')).toBe('received');
  expect(params.get('page')).toBe('2');
  expect(params.get('conversation_id')).toBe('conversation-1');
  expect(params.has('message_id')).toBe(false);
  expect(params.has('conversation_page')).toBe(false);
  expect(screen.getAllByRole('dialog')).toHaveLength(1);
  const listRequest = calls.find(
    (url) => url.pathname === '/api/console/communications/messages'
  )!;
  expect(listRequest.searchParams.get('direction')).toBe('inbound');
  expect(listRequest.searchParams.get('processing_status')).toBe('succeeded');
  expect(listRequest.searchParams.get('send_status')).toBe('failed');
  expect(listRequest.searchParams.has('instance_id')).toBe(false);
  expect(
    calls.some((url) => /\/messages\/message-\d+$/.test(url.pathname))
  ).toBe(false);
});
