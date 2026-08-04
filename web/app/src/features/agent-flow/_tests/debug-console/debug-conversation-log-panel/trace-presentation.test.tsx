import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode, type ComponentProps, type ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { AgentFlowRunContext } from '../../../api/runtime';
import { AgentFlowDebugConsole } from '../../../components/debug-console/AgentFlowDebugConsole';
import { ConversationLogPanel } from '../../../components/debug-console/ConversationLogPanel';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import {
  answerSnapshotAssistantMessage,
  assistantMessage,
  fusionHistoricalBranchDetailAssistantMessage,
  fusionSummaryOnlyAssistantMessage,
  llmRoundAssistantMessage,
  multiLlmRunAssistantMessage,
  toolCallbackDetailPayload,
  truncatedLlmRoundsAssistantMessage
} from '../debug-conversation-log-panel.fixtures';
const runContext: AgentFlowRunContext = {
  environmentLabel: 'draft',
  remembered: false,
  fields: [
    {
      nodeId: 'node-start',
      nodeLabel: 'Start',
      key: 'query',
      title: '问题',
      valueType: 'string',
      value: '你好?'
    }
  ]
};

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

function expandToolsNode(container: HTMLElement, name: RegExp) {
  const toolsNode = within(container).getByRole('button', { name });

  expect(toolsNode).toHaveAttribute('aria-expanded', 'false');
  fireEvent.click(toolsNode);
  expect(toolsNode).toHaveAttribute('aria-expanded', 'true');

  return toolsNode;
}
function renderConsole(
  props: Partial<ComponentProps<typeof AgentFlowDebugConsole>> = {}
) {
  return render(
    <StrictMode>
      <AgentFlowDebugConsole
        messages={[
          {
            id: 'user-1',
            role: 'user',
            status: 'completed',
            runId: 'run-1',
            content: '你好?',
            rawOutput: null,
            traceSummary: []
          },
          assistantMessage
        ]}
        runContext={runContext}
        status="completed"
        stopping={false}
        onChangeRunContextValue={vi.fn()}
        onClearSession={vi.fn()}
        onClose={vi.fn()}
        onStopRun={vi.fn()}
        onSubmitPrompt={vi.fn()}
        {...props}
      />
    </StrictMode>
  );
}

describe('debug conversation log panel', () => {
  beforeEach(async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
  });

  test('shows clickable trace nodes and reuses node run detail sections', () => {
    renderConsole();

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    expect(within(panel).getByText('4.26 s')).toBeInTheDocument();
    expect(within(panel).queryByText('4257 ms')).not.toBeInTheDocument();
    const llmTraceNode = within(panel).getByRole('button', { name: /LLM/ });

    expect(llmTraceNode).toHaveAttribute('aria-expanded', 'false');

    fireEvent.click(llmTraceNode);

    expect(llmTraceNode).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(panel).getAllByTestId('debug-workflow-node-item')[1]
    ).toHaveAttribute('data-selected', 'false');

    const nodeDetail = within(panel).getByRole('region', {
      name: 'LLM 节点详情'
    });
    expect(nodeDetail).toBeInTheDocument();
    expect(within(nodeDetail).queryByText('LLM')).not.toBeInTheDocument();
    expect(within(nodeDetail).queryByText('llm')).not.toBeInTheDocument();
    expect(within(nodeDetail).getByLabelText('输入 JSON')).toHaveTextContent(
      'prompt'
    );
    expect(
      within(nodeDetail).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('provider');
    expect(within(nodeDetail).getByLabelText('输出 JSON')).toHaveTextContent(
      '你好，我可以帮你。'
    );
    expect(
      within(panel).getAllByTestId('debug-workflow-node-row')
    ).toHaveLength(2);
  }, 10_000);

  test('groups LLM tool callbacks behind a virtual Tools child node', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '天气?',
          rawOutput: null,
          traceSummary: []
        },
        llmRoundAssistantMessage
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    fireEvent.click(within(panel).getByRole('button', { name: /LLM/ }));

    const nodeDetail = within(panel).getByRole('region', {
      name: 'LLM 节点详情'
    });
    expect(within(nodeDetail).queryByText('Round #1')).not.toBeInTheDocument();
    expect(
      within(nodeDetail).queryByText('Tool Callback #1')
    ).not.toBeInTheDocument();

    expandToolsNode(nodeDetail, /工具.*1 次工具回调/);
    expect(
      within(nodeDetail).queryByText('temperature')
    ).not.toBeInTheDocument();

    expect(
      within(nodeDetail).queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();

    const toolCallback = within(nodeDetail).getByRole('button', {
      name: /lookup_weather.*14 tokens.*1\.23 s/
    });
    expect(toolCallback).toHaveTextContent('lookup_weather');
    expect(toolCallback).toHaveTextContent('14 tokens · 1.23 s');
    expect(toolCallback).not.toHaveTextContent('+10 tokens');
    expect(toolCallback).toHaveAttribute('aria-expanded', 'false');
    expect(
      within(nodeDetail).queryByText('call_weather')
    ).not.toBeInTheDocument();
    expect(
      within(nodeDetail).queryByText('temperature')
    ).not.toBeInTheDocument();

    fireEvent.click(toolCallback);

    expect(toolCallback).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(nodeDetail).getByLabelText('工具调用 JSON')
    ).toHaveTextContent('Shanghai');
    expect(
      within(nodeDetail).getByLabelText('完整回调 JSON')
    ).toHaveTextContent('temperature');
    expect(nodeDetail).not.toHaveTextContent('工具 token 归因');
    expect(
      within(nodeDetail).getByLabelText('工具调用 JSON')
    ).toHaveTextContent('total_tokens');
    expect(
      within(nodeDetail).getByLabelText('完整回调 JSON')
    ).toHaveTextContent('result_context_usage');
    expect(nodeDetail).toHaveTextContent('已返回');
    expect(nodeDetail).not.toHaveTextContent('执行未知');
    expect(nodeDetail).toHaveTextContent('weather is clear');
    within(nodeDetail)
      .getAllByLabelText('数据处理 JSON')
      .forEach((block) => {
        expect(block).not.toHaveTextContent('llm_rounds');
      });
  }, 10_000);

  test('shows empty detail sections for route branch nodes without detail', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '做 fusion 评审',
          rawOutput: null,
          traceSummary: []
        },
        fusionSummaryOnlyAssistantMessage
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    fireEvent.click(within(panel).getByRole('button', { name: /LLM/ }));
    expandToolsNode(panel, /工具.*1 次工具回调/);
    fireEvent.click(
      within(panel).getByRole('button', { name: /fusion_review/ })
    );

    const branchNode = within(panel).getByTestId('debug-llm-route-branch-node');
    expect(
      within(panel).queryByTestId('debug-llm-route-node')
    ).not.toBeInTheDocument();
    fireEvent.click(within(branchNode).getByRole('button', { name: /LLM2/ }));

    expect(within(branchNode).getByLabelText('输入 JSON')).toHaveTextContent(
      '{}'
    );
    expect(
      within(branchNode).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('{}');
    expect(within(branchNode).getByLabelText('输出 JSON')).toHaveTextContent(
      '{}'
    );
  }, 10_000);

  test('shows fusion branch LLM tokens from metrics payload and reuses node detail sections', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '做 fusion 评审',
          rawOutput: null,
          traceSummary: []
        },
        fusionHistoricalBranchDetailAssistantMessage
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    fireEvent.click(within(panel).getByRole('button', { name: /LLM/ }));
    expandToolsNode(panel, /工具.*1 次工具回调/);
    fireEvent.click(
      within(panel).getByRole('button', { name: /fusion_review/ })
    );

    const branchNode = within(panel).getByTestId('debug-llm-route-branch-node');
    const branchButton = within(branchNode).getByRole('button', {
      name: /LLM5/
    });
    expect(branchButton).toHaveTextContent('7.96 K tokens');
    expect(branchButton).not.toHaveTextContent('执行成功');

    fireEvent.click(branchButton);

    expect(within(branchNode).getByLabelText('输入 JSON')).toHaveTextContent(
      'Merge panel answers.'
    );
    expect(
      within(branchNode).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('assistant_message');
    expect(within(branchNode).getByLabelText('输出 JSON')).toHaveTextContent(
      'judge merged answer'
    );
  }, 10_000);

  test('collapses repeated LLM node runs into one trace row', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '天气?',
          rawOutput: null,
          traceSummary: []
        },
        multiLlmRunAssistantMessage
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));

    expect(
      within(panel).getAllByTestId('debug-workflow-node-row')
    ).toHaveLength(2);

    const llmTraceNode = within(panel).getByRole('button', { name: /LLM/ });
    expect(llmTraceNode).toHaveTextContent('工具 2');

    fireEvent.click(llmTraceNode);

    const nodeDetail = within(panel).getByRole('region', {
      name: 'LLM 节点详情'
    });
    expandToolsNode(nodeDetail, /工具.*2 次工具回调/);

    expect(
      within(nodeDetail).queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    expect(
      within(nodeDetail).getByRole('button', {
        name: /lookup_weather/
      })
    ).toBeInTheDocument();
    expect(
      within(nodeDetail).getByRole('button', {
        name: /read_policy/
      })
    ).toBeInTheDocument();
    expect(
      within(nodeDetail).queryByText('call_weather')
    ).not.toBeInTheDocument();
    expect(
      within(nodeDetail).queryByText('call_policy')
    ).not.toBeInTheDocument();
    expect(within(nodeDetail).getByLabelText('输出 JSON')).toHaveTextContent(
      'weather is clear'
    );
  }, 10_000);

  test('renders waiting answer snapshots inside the waiting LLM trace row', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '继续?',
          rawOutput: null,
          traceSummary: []
        },
        answerSnapshotAssistantMessage
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));

    expect(
      within(panel).getAllByTestId('debug-workflow-node-row')
    ).toHaveLength(1);
    expect(within(panel).queryByText('直接回复')).not.toBeInTheDocument();

    const llmTraceNode = within(panel).getByRole('button', { name: /LLM2/ });
    fireEvent.click(llmTraceNode);

    const nodeDetail = within(panel).getByRole('region', {
      name: 'LLM2 节点详情'
    });
    const answerSnapshot = within(nodeDetail).getByRole('button', {
      name: /answer快照/
    });
    expect(answerSnapshot).toHaveAttribute('aria-expanded', 'false');
    expect(
      within(nodeDetail).queryByText('LLM1 final')
    ).not.toBeInTheDocument();

    fireEvent.click(answerSnapshot);

    expect(answerSnapshot).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(nodeDetail).getByLabelText('answer快照 JSON')
    ).toHaveTextContent('LLM1 final');
  }, 10_000);

  test('loads full LLM tool callbacks when the rounds payload is truncated', async () => {
    const onLoadArtifact = vi.fn().mockResolvedValue(toolCallbackDetailPayload);

    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '天气?',
          rawOutput: null,
          traceSummary: []
        },
        truncatedLlmRoundsAssistantMessage
      ],
      onLoadArtifact
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    fireEvent.click(within(panel).getByRole('button', { name: /LLM/ }));

    const nodeDetail = within(panel).getByRole('region', {
      name: 'LLM 节点详情'
    });
    expandToolsNode(nodeDetail, /工具.*1 次工具回调/);

    expect(
      within(nodeDetail).queryByRole('button', { name: '加载完整工具' })
    ).not.toBeInTheDocument();
    expect(onLoadArtifact).not.toHaveBeenCalled();
    const toolCallback = within(nodeDetail).getByRole('button', {
      name: /lookup_weather.*14 tokens.*1\.23 s/
    });
    expect(toolCallback).toHaveTextContent('14 tokens · 1.23 s');
    expect(toolCallback).not.toHaveTextContent('+10 tokens');
    expect(
      within(nodeDetail).queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    expect(within(nodeDetail).queryByText('Shanghai')).not.toBeInTheDocument();

    fireEvent.click(toolCallback);

    expect(onLoadArtifact).toHaveBeenCalledWith('artifact-tool-call-weather');
    expect(
      await within(nodeDetail).findByLabelText('工具调用 JSON')
    ).toHaveTextContent('Shanghai');
    expect(
      within(nodeDetail).getByLabelText('完整回调 JSON')
    ).toHaveTextContent('trace-weather-1');
    expect(
      within(nodeDetail).getByLabelText('解析结果 JSON')
    ).toHaveTextContent('temperature');
    expect(nodeDetail).not.toHaveTextContent('工具 token 归因');
    expect(
      within(nodeDetail).getByLabelText('工具调用 JSON')
    ).toHaveTextContent('call_usage');
    expect(
      within(nodeDetail).getByLabelText('完整回调 JSON')
    ).toHaveTextContent('result_context_usage');
  }, 10_000);

  test('treats stitched context and stitched runs as group-only trace nodes', async () => {
    const stitchedContextNode = {
      trace_node_id: 'stitched_context:root',
      node_kind: 'stitched_context',
      node_run_id: null,
      node_id: null,
      node_type: 'stitched_context',
      node_alias: '续聊上下文',
      status: 'succeeded',
      started_at: '2026-04-25T09:59:00Z',
      finished_at: '2026-04-25T10:00:00Z',
      duration_ms: null,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: false
    };
    const stitchedRunNode = {
      trace_node_id: 'stitched_run:prior-run',
      parent_trace_node_id: stitchedContextNode.trace_node_id,
      node_kind: 'stitched_run',
      node_run_id: null,
      node_id: null,
      node_type: 'flow_run',
      node_alias: '历史 run',
      status: 'succeeded',
      started_at: '2026-04-25T09:59:01Z',
      finished_at: '2026-04-25T09:59:30Z',
      duration_ms: 29000,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: false,
      source_flow_run_id: 'run-prior'
    };
    const historicalNode = {
      trace_node_id: 'stitched_node_run:prior-llm',
      parent_trace_node_id: stitchedRunNode.trace_node_id,
      node_kind: 'node_run',
      node_run_id: 'prior-node-run',
      node_id: 'prior-node',
      node_type: 'llm',
      node_alias: 'Prior LLM',
      status: 'succeeded',
      started_at: '2026-04-25T09:59:02Z',
      finished_at: '2026-04-25T09:59:20Z',
      duration_ms: 18000,
      metrics_payload: {},
      has_children: false,
      child_count: 0,
      has_content: true,
      source_flow_run_id: 'run-prior',
      source_trace_node_id: 'node_run:prior-node-run'
    };
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [stitchedContextNode] }),
      loadChildren: vi
        .fn()
        .mockImplementation(
          async (_runId: string, parentTraceNodeId: string) => ({
            items:
              parentTraceNodeId === stitchedContextNode.trace_node_id
                ? [stitchedRunNode]
                : parentTraceNodeId === stitchedRunNode.trace_node_id
                  ? [historicalNode]
                  : [],
            page_info: {
              has_more: false,
              next_cursor: null,
              page_size: 20
            }
          })
        ),
      loadContent: vi
        .fn()
        .mockImplementation(async (_runId: string, traceNodeId: string) => ({
          trace_node_id: traceNodeId,
          node_kind: 'node_run',
          content_kind: 'node_run',
          detail_refs: [
            {
              detail_ref_id: 'node_run',
              detail_kind: 'node_run',
              source_kind: 'stitched_node_run',
              source_locator: 'prior-node-run',
              source_flow_run_id: 'run-prior',
              count: 1
            }
          ],
          payload: {
            payload_index: {
              node_run_count: 1,
              checkpoint_count: 0,
              event_count: 0,
              source_flow_run_id: 'run-prior'
            }
          }
        })),
      loadDetail: vi.fn().mockResolvedValue({
        trace_node_id: historicalNode.trace_node_id,
        detail_ref_id: 'node_run',
        detail_kind: 'node_run',
        payload: {
          node_run: {
            id: 'prior-node-run',
            flow_run_id: 'run-prior',
            node_id: 'prior-node',
            node_type: 'llm',
            node_alias: 'Prior LLM',
            status: 'succeeded',
            input_payload: {
              prompt: '历史问题'
            },
            output_payload: {
              answer: '历史回答'
            },
            error_payload: null,
            metrics_payload: {},
            debug_payload: {},
            started_at: '2026-04-25T09:59:02Z',
            finished_at: '2026-04-25T09:59:20Z'
          }
        }
      })
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-application-log',
          role: 'assistant',
          content: '当前回答',
          status: 'completed',
          runId: 'run-application-log',
          detailRunId: 'run-application-log',
          rawOutput: null,
          traceSummary: []
        }}
        traceLoader={traceLoader}
        onClose={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('tab', { name: '追踪' }));

    fireEvent.click(await screen.findByRole('button', { name: /续聊上下文/ }));
    await waitFor(() =>
      expect(traceLoader.loadChildren).toHaveBeenCalledWith(
        'run-application-log',
        stitchedContextNode.trace_node_id,
        undefined
      )
    );
    expect(
      screen.queryByRole('region', { name: '续聊上下文 节点详情' })
    ).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole('button', { name: /历史 run/ }));
    await waitFor(() =>
      expect(traceLoader.loadChildren).toHaveBeenCalledWith(
        'run-application-log',
        stitchedRunNode.trace_node_id,
        undefined
      )
    );
    expect(
      screen.queryByRole('region', { name: '历史 run 节点详情' })
    ).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole('button', { name: /Prior LLM/ }));
    await waitFor(() =>
      expect(traceLoader.loadContent).toHaveBeenCalledWith(
        'run-application-log',
        historicalNode.trace_node_id
      )
    );
    const nodeDetail = await screen.findByRole('region', {
      name: 'Prior LLM 节点详情'
    });
    expect(
      await within(nodeDetail).findByLabelText('输入 JSON')
    ).toHaveTextContent('历史问题');
    expect(
      await within(nodeDetail).findByLabelText('输出 JSON')
    ).toHaveTextContent('历史回答');
  });

  test('delegates log opening when the canvas shell controls the log panel', () => {
    const onOpenMessageLog = vi.fn();

    renderConsole({ onOpenMessageLog });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));

    expect(onOpenMessageLog).toHaveBeenCalledWith(assistantMessage);
    expect(
      screen.queryByRole('complementary', { name: '对话日志' })
    ).not.toBeInTheDocument();
  });
});
