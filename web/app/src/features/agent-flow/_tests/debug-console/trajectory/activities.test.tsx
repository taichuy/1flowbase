import './navigation';
import { App } from 'antd';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import { WorkflowActivityWorkspace } from '../../../components/debug-console/trajectory/activities/WorkflowActivityWorkspace';
import { LazyTraceNodeItem } from '../../../components/debug-console/ConversationLogPanel';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceNodeSummary
} from '../../../components/debug-console/conversation-log-trace-model';
import { appI18n } from '../../../../../shared/i18n/app-i18n';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
function node(
  id: string,
  kind: string,
  children = false
): ConversationLogTraceNodeSummary {
  return {
    trace_node_id: id,
    node_kind: kind,
    node_type: kind === 'node_run' ? 'llm' : kind,
    node_run_id: kind === 'node_run' ? id : undefined,
    node_alias: id,
    status: 'succeeded',
    started_at: '2026-09-22T01:00:00Z',
    has_children: children,
    has_content: false
  };
}
function fixture() {
  const llm = node('llm-1', 'node_run', true);
  const tool = node('Tools', 'tool_group', true);
  const rounds = node('Rounds', 'round_group', true);
  const agents = node('Agents', 'agent_group', true);
  const branch = {
    ...node('child-llm', 'node_run'),
    has_content: true,
    source_flow_run_id: 'child-run',
    source_trace_node_id: 'source-child-node'
  };
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn().mockResolvedValue({ nodes: [llm] }),
    loadChildren: vi.fn(async (_run, parent, cursor) => ({
      items:
        parent === 'llm-1'
          ? cursor
            ? [rounds, agents]
            : [tool]
          : parent === 'Agents'
            ? [branch]
            : parent === 'Tools'
              ? [node('echo', 'tool_callback')]
              : parent === 'Rounds'
                ? [node('Compact', 'task_round')]
                : [],
      page_info: {
        has_more: parent === 'llm-1' && !cursor,
        next_cursor: parent === 'llm-1' && !cursor ? 'page-2' : null,
        page_size: 2
      }
    })),
    loadContent: vi.fn().mockResolvedValue({
      trace_node_id: 'child-llm',
      node_kind: 'node_run',
      payload: {}
    }),
    loadRunTrajectory: vi.fn()
  };
  return { loader, llm };
}
function surface(content: React.ReactNode) {
  return (
    <App>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        {content}
      </QueryClientProvider>
    </App>
  );
}

test.each([
  ['tools', 'Tools'],
  ['rounds', 'Rounds']
] as const)(
  'moves %s into a paged activity tree without reading bodies',
  async (category, label) => {
    const { loader } = fixture();
    render(
      surface(
        <WorkflowActivityWorkspace
          runId="run-1"
          loader={loader}
          active
          category={category}
        />
      )
    );
    const ledger = screen.getByLabelText('工作流内部活动');
    await within(ledger).findByRole('button', { name: label });
    await waitFor(() =>
      expect(loader.loadChildren).toHaveBeenCalledWith(
        'run-1',
        'llm-1',
        'page-2'
      )
    );
    expect(loader.loadContent).not.toHaveBeenCalled();
    expect(loader.loadChildren).not.toHaveBeenCalledWith(
      'run-1',
      label,
      undefined
    );
    fireEvent.click(within(ledger).getByRole('button', { name: label }));
    await waitFor(() =>
      expect(loader.loadChildren).toHaveBeenCalledWith(
        'run-1',
        label,
        undefined
      )
    );
  }
);
test.each([true, false])(
  'workflow keeps its LLM and migrates groups only when trajectory is available: %s',
  async (enabled) => {
    const { loader, llm } = fixture();
    if (!enabled) delete loader.loadRunTrajectory;
    render(
      surface(
        <LazyTraceNodeItem
          initiallyExpanded
          defaultToolsExpanded={false}
          node={llm}
          runId="run-1"
          traceLoader={loader}
        />
      )
    );
    await waitFor(() =>
      expect(loader.loadChildren).toHaveBeenCalledWith(
        'run-1',
        'llm-1',
        undefined
      )
    );
    expect(screen.getByText('llm-1')).toBeInTheDocument();
    if (enabled) expect(screen.queryByText('Tools')).not.toBeInTheDocument();
    else await screen.findByText('Tools');
  }
);

test.each(['failed', 'partial', 'stale'] as const)(
  'does not report empty activities for a %s projection',
  async (status) => {
    const { loader } = fixture();
    vi.mocked(loader.loadTree).mockResolvedValue({
      nodes: [],
      projection_status: {
        projection_status: status,
        projection_version: 15,
        source_watermark: 'fixture',
        attempt_count: 1,
        retriable: true
      }
    });
    render(
      surface(
        <WorkflowActivityWorkspace
          runId="run-1"
          loader={loader}
          active
          category="tools"
        />
      )
    );
    await screen.findByRole('alert');
    expect(screen.queryByText('没有这类活动记录')).not.toBeInTheDocument();
    expect(document.querySelector('.ant-empty')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    await waitFor(() => expect(loader.loadTree).toHaveBeenCalledTimes(2));
  }
);
