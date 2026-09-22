import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { ConversationLogPanel } from '../../components/debug-console/ConversationLogPanel';
import { appI18n } from '../../../../shared/i18n/app-i18n';

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
}

function renderWithQueryClient(children: ReactNode) {
  const queryClient = createQueryClient();

  return render(
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('debug conversation log pagination', () => {
  beforeEach(async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
  });

  test.each(['tool_group', 'llm'])(
    'keeps %s trace children and pagination in the workflow tree',
    async (kind) => {
      const rootNode = {
        trace_node_id: 'node_run:node-run-tools',
        node_kind: kind === 'llm' ? 'node_run' : 'tool_group',
        node_run_id: kind === 'llm' ? 'node-run-tools' : null,
        node_id: null,
        node_type: kind === 'llm' ? 'llm' : 'tools',
        node_alias: '工具',
        status: 'succeeded',
        started_at: '2026-04-25T10:00:01Z',
        finished_at: '2026-04-25T10:00:05Z',
        duration_ms: null,
        metrics_payload: {},
        has_children: true,
        child_count: 3,
        has_content: false
      };
      const childNode = (index: number) => ({
        trace_node_id: `tool-callback:${index}`,
        node_kind: 'tool_callback',
        node_run_id: null,
        node_id: null,
        node_type: 'tool',
        node_alias: `tool_${index}`,
        status: 'succeeded',
        started_at: '2026-04-25T10:00:02Z',
        finished_at: '2026-04-25T10:00:03Z',
        duration_ms: 1000,
        metrics_payload: {},
        has_children: false,
        child_count: 0,
        has_content: false
      });
      const traceLoader = {
        loadTree: vi.fn().mockResolvedValue({ nodes: [rootNode] }),
        loadChildren: vi
          .fn()
          .mockResolvedValueOnce({
            items: [childNode(1), childNode(2)],
            page_info: {
              has_more: true,
              next_cursor: 'cursor-page-2',
              page_size: 2
            }
          })
          .mockResolvedValueOnce({
            items: [childNode(3)],
            page_info: {
              has_more: false,
              next_cursor: null,
              page_size: 2
            }
          }),
        loadContent: vi.fn()
      };

      renderWithQueryClient(
        <ConversationLogPanel
          message={{
            id: 'conversation-assistant-run-paged-tree',
            role: 'assistant',
            content: '分页 trace',
            status: 'completed',
            runId: 'run-paged-tree',
            detailRunId: 'run-paged-tree',
            rawOutput: null,
            traceSummary: []
          }}
          traceLoader={traceLoader}
          onClose={vi.fn()}
        />
      );

      fireEvent.click(screen.getByRole('tab', { name: '追踪' }));
      const toolsNode = await screen.findByRole('button', { name: /工具/ });
      fireEvent.click(toolsNode);

      expect(
        await screen.findByRole('button', { name: /tool_1/ })
      ).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: /tool_2/ })
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('button', { name: /tool_3/ })
      ).not.toBeInTheDocument();

      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: '加载更多' }));

      expect(
        await screen.findByRole('button', { name: /tool_3/ })
      ).toBeInTheDocument();
      expect(traceLoader.loadChildren).toHaveBeenNthCalledWith(
        1,
        'run-paged-tree',
        'node_run:node-run-tools',
        undefined
      );
      expect(traceLoader.loadChildren).toHaveBeenNthCalledWith(
        2,
        'run-paged-tree',
        'node_run:node-run-tools',
        'cursor-page-2'
      );
      expect(
        screen.queryByRole('button', { name: '加载更多' })
      ).not.toBeInTheDocument();
    },
    10000
  );
});
