import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { MessageStatus } from '../communications/CommunicationHistory';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import type { CommunicationMessage } from '@1flowbase/api-client';
afterEach(async () => {
  cleanup();
  await appI18n.changeLanguage('zh_Hans');
});
const message: CommunicationMessage = {
  id: 'message',
  conversation_id: 'conversation',
  external_message_key: 'external',
  received_at_ms: 1000,
  content: 'User request',
  processing_status: 'succeeded',
  run_id: 'run',
  final_reply: 'Assistant response',
  send_status: 'failed',
  failure_code: 'send_interrupted',
  can_retry: true
};
test('delivery failure remains visible after successful processing and translates failure reason', async () => {
  await appI18n.changeLanguage('en_US');
  render(<MessageStatus message={message} />);
  expect(screen.getByText('Processed')).toBeInTheDocument();
  expect(screen.getByText('Delivery failed')).toBeInTheDocument();
  expect(
    screen.getByText('Failure reason: Delivery was interrupted')
  ).toBeInTheDocument();
  expect(screen.queryByText('send_interrupted')).not.toBeInTheDocument();
});
test('unknown failure does not expose provider diagnostic codes', async () => {
  await appI18n.changeLanguage('en_US');
  render(
    <MessageStatus
      message={{ ...message, failure_code: 'private_provider_code' }}
    />
  );
  expect(
    screen.getByText('Failure reason: Failure reason unavailable')
  ).toBeInTheDocument();
  expect(screen.queryByText(/private_provider_code/)).not.toBeInTheDocument();
});

test('conversation shows both directions and paginates history; retry follows authoritative eligibility', async () => {
  const { fireEvent, waitFor, act } = await import('@testing-library/react');
  const { vi } = await import('vitest');
  const { QueryClient, QueryClientProvider } =
    await import('@tanstack/react-query');
  const { CommunicationHistory } =
    await import('../communications/CommunicationHistory');
  const { useAuthStore, resetAuthStore } =
    await import('../../../../../state/auth-store');
  await appI18n.changeLanguage('en_US');
  const requests: string[] = [];
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  useAuthStore.setState({ csrfToken: 'csrf' });
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string) => {
      requests.push(input);
      const url = new URL(input, 'http://localhost');
      const data = url.pathname.endsWith('/instances')
        ? [{ id: 'channel', display_name: 'Chat channel' }]
        : url.pathname.endsWith('/conversations')
          ? {
              items: [
                {
                  id: 'conversation',
                  sender_display_name: 'Alice',
                  external_identity: 'alice',
                  external_conversation_key: 'thread'
                }
              ],
              next_cursor: null
            }
          : url.pathname.endsWith('/files')
            ? { items: [], next_cursor: null }
            : {
                items: url.searchParams.has('cursor')
                  ? [
                      {
                        ...message,
                        id: 'older',
                        content: 'Earlier request',
                        final_reply: null,
                        can_retry: false
                      }
                    ]
                  : [{ ...message, can_retry: true }],
                next_cursor: url.searchParams.has('cursor') ? null : 'next-page'
              };
      return new Response(JSON.stringify({ data, meta: null }));
    })
  );
  try {
    render(
      <QueryClientProvider client={client}>
        <CommunicationHistory />
      </QueryClientProvider>
    );
    await waitFor(() => expect(client.isFetching()).toBe(0));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Channel name' }));
    fireEvent.click(
      await screen.findByText('Chat channel', {
        selector: '.ant-select-item-option-content'
      })
    );
    await screen.findByText('Alice');
    fireEvent.click(screen.getByRole('button', { name: 'Details' }));
    await screen.findByText('User request');
    expect(screen.getByText('Assistant response')).toBeInTheDocument();
    expect(
      screen.getAllByRole('button', { name: 'Retry delivery' })
    ).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Load more' }));
    await screen.findByText('Earlier request');
    expect(
      screen.getAllByRole('button', { name: 'Retry delivery' })
    ).toHaveLength(1);
    expect(
      requests.some((url) =>
        url.includes('/messages?cursor=next-page&limit=20')
      )
    ).toBe(true);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Retry delivery' }));
    });
    await waitFor(() =>
      expect(
        requests.some((url) => url.includes('/messages/message/retry'))
      ).toBe(true)
    );
  } finally {
    cleanup();
    client.clear();
    resetAuthStore();
    vi.unstubAllGlobals();
  }
});
