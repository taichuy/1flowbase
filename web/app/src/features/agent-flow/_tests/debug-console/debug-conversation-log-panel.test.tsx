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

import type { AgentFlowRunContext } from '../../api/runtime';
import { AgentFlowDebugConsole } from '../../components/debug-console/AgentFlowDebugConsole';
import { ConversationLogPanel } from '../../components/debug-console/ConversationLogPanel';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import {
  answerSnapshotAssistantMessage,
  assistantMessage,
  fusionHistoricalBranchDetailAssistantMessage,
  fusionSummaryOnlyAssistantMessage,
  llmRoundAssistantMessage,
  multiLlmRunAssistantMessage,
  toolCallbackDetailPayload,
  truncatedLlmRoundsAssistantMessage
} from './debug-conversation-log-panel.fixtures';
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

  test('opens from an assistant message and keeps detail limited to input, output and metadata', () => {
    renderConsole();

    expect(
      screen.queryByRole('complementary', { name: '对话日志' })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));

    const panel = screen.getByRole('complementary', { name: '对话日志' });
    expect(panel).toBeInTheDocument();
    expect(within(panel).getByRole('tab', { name: '详情' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(within(panel).getByLabelText('输入 JSON')).toHaveTextContent(
      '你好?'
    );
    expect(within(panel).getByLabelText('输出 JSON')).toHaveTextContent(
      '你好，我可以帮你。'
    );
    expect(within(panel).getByText('元数据')).toBeInTheDocument();
    expect(within(panel).getByText('run-1')).toBeInTheDocument();
    expect(within(panel).getByText('协议')).toBeInTheDocument();
    expect(within(panel).getByText('OpenAI Responses')).toBeInTheDocument();
    expect(within(panel).getByText('总 tokens')).toBeInTheDocument();
    expect(within(panel).getByText('154')).toBeInTheDocument();
    expect(within(panel).getByText('真实节点数')).toBeInTheDocument();
    expect(within(panel).getByText('2')).toBeInTheDocument();
    expect(within(panel).getByText('工具回调次数')).toBeInTheDocument();
    expect(within(panel).getByText('0')).toBeInTheDocument();
    expect(within(panel).queryByText('节点数')).not.toBeInTheDocument();
    expect(within(panel).queryByText('数据处理')).not.toBeInTheDocument();
    expect(within(panel).queryByText('provider')).not.toBeInTheDocument();
  });

  test('shows intercepted tool trace nodes instead of success', () => {
    renderConsole({
      messages: [
        {
          id: 'user-1',
          role: 'user',
          status: 'completed',
          runId: 'run-1',
          content: '看这张图',
          rawOutput: null,
          traceSummary: []
        },
        {
          ...assistantMessage,
          traceSummary: [
            {
              nodeId: 'tool-image-llm',
              nodeRunId: 'tool-image-llm-run',
              nodeAlias: 'image_llm',
              nodeType: 'tool',
              status: 'intercepted',
              startedAt: '2026-04-25T10:00:01Z',
              finishedAt: '2026-04-25T10:00:02Z',
              durationMs: null,
              inputPayload: {},
              outputPayload: {
                error: {
                  details: {
                    error_code: 'visible_internal_llm_tool_media_unavailable'
                  }
                }
              },
              errorPayload: null,
              metricsPayload: {},
              debugPayload: {
                route_trace: {
                  route_kind: 'route',
                  status: 'intercepted'
                }
              }
            }
          ]
        }
      ]
    });

    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    const panel = screen.getByRole('complementary', { name: '对话日志' });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));

    const toolNode = within(panel).getByRole('button', { name: /image_llm/ });
    expect(toolNode).toHaveTextContent('拦截');
    expect(toolNode).not.toHaveTextContent('执行成功');
  });

  test('loads lazy overview for application log details before trace root', async () => {
    const loadOverview = vi.fn().mockResolvedValue({
      run: {
        id: 'run-application-log',
        compatibility_mode: 'openai-responses-v1',
        started_at: '2026-04-25T10:00:00Z',
        finished_at: '2026-04-25T10:00:05Z'
      },
      statistics: {
        total_tokens: 154,
        unique_node_count: 2,
        tool_callback_count: 0
      },
      flow_run: {
        id: 'run-application-log',
        status: 'succeeded',
        input_payload: {
          'node-start': {
            query: '总结退款政策',
            model: 'deepseek-chat'
          }
        },
        output_payload: {
          answer: '退款政策摘要'
        },
        error_payload: null,
        started_at: '2026-04-25T10:00:00Z',
        finished_at: '2026-04-25T10:00:05Z'
      },
      answer_snapshot: null
    });
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [] }),
      loadChildren: vi.fn(),
      loadContent: vi.fn()
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-application-log',
          role: 'assistant',
          content: '退款政策摘要',
          status: 'completed',
          runId: 'run-application-log',
          detailRunId: 'run-application-log',
          rawOutput: null,
          traceSummary: []
        }}
        overviewLoader={{ loadOverview }}
        traceLoader={traceLoader}
        onClose={vi.fn()}
      />
    );

    await waitFor(() =>
      expect(screen.getByLabelText('输入 JSON')).toHaveTextContent('query')
    );
    expect(screen.getByLabelText('输入 JSON')).toHaveTextContent(
      '总结退款政策'
    );
    expect(screen.getByLabelText('输出 JSON')).toHaveTextContent(
      '退款政策摘要'
    );
    expect(screen.getByText('run-application-log')).toBeInTheDocument();
    expect(screen.getByText('154')).toBeInTheDocument();
    expect(loadOverview).toHaveBeenCalledWith('run-application-log');
    expect(traceLoader.loadTree).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('tab', { name: '追踪' }));

    await waitFor(() =>
      expect(traceLoader.loadTree).toHaveBeenCalledWith('run-application-log')
    );
  });

  test('shows projection status instead of empty trace while the lazy trace index is pending', async () => {
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({
        projection_status: {
          projection_status: 'pending',
          projection_version: 1,
          source_watermark: 'run-application-log:1',
          attempt_count: 0,
          last_attempt_at: null,
          last_success_at: null,
          last_error_code: null,
          last_error_stage: null,
          last_error_source_kind: null,
          last_error_source_locator: null,
          last_error_ref: null,
          retriable: true
        },
        nodes: []
      }),
      loadChildren: vi.fn(),
      loadContent: vi.fn()
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-application-log',
          role: 'assistant',
          content: '退款政策摘要',
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

    expect(await screen.findByText('追踪索引等待生成')).toBeInTheDocument();
    expect(screen.queryByText('暂无追踪记录')).not.toBeInTheDocument();
    expect(traceLoader.loadChildren).not.toHaveBeenCalled();
    expect(traceLoader.loadContent).not.toHaveBeenCalled();
  });

  test('loads lazy trace children and node content when a node expands', async () => {
    const rootNode = {
      trace_node_id: 'node_run:node-run-llm',
      node_kind: 'node_run',
      node_run_id: 'node-run-llm',
      node_id: 'node-llm',
      node_type: 'llm',
      node_alias: 'LLM',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:01Z',
      finished_at: '2026-04-25T10:00:05Z',
      duration_ms: 4000,
      metrics_payload: {
        total_tokens: 154
      },
      has_children: true,
      has_content: true
    };
    const childNode = {
      trace_node_id: 'callback_task:callback-weather',
      node_kind: 'callback_task',
      node_run_id: null,
      node_id: null,
      node_type: 'callback_task',
      node_alias: 'lookup_weather',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:02Z',
      finished_at: '2026-04-25T10:00:03Z',
      duration_ms: 1000,
      metrics_payload: {},
      has_children: false,
      has_content: false
    };
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [rootNode] }),
      loadChildren: vi.fn().mockResolvedValue({
        items: [childNode],
        page_info: {
          has_more: false,
          next_cursor: null,
          page_size: 20
        }
      }),
      loadContent: vi.fn().mockResolvedValue({
        trace_node_id: 'node_run:node-run-llm',
        node_kind: 'node_run',
        content_kind: 'node_run',
        payload: {
          payload_index: {
            node_run_count: 1,
            checkpoint_count: 0,
            event_count: 0
          }
        },
        detail_refs: [
          {
            detail_ref_id: 'node_run',
            detail_kind: 'node_run',
            source_kind: 'node_run',
            source_locator: 'node-run-llm',
            count: 1
          }
        ]
      }),
      loadDetail: vi.fn().mockResolvedValue({
        trace_node_id: 'node_run:node-run-llm',
        detail_ref_id: 'node_run',
        detail_kind: 'node_run',
        payload: {
          node_run: {
            id: 'node-run-llm',
            node_id: 'node-llm',
            node_type: 'llm',
            node_alias: 'LLM',
            status: 'succeeded',
            input_payload: {
              prompt: '总结退款政策'
            },
            output_payload: {
              answer: '退款政策摘要'
            },
            error_payload: null,
            metrics_payload: {
              total_tokens: 154
            },
            debug_payload: {
              provider: 'deepseek'
            },
            started_at: '2026-04-25T10:00:01Z',
            finished_at: '2026-04-25T10:00:05Z'
          }
        }
      })
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-application-log',
          role: 'assistant',
          content: '退款政策摘要',
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
    await waitFor(() =>
      expect(traceLoader.loadTree).toHaveBeenCalledWith('run-application-log')
    );
    expect(traceLoader.loadChildren).not.toHaveBeenCalled();
    expect(traceLoader.loadContent).not.toHaveBeenCalled();

    const llmTraceNode = await screen.findByRole('button', { name: /LLM/ });
    fireEvent.click(llmTraceNode);

    await waitFor(() =>
      expect(traceLoader.loadChildren).toHaveBeenCalledWith(
        'run-application-log',
        'node_run:node-run-llm',
        undefined
      )
    );
    await waitFor(() =>
      expect(traceLoader.loadContent).toHaveBeenCalledWith(
        'run-application-log',
        'node_run:node-run-llm'
      )
    );
    await waitFor(() =>
      expect(traceLoader.loadDetail).toHaveBeenCalledWith(
        'run-application-log',
        'node_run:node-run-llm',
        'node_run'
      )
    );
    const nodeDetail = await screen.findByRole('region', {
      name: 'LLM 节点详情'
    });
    expect(
      within(nodeDetail).queryByRole('button', { name: '详情' })
    ).not.toBeInTheDocument();
    expect(
      await within(nodeDetail).findByRole('button', { name: /lookup_weather/ })
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(within(nodeDetail).getByLabelText('输入 JSON')).toHaveTextContent(
        '总结退款政策'
      )
    );
  });
});
