import './navigation';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
  ClientTrajectoryStep,
  ClientTrajectoryOptions,
  ConversationClientTrajectoryStep,
  ConversationTrajectoryQuery,
  ConversationClientTrajectoryQueryPage
} from '@1flowbase/api-client';
import { ProviderTrajectory } from '../../../components/debug-console/trajectory/ProviderTrajectory';
import type { ConversationLogTraceLoader } from '../../../components/debug-console/conversation-log-trace-model';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
const root: ClientTrajectoryStep = {
  id: 'request-1',
  request_id: 'request-1',
  sequence: 1,
  created_at: '2026-09-22T00:00:00Z',
  category: 'request',
  name: 'Responses request',
  namespace: null,
  preview: 'gpt-5.6-luna · max',
  parameters_preview: null,
  result_preview: null,
  status: 'submitted',
  origin: 'submitted',
  protocol: 'responses',
  transport: 'http',
  flow_run_id: 'run-1',
  node_run_id: null,
  parent_id: null,
  call_id: null,
  item_id: null,
  response_id: null,
  turn_id: null,
  related_step_id: null,
  available_sections: ['overview', 'timing', 'raw']
};
const tool: ClientTrajectoryStep = {
  ...root,
  id: 'call-1',
  sequence: 2,
  parent_id: root.id,
  category: 'tool_call',
  name: 'exec_command',
  preview: 'pwd',
  parameters_preview: '{"cmd":"pwd"}',
  call_id: 'call-original',
  origin: 'emitted',
  available_sections: ['overview', 'parameters', 'schema', 'timing', 'raw']
};
function indexOverview(
  items: (ClientTrajectoryStep | ConversationClientTrajectoryStep)[]
) {
  const category_counts: Record<string, number> = {};
  for (const step of items)
    category_counts[step.category] = (category_counts[step.category] ?? 0) + 1;
  return {
    total_count: items.length,
    category_counts,
    time_start: items[0]?.created_at ?? null,
    time_end: items.at(-1)?.created_at ?? null,
    items: items.map((step) => ({
      id: step.id,
      request_id: step.request_id,
      sequence: step.sequence,
      created_at: step.created_at,
      category: step.category,
      protocol: step.protocol,
      flow_run_id: step.flow_run_id,
      node_run_id: step.node_run_id,
      ...('record_id' in step
        ? {
            record_id: step.record_id,
            log_conversation_id: step.log_conversation_id,
            is_subagent: step.is_subagent
          }
        : {})
    }))
  };
}
function fixture(namespace: string | null = null, protocol = 'responses') {
  const all = [
    { ...root, protocol },
    { ...tool, namespace, protocol },
    {
      ...root,
      id: 'result-1',
      protocol,
      sequence: 3,
      category: 'tool_result',
      name: 'exec_command',
      result_preview: '/work',
      related_step_id: tool.id,
      call_id: tool.call_id,
      available_sections: ['result']
    }
  ];
  const loadClientTrajectory = vi
    .fn()
    .mockImplementation(
      (_run, _node, _cursor, options?: ClientTrajectoryOptions) => {
        const matching = all.filter(
          (step) =>
            (!options?.category || step.category === options.category) &&
            (!options?.keyword ||
              [
                step.name,
                step.namespace,
                step.preview,
                step.call_id,
                step.parameters_preview,
                step.result_preview
              ]
                .join(' ')
                .toLowerCase()
                .includes(options.keyword.toLowerCase()))
        );
        return Promise.resolve({
          items: options?.index_only ? [] : all,
          ...(options?.index_only ? { overview: indexOverview(matching) } : {}),
          next_cursor: null,
          integrity: 'complete'
        });
      }
    );
  const loadClientTrajectorySection = vi
    .fn()
    .mockImplementation((_run, stepId, section) =>
      Promise.resolve({
        step_id: stepId,
        request_id: root.id,
        evidence_scope: section === 'raw' ? 'capture' : 'step',
        section,
        items: [
          {
            sequence: 1,
            value:
              section === 'parameters'
                ? '  {"cmd":"pwd"}  '
                : section === 'raw'
                  ? {
                      direction: 'submitted',
                      encoding: 'utf8',
                      body: '  { "input": "客户端原文" }\n',
                      frame_kind: 'request'
                    }
                  : { model: 'gpt-5.6-luna' }
          }
        ],
        next_cursor: null
      })
    );
  const loadWorkflowTrajectory = vi
    .fn()
    .mockResolvedValue({ items: [], next_cursor: null });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadClientTrajectory,
    loadClientTrajectorySection,
    loadWorkflowTrajectory
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ProviderTrajectory runId="run-1" loader={loader} />
    </QueryClientProvider>
  );
  return {
    loadClientTrajectory,
    loadClientTrajectorySection,
    loadWorkflowTrajectory
  };
}
test('defaults to original client classification and lazily loads only selected sections', async () => {
  const {
    loadClientTrajectory,
    loadClientTrajectorySection,
    loadWorkflowTrajectory
  } = fixture();
  expect(loadClientTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  const call = await screen.findByRole('button', {
    name: '工具调用 · exec_command'
  });
  expect(loadWorkflowTrajectory).toHaveBeenCalled();
  expect(loadClientTrajectorySection).not.toHaveBeenCalled();
  fireEvent.click(call);
  await waitFor(() =>
    expect(loadClientTrajectorySection).toHaveBeenCalledWith(
      'run-1',
      'call-1',
      'overview',
      undefined,
      undefined
    )
  );
  const inspector = screen.getByRole('complementary', { name: '步骤检查器' });
  expect(inspector.parentElement).toBe(
    call.closest('.provider-trajectory__split')
  );
  fireEvent.click(within(inspector).getByRole('tab', { name: '参数' }));
  const parameters = await within(inspector).findByText('{"cmd":"pwd"}', {
    selector: 'pre'
  });
  // eslint-disable-next-line jest-dom/prefer-to-have-text-content -- Exact raw whitespace is the protocol contract; substring matching is insufficient.
  expect(parameters.textContent).toBe('  {"cmd":"pwd"}  ');
  expect(
    loadClientTrajectorySection.mock.calls.some((args) => args[2] === 'raw')
  ).toBe(false);
  fireEvent.click(within(inspector).getByRole('tab', { name: '原始协议' }));
  await waitFor(() =>
    expect(loadClientTrajectorySection).toHaveBeenCalledWith(
      'run-1',
      'call-1',
      'raw',
      undefined,
      undefined
    )
  );
  // eslint-disable-next-line jest-dom/prefer-to-have-text-content -- Preserve exact raw protocol bytes, including the trailing newline.
  expect((await screen.findByText(/客户端原文/)).textContent).toBe(
    '  { "input": "客户端原文" }\n'
  );
  expect(within(inspector).getByText('utf8')).toBeInTheDocument();
  expect(within(inspector).getByText('request')).toBeInTheDocument();
  fireEvent.click(call);
  expect(screen.getByRole('complementary')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '关闭详情' }));
  expect(screen.queryByRole('complementary')).not.toBeInTheDocument();
});
test('loads viewport rows, keeps original categories and follows actual call relation', async () => {
  const { loadClientTrajectory } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await screen.findByRole('button', { name: '工具调用 · exec_command' });
  const result = await screen.findByRole('button', {
    name: '工具结果 · exec_command'
  });
  expect(loadClientTrajectory).toHaveBeenCalledWith(
    'run-1',
    undefined,
    undefined,
    expect.objectContaining({ index_only: true })
  );
  expect(
    loadClientTrajectory.mock.calls.every((call) => call[2] === undefined)
  ).toBe(true);
  fireEvent.click(result);
  fireEvent.click(await screen.findByRole('button', { name: '定位关联调用' }));
  expect(
    screen.getByRole('button', { name: '工具调用 · exec_command' })
  ).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(screen.getByRole('button', { name: '收起请求' }));
  expect(
    screen.queryByRole('button', { name: '工具调用 · exec_command' })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '展开请求' }));
  expect(
    screen.getByRole('button', { name: '工具调用 · exec_command' })
  ).toBeInTheDocument();
});

test('shows and searches actual protocol namespace without replacing the original tool name', async () => {
  const { loadClientTrajectory } = fixture('mcp__codex_apps__github');
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await screen.findByRole('button', {
    name: '工具调用 · mcp__codex_apps__github.exec_command'
  });
  fireEvent.change(screen.getByRole('textbox', { name: '搜索已加载步骤' }), {
    target: { value: 'mcp__codex_apps__github' }
  });
  const filteredRow = await screen.findByRole('button', {
    name: '工具调用 · mcp__codex_apps__github.exec_command'
  });
  expect(loadClientTrajectory).toHaveBeenCalledWith(
    'run-1',
    undefined,
    undefined,
    expect.objectContaining({
      index_only: true,
      keyword: 'mcp__codex_apps__github'
    })
  );
  fireEvent.click(filteredRow);
  const inspector = screen.getByRole('complementary', { name: '步骤检查器' });
  expect(within(inspector).getByText('namespace')).toBeInTheDocument();
  expect(
    within(inspector).getByText('mcp__codex_apps__github')
  ).toBeInTheDocument();
});

test('Chat toolbar uses backend protocol classification', async () => {
  fixture(null, 'chat_completions');
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  expect(await screen.findByText('Chat Completions')).toBeInTheDocument();
  expect(
    screen.queryByText('Responses', {
      selector: '.client-trajectory__protocol'
    })
  ).not.toBeInTheDocument();
});

test('imported records reuse total trajectory and lazy sections without a native run or internal calls', async () => {
  const canonicalPage: ConversationClientTrajectoryQueryPage = {
    log_conversation_id: 'canonical-1',
    items: [
      {
        ...tool,
        record_id: 'record-1',
        log_conversation_id: 'canonical-1',
        parent_task_run_id: null,
        is_subagent: false,
        source_kind: 'imported',
        source_id: 'source-1',
        source_client: 'codex',
        source_session_id: 'session-1',
        source_task_id: 'task-1',
        flow_run_id: null,
        transport: 'file',
        preview: 'Intermediate reasoning',
        available_sections: ['parameters', 'raw']
      }
    ],
    matches: [],
    previous_cursor: null,
    next_cursor: null,
    search_sections: ['overview', 'parameters', 'result'],
    integrity: 'complete'
  };
  const queryConversationClientTrajectory = vi
    .fn()
    .mockImplementation((_canonical, query: ConversationTrajectoryQuery) =>
      Promise.resolve(
        query.index_only
          ? {
              ...canonicalPage,
              items: [],
              overview: indexOverview(canonicalPage.items)
            }
          : canonicalPage
      )
    );
  const loadConversationClientTrajectorySection = vi
    .fn()
    .mockImplementation((_canonical, stepId, section) =>
      Promise.resolve({
        request_id: 'session-1',
        evidence_scope: 'step',
        step_id: stepId,
        section,
        items: [
          {
            sequence: 2,
            value:
              section === 'parameters'
                ? 'tool input from source'
                : { original: 'source event' }
          }
        ],
        next_cursor: null
      })
    );
  const loadRecordClientTrajectory = vi.fn();
  const loader: ConversationLogTraceLoader = {
    sourceKind: 'imported',
    queryConversationClientTrajectory,
    loadConversationClientTrajectorySection,
    loadRecordClientTrajectory
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ProviderTrajectory
        runId="record-1"
        logConversationId="canonical-1"
        loader={loader}
      />
    </QueryClientProvider>
  );
  expect(queryConversationClientTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  const call = await screen.findByRole('button', {
    name: '工具调用 · exec_command'
  });
  expect(queryConversationClientTrajectory).toHaveBeenCalledWith(
    'canonical-1',
    expect.objectContaining({ focus_record_id: 'record-1', index_only: true })
  );
  expect(loadRecordClientTrajectory).not.toHaveBeenCalled();
  expect(loadConversationClientTrajectorySection).not.toHaveBeenCalled();
  expect(screen.queryByText('内部调用')).not.toBeInTheDocument();
  fireEvent.click(call);
  expect(await screen.findByText('tool input from source')).toBeInTheDocument();
  await waitFor(() =>
    expect(loadConversationClientTrajectorySection).toHaveBeenCalledWith(
      'canonical-1',
      'call-1',
      'parameters',
      undefined
    )
  );
  expect(screen.getByText('responses · file')).toBeInTheDocument();
  expect(
    loadConversationClientTrajectorySection.mock.calls.some(
      (args) => args[2] === 'raw'
    )
  ).toBe(false);
  fireEvent.click(screen.getByRole('tab', { name: '原始协议' }));
  await waitFor(() =>
    expect(loadConversationClientTrajectorySection).toHaveBeenCalledWith(
      'canonical-1',
      'call-1',
      'raw',
      undefined
    )
  );
  expect(
    screen.queryByRole('button', { name: '打开内部调用' })
  ).not.toBeInTheDocument();
  const requestsBeforeLocale = queryConversationClientTrajectory.mock.calls.length;
  await act(async () => { await appI18n.changeLanguage('en_US'); });
  expect(await screen.findByRole('button', { name: 'Tool call · exec_command' })).toBeInTheDocument();
  expect(screen.getByLabelText('Tool call #2')).toBeInTheDocument();
  expect(queryConversationClientTrajectory).toHaveBeenCalledTimes(requestsBeforeLocale);

});

test('updates mounted client row and cached timeline labels on a locale change', async () => {
  const { loadClientTrajectory } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await screen.findByRole('button', { name: '工具调用 · exec_command' });
  const requests = loadClientTrajectory.mock.calls.length;
  await act(async () => { await appI18n.changeLanguage('en_US'); });
  expect(await screen.findByRole('button', { name: 'Tool call · exec_command' })).toBeInTheDocument();
  expect(screen.getByLabelText('Request #1')).toBeInTheDocument();
  expect(loadClientTrajectory).toHaveBeenCalledTimes(requests);
});

test('canonical index preserves equal-sequence events and loads missing viewport windows by identity', async () => {
  const steps: ConversationClientTrajectoryStep[] = ['first', 'second'].map(
    (name) => ({
      ...tool,
      id: `step-${name}`,
      sequence: 10,
      name: `same-sequence-${name}`,
      record_id: 'record-equal',
      log_conversation_id: 'canonical-equal',
      parent_task_run_id: null,
      is_subagent: name === 'second',
      source_kind: 'imported',
      source_id: 'source-1',
      source_client: 'codex',
      source_session_id: 'session-1',
      source_task_id: 'task-equal',
      flow_run_id: null,
      transport: 'file',
      parent_id: null,
      available_sections: ['parameters']
    })
  );
  const queryConversationClientTrajectory = vi
    .fn()
    .mockImplementation(
      (_canonical: string, query: ConversationTrajectoryQuery) =>
        Promise.resolve({
          log_conversation_id: 'canonical-equal',
          matches: [],
          previous_cursor: null,
          next_cursor: 'opaque-do-not-drain',
          search_sections: ['overview', 'parameters', 'result'],
          integrity: 'complete',
          ...(query.index_only
            ? { items: [], overview: indexOverview(steps) }
            : { items: [steps[query.focus_step_id === 'step-second' ? 1 : 0]] })
        })
    );
  const loadConversationClientTrajectorySection = vi.fn().mockResolvedValue({
    request_id: root.id,
    evidence_scope: 'step',
    step_id: 'step-second',
    section: 'parameters',
    items: [{ sequence: 10, value: 'second same-sequence body' }],
    next_cursor: null
  });
  const loader: ConversationLogTraceLoader = {
    sourceKind: 'imported',
    queryConversationClientTrajectory,
    loadConversationClientTrajectorySection
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ProviderTrajectory
        runId="record-equal"
        logConversationId="canonical-equal"
        loader={loader}
      />
    </QueryClientProvider>
  );
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await screen.findByRole('button', { name: '工具调用 · same-sequence-first' });
  const second = await screen.findByRole('button', {
    name: '工具调用 · same-sequence-second'
  });
  expect(queryConversationClientTrajectory).toHaveBeenCalledWith(
    'canonical-equal',
    expect.objectContaining({
      focus_record_id: 'record-equal',
      focus_step_id: 'step-second',
      index_only: false
    })
  );
  expect(
    queryConversationClientTrajectory.mock.calls.every(
      (call) => call[1].cursor === undefined
    )
  ).toBe(true);
  expect(
    screen.getByText('subagent', {
      selector: '.conversation-trajectory__source'
    })
  ).toBeInTheDocument();
  expect(loadConversationClientTrajectorySection).not.toHaveBeenCalled();
  fireEvent.click(second);
  expect(
    await screen.findByText('second same-sequence body')
  ).toBeInTheDocument();
  expect(loadConversationClientTrajectorySection).toHaveBeenCalledWith(
    'canonical-equal',
    'step-second',
    'parameters',
    undefined
  );
});

test('a large single request stays virtual and a timeline jump loads the target without intermediate pages', async () => {
  const all: ClientTrajectoryStep[] = [
    root,
    ...Array.from({ length: 5000 }, (_, index) => ({
      ...tool,
      id: `large-${index}`,
      sequence: index + 2,
      name: `call-${index}`
    }))
  ];
  const loadClientTrajectory = vi
    .fn()
    .mockImplementation(
      (_run, _node, _cursor, options?: ClientTrajectoryOptions) => {
        const focused = options?.focus_step_id
          ? all.findIndex((step) => step.id === options.focus_step_id)
          : 0;
        return Promise.resolve(
          options?.index_only
            ? {
                items: [],
                overview: indexOverview(all),
                next_cursor: null,
                integrity: 'complete'
              }
            : {
                items: all.slice(
                  Math.max(0, focused),
                  Math.max(0, focused) + 100
                ),
                next_cursor: 100,
                integrity: 'complete'
              }
        );
      }
    );
  const { ClientTrajectoryWorkspace } =
    await import('../../../components/debug-console/trajectory/client/ClientTrajectoryWorkspace');
  const view = render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ClientTrajectoryWorkspace
        runId="run-1"
        loader={{ loadClientTrajectory }}
      />
    </QueryClientProvider>
  );
  await screen.findByRole('button', { name: '工具调用 · call-0' });
  expect(
    view.container.querySelectorAll('.client-trajectory__row').length
  ).toBeLessThan(150);
  expect(
    loadClientTrajectory.mock.calls.filter((call) => !call[3]?.index_only)
      .length
  ).toBeLessThan(5);
  const canvasPaints = vi.mocked(HTMLCanvasElement.prototype.getContext).mock
    .calls.length;
  // Full index keeps the last event keyboard-addressable while its row is absent.
  const axis = view.container.querySelector('.provider-trajectory__lanes')!;
  fireEvent.keyDown(axis, { key: 'End' });
  fireEvent.keyDown(axis, { key: 'Enter' });
  await waitFor(() =>
    expect(loadClientTrajectory).toHaveBeenCalledWith(
      'run-1',
      undefined,
      undefined,
      expect.objectContaining({
        focus_step_id: 'large-4999',
        index_only: false
      })
    )
  );
  expect(
    loadClientTrajectory.mock.calls.every((call) => call[2] === undefined)
  ).toBe(true);
  expect(
    view.container.querySelectorAll('.client-trajectory__row').length
  ).toBeLessThan(150);
  expect(
    vi.mocked(HTMLCanvasElement.prototype.getContext).mock.calls.length
  ).toBe(canvasPaints);
});
