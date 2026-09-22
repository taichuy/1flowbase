import './navigation';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import type { ClientTrajectoryStep } from '@1flowbase/api-client';
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
function fixture(namespace: string | null = null) {
  const loadClientTrajectory = vi
    .fn()
    .mockImplementation((_run, _node, cursor) =>
      Promise.resolve({
        items: cursor
          ? [
              {
                ...root,
                id: 'result-1',
                sequence: 3,
                category: 'tool_result',
                name: 'exec_command',
                result_preview: '/work',
                related_step_id: tool.id,
                call_id: tool.call_id,
                available_sections: ['result']
              }
            ]
          : [root, { ...tool, namespace }],
        next_cursor: cursor ? null : 2,
        integrity: 'complete'
      })
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
  const loadRunTrajectory = vi
    .fn()
    .mockResolvedValue({ items: [], next_cursor: null });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadClientTrajectory,
    loadClientTrajectorySection,
    loadRunTrajectory
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
    loadRunTrajectory
  };
}
test('defaults to original client classification and lazily loads only selected sections', async () => {
  const {
    loadClientTrajectory,
    loadClientTrajectorySection,
    loadRunTrajectory
  } = fixture();
  expect(loadClientTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  const call = await screen.findByRole('button', {
    name: '工具调用 · exec_command'
  });
  expect(loadRunTrajectory).not.toHaveBeenCalled();
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
test('paginates summaries, keeps original categories and follows actual call relation', async () => {
  const { loadClientTrajectory } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await screen.findByRole('button', { name: '工具调用 · exec_command' });
  fireEvent.click(screen.getByRole('button', { name: '加载更多步骤' }));
  const result = await screen.findByRole('button', {
    name: '工具结果 · exec_command'
  });
  expect(loadClientTrajectory).toHaveBeenCalledWith(
    'run-1',
    undefined,
    2,
    undefined
  );
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
  fixture('mcp__codex_apps__github');
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  const row = await screen.findByRole('button', {
    name: '工具调用 · mcp__codex_apps__github.exec_command'
  });
  fireEvent.change(screen.getByRole('textbox', { name: '搜索已加载步骤' }), {
    target: { value: 'mcp__codex_apps__github' }
  });
  expect(row).toBeInTheDocument();
  fireEvent.click(row);
  const inspector = screen.getByRole('complementary', { name: '步骤检查器' });
  expect(within(inspector).getByText('namespace')).toBeInTheDocument();
  expect(
    within(inspector).getByText('mcp__codex_apps__github')
  ).toBeInTheDocument();
});
