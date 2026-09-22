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
import type { ProviderTrajectoryStep } from '@1flowbase/api-client';
import { WorkflowActivityWorkspace } from '../../../components/debug-console/trajectory/activities/WorkflowActivityWorkspace';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceNodeSummary
} from '../../../components/debug-console/conversation-log-trace-model';
import { appI18n } from '../../../../../shared/i18n/app-i18n';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
function node(id: string, kind: string): ConversationLogTraceNodeSummary {
  return {
    trace_node_id: id,
    node_kind: kind,
    node_type: kind,
    node_alias: 'thread_spawn · PROMPT MUST NOT BECOME A HEADING',
    status: 'waiting_callback',
    started_at: '2026-09-22T01:00:00Z',
    has_children: true,
    has_content: false
  };
}
function event(
  run: string,
  sequence: number,
  invocation: string,
  kind: ProviderTrajectoryStep['metadata']['kind']
): ProviderTrajectoryStep {
  return {
    event_id: `${run}-${sequence}`,
    event_sequence: sequence,
    event_type: 'provider_semantic_step',
    created_at: '2026-09-22T01:00:00Z',
    links: [
      { relation: 'trigger', flow_run_id: run, request_id: `${run}-request` }
    ],
    metadata: {
      purpose: 'generate',
      source: 'ai_native',
      kind,
      status: 'recorded',
      step_key: `${sequence}`,
      flow_run_id: run,
      node_id: 'gpt-5.6-luna',
      node_run_id: `${run}-llm`,
      invocation_id: invocation,
      provider_attempt_index: 1
    }
  };
}
function fixture() {
  const group = node('Agents', 'agent_group');
  const child = {
    ...node('child-projection', 'child_task'),
    source_flow_run_id: 'child-run',
    node_mode: 'thread_spawn'
  };
  const sibling = {
    ...node('sibling-projection', 'node_run'),
    source_flow_run_id: 'sibling-run',
    trace_relation_kind: 'subagent',
    has_content: true
  };
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn().mockResolvedValue({ nodes: [group] }),
    loadChildren: vi.fn(async (_run, parent, cursor) => ({
      items: parent === 'Agents' ? (cursor ? [sibling] : [child]) : [],
      page_info: {
        has_more: parent === 'Agents' && !cursor,
        next_cursor: parent === 'Agents' && !cursor ? 'next' : null,
        page_size: 1
      }
    })),
    loadContent: vi.fn().mockResolvedValue({
      trace_node_id: 'sibling-projection',
      node_kind: 'node_run',
      payload: {}
    }),
    loadRunTrajectory: vi.fn<
      NonNullable<ConversationLogTraceLoader['loadRunTrajectory']>
    >(async (run, cursor) => ({
      items: cursor
        ? [
            event(run, 3, 'second', 'model_call'),
            event(run, 4, 'second', 'model_reply')
          ]
        : [
            event(run, 1, 'first', 'model_call'),
            event(run, 2, 'first', 'model_reply')
          ],
      next_cursor: cursor ? null : 2,
      integrity: 'complete',
      observation_count: 4,
      persist_failed_count: 0,
      protocol_integrity: 'not_recorded',
      protocol_persist_failed_count: 0
    })),
    loadTrajectoryBody: vi.fn<
      NonNullable<ConversationLogTraceLoader['loadTrajectoryBody']>
    >(async (run, _node, id) => ({
      source: 'ai_native',
      evidence_scope: 'step',
      event_id: id,
      sections: [{ kind: 'output', value: `body:${run}:${id}` }],
      items: [],
      next_cursor: null
    }))
  };
  const onClient = vi.fn();
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const view = (active = true) => (
    <App>
      <QueryClientProvider client={client}>
        <WorkflowActivityWorkspace
          runId="parent-run"
          loader={loader}
          active={active}
          category="agents"
          onClient={onClient}
        />
      </QueryClientProvider>
    </App>
  );
  return { loader, onClient, view };
}

test('automatically lists each subagent invocation, keeps source runs isolated, and loads only selected evidence', async () => {
  const { loader, onClient, view } = fixture();
  render(view());
  const child = await screen.findByRole('region', { name: 'child-run' });
  const sibling = await screen.findByRole('region', { name: 'sibling-run' });
  await within(child).findByRole('button', { name: /请求 #2/ });
  await within(sibling).findByRole('button', { name: /请求 #2/ });
  expect(within(child).getAllByRole('button')).toHaveLength(2);
  expect(screen.queryByText(/PROMPT MUST/)).not.toBeInTheDocument();
  expect(loader.loadTrajectoryBody).not.toHaveBeenCalled();
  expect(loader.loadChildren).not.toHaveBeenCalledWith(
    'parent-run',
    'child-projection',
    undefined
  );
  fireEvent.click(within(child).getByRole('button', { name: /请求 #2/ }));
  await screen.findByText('body:child-run:child-run-3');
  expect(loader.loadTrajectoryBody).toHaveBeenCalledWith(
    'child-run',
    'child-run-llm',
    'child-run-3',
    undefined,
    'semantic'
  );
  fireEvent.click(screen.getByRole('tab', { name: /模型回复.*#4/ }));
  await screen.findByText('body:child-run:child-run-4');
  fireEvent.click(
    screen.getByRole('button', { name: /来源请求.*child-run-request/ })
  );
  expect(onClient).toHaveBeenCalledWith({
    relation: 'trigger',
    flow_run_id: 'child-run',
    request_id: 'child-run-request'
  });
  fireEvent.click(within(sibling).getByRole('button', { name: /请求 #1/ }));
  await screen.findByText('body:sibling-run:sibling-run-1');
  expect(
    screen.queryByText('body:child-run:child-run-4')
  ).not.toBeInTheDocument();
  expect(loader.loadContent).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: /关联工作流节点/ }));
  await waitFor(() =>
    expect(loader.loadContent).toHaveBeenCalledWith(
      'parent-run',
      'sibling-projection'
    )
  );
  expect(
    screen.queryByRole('button', { name: '调用轨迹' })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /关闭/ }));
  expect(screen.queryByRole('complementary')).not.toBeInTheDocument();
});

test('inactive view does not start discovery or request loading', () => {
  const { loader, view } = fixture();
  render(view(false));
  expect(loader.loadTree).not.toHaveBeenCalled();
  expect(loader.loadRunTrajectory).not.toHaveBeenCalled();
});

test('failed request loading is visible and retryable instead of rendered as an empty agent', async () => {
  const { loader, view } = fixture();
  vi.mocked(loader.loadRunTrajectory!).mockRejectedValueOnce(
    new Error('unavailable')
  );
  render(view());
  const child = await screen.findByRole('region', { name: 'child-run' });
  await within(child).findByRole('alert');
  fireEvent.click(within(child).getByRole('button', { name: /重\s*试/ }));
  await within(child).findByRole('button', { name: /请求 #2/ });
});

test('missing source identity never falls back to parent requests', async () => {
  const { loader, view } = fixture();
  vi.mocked(loader.loadChildren).mockResolvedValue({
    items: [node('unlinked-child', 'child_task')],
    page_info: { has_more: false, next_cursor: null, page_size: 1 }
  });
  render(view());
  const child = await screen.findByRole('region', { name: 'unlinked-child' });
  expect(
    within(child).queryByRole('button', { name: /请求 #/ })
  ).not.toBeInTheDocument();
  expect(loader.loadRunTrajectory).not.toHaveBeenCalled();
  expect(loader.loadTrajectoryBody).not.toHaveBeenCalled();
});
