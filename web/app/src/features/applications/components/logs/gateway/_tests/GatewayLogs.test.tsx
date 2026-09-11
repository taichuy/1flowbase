import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { GatewayLogs } from '../GatewayLogs';

const fetchPage = vi.hoisted(() => vi.fn());
vi.mock('../../../../api/gateway-logs', () => ({ fetchGatewayLogs: fetchPage }));
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

// AC-001/002/011: navigation consumes server parent identities, never prompt similarity.
describe('gateway log hierarchy', () => {
  it('drills down exact server identities and retains distinct same-text turns', async () => {
    const entry = (id: string, kind: string, title: string) => ({
      id, kind, title, identity_status: 'identified', completion_status: 'unknown',
      observations: [], metrics: { invocation_count: 4, attempt_count: 4, costs: [], total_tokens: null }, messages: []
    });
    fetchPage.mockImplementation(async (_app: string, query: { conversation_id?: string }) => ({
      items: query.conversation_id ? [entry('turn-a', 'turn', 'same question'), entry('turn-b', 'turn', 'same question')] : [entry('conversation-a', 'conversation', 'thread-a')],
      page: 1, page_size: 20, total: query.conversation_id ? 2 : 1
    }));
    render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><GatewayLogs applicationId="app-a" /></QueryClientProvider>);
    fireEvent.click(await screen.findByRole('button', { name: 'thread-a' }));
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'same question' })).toHaveLength(2));
    expect(fetchPage).toHaveBeenLastCalledWith('app-a', expect.objectContaining({ conversation_id: 'conversation-a' }));
    expect(screen.getAllByText('gateway.unknown').length).toBeGreaterThan(0);
  });
});
