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

  test('renders summary-only tool group trace nodes as pure collapsible groups', async () => {
    const rootNode = {
      trace_node_id: 'tool_group:node-run-empty',
      node_kind: 'tool_group',
      node_run_id: null,
      node_id: null,
      node_type: 'tools',
      node_alias: 'Tools',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:01Z',
      finished_at: '2026-04-25T10:00:02Z',
      duration_ms: 1000,
      metrics_payload: {},
      has_children: false,
      child_count: 0,
      has_content: false
    };
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [rootNode] }),
      loadChildren: vi.fn(),
      loadContent: vi.fn()
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-empty-trace-node',
          role: 'assistant',
          content: '空 trace 节点',
          status: 'completed',
          runId: 'run-empty-trace-node',
          detailRunId: 'run-empty-trace-node',
          rawOutput: null,
          traceSummary: []
        }}
        traceLoader={traceLoader}
        onClose={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('tab', { name: '追踪' }));
    const toolsTraceNode = await screen.findByRole('button', { name: /Tools/ });
    fireEvent.click(toolsTraceNode);
    const toolsTraceItem = screen.getByTestId('debug-workflow-node-item');

    expect(
      screen.queryByRole('region', {
        name: 'Tools 节点详情'
      })
    ).not.toBeInTheDocument();
    expect(
      within(toolsTraceItem).queryByLabelText('输入 JSON')
    ).not.toBeInTheDocument();
    expect(
      within(toolsTraceItem).queryByLabelText('数据处理 JSON')
    ).not.toBeInTheDocument();
    expect(
      within(toolsTraceItem).queryByLabelText('输出 JSON')
    ).not.toBeInTheDocument();
    expect(traceLoader.loadContent).not.toHaveBeenCalled();
  });

  test('renders backend-linked agent groups as subagent LLM nodes with their own tools', async () => {
    const rootNode = {
      trace_node_id: 'node_run:parent-llm',
      node_kind: 'node_run',
      flow_run_id: 'run-application-log',
      node_run_id: 'parent-llm',
      node_id: 'node-parent-llm',
      node_type: 'llm',
      node_alias: 'Parent LLM',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:01Z',
      finished_at: '2026-04-25T10:00:10Z',
      duration_ms: 9000,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: true
    };
    const agentsNode = {
      trace_node_id: 'agent_group:parent-llm',
      parent_trace_node_id: rootNode.trace_node_id,
      node_kind: 'agent_group',
      flow_run_id: 'run-application-log',
      node_run_id: null,
      node_id: null,
      node_type: 'agents',
      node_alias: 'Agents',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:02Z',
      finished_at: '2026-04-25T10:00:09Z',
      duration_ms: 7000,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: false
    };
    const subagentNode = {
      trace_node_id: 'subagent_node_run:research-agent',
      parent_trace_node_id: agentsNode.trace_node_id,
      node_kind: 'node_run',
      flow_run_id: 'run-application-log',
      node_run_id: 'research-agent-node-run',
      node_id: 'research-agent-node',
      node_type: 'llm',
      node_alias: 'Research agent',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:03Z',
      finished_at: '2026-04-25T10:00:08Z',
      duration_ms: 5000,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: true,
      source_flow_run_id: 'run-subagent-research',
      source_trace_node_id: 'node_run:research-agent-node-run',
      parent_callback_task_id: 'callback-agent-task',
      parent_tool_call_id: 'tooluse-agent',
      trace_relation_kind: 'subagent'
    };
    const subagentToolsNode = {
      trace_node_id: 'tool_group:research-agent',
      parent_trace_node_id: subagentNode.trace_node_id,
      node_kind: 'tool_group',
      flow_run_id: 'run-application-log',
      node_run_id: null,
      node_id: null,
      node_type: 'tools',
      node_alias: 'Tools',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:04Z',
      finished_at: '2026-04-25T10:00:05Z',
      duration_ms: 1000,
      metrics_payload: {},
      has_children: true,
      child_count: 1,
      has_content: false
    };
    const subagentToolCallbackNode = {
      trace_node_id: 'tool_callback:subagent-bash',
      parent_trace_node_id: subagentToolsNode.trace_node_id,
      node_kind: 'tool_callback',
      flow_run_id: 'run-application-log',
      node_run_id: null,
      node_id: null,
      node_type: 'tool',
      node_mode: null,
      node_alias: 'Bash',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:04Z',
      finished_at: '2026-04-25T10:00:05Z',
      duration_ms: 1000,
      metrics_payload: {},
      has_children: false,
      child_count: 0,
      has_content: true
    };
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [rootNode] }),
      loadChildren: vi
        .fn()
        .mockImplementation(
          async (_runId: string, parentTraceNodeId: string) => ({
            items:
              parentTraceNodeId === rootNode.trace_node_id
                ? [agentsNode]
                : parentTraceNodeId === agentsNode.trace_node_id
                  ? [subagentNode]
                  : parentTraceNodeId === subagentNode.trace_node_id
                    ? [subagentToolsNode]
                    : parentTraceNodeId === subagentToolsNode.trace_node_id
                      ? [subagentToolCallbackNode]
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
          node_kind:
            traceNodeId === subagentToolCallbackNode.trace_node_id
              ? 'tool_callback'
              : 'node_run',
          content_kind:
            traceNodeId === subagentToolCallbackNode.trace_node_id
              ? 'tool_callback'
              : 'node_run',
          payload:
            traceNodeId === subagentToolCallbackNode.trace_node_id
              ? {
                  id: 'tooluse-subagent-bash',
                  name: 'Bash',
                  callback_status: 'returned',
                  execution_status: 'succeeded',
                  request_payload: {
                    arguments: {
                      command: 'rg agent'
                    }
                  },
                  callback_payload: {
                    content: 'agent relation found'
                  },
                  parsed_result: {
                    content: 'agent relation found'
                  },
                  duration_ms: 1000
                }
              : {
                  payload_index: {
                    node_run_count: 1,
                    checkpoint_count: 0,
                    event_count: 0
                  },
                  debug_payload: {
                    parent_agent_tool_call: {
                      description: 'Research agent short brief'
                    }
                  }
                },
          detail_refs:
            traceNodeId === subagentToolCallbackNode.trace_node_id
              ? []
              : [
                  {
                    detail_ref_id: 'node_run',
                    detail_kind: 'node_run',
                    source_kind: 'node_run',
                    source_locator:
                      traceNodeId === subagentNode.trace_node_id
                        ? 'research-agent-node-run'
                        : 'parent-llm',
                    count: 1
                  }
                ]
        })),
      loadDetail: vi
        .fn()
        .mockImplementation(
          async (
            _runId: string,
            traceNodeId: string
          ) => ({
            trace_node_id: traceNodeId,
            detail_ref_id: 'node_run',
            detail_kind: 'node_run',
            payload: {
              node_run:
                traceNodeId === subagentNode.trace_node_id
                  ? {
                      id: 'research-agent-node-run',
                      node_id: 'research-agent-node',
                      node_type: 'llm',
                      node_alias: 'Research agent',
                      status: 'succeeded',
                      input_payload: {
                        prompt: 'Investigate agent projection'
                      },
                      output_payload: {
                        answer: 'Use a dedicated Agents group'
                      },
                      error_payload: null,
                      metrics_payload: {},
                      debug_payload: {
                        provider: 'anthropic'
                      },
                      started_at: '2026-04-25T10:00:03Z',
                      finished_at: '2026-04-25T10:00:08Z'
                    }
                  : {
                      id: 'parent-llm',
                      node_id: 'node-parent-llm',
                      node_type: 'llm',
                      node_alias: 'Parent LLM',
                      status: 'succeeded',
                      input_payload: {
                        prompt: 'Coordinate subagents'
                      },
                      output_payload: {
                        answer: 'Subagent done'
                      },
                      error_payload: null,
                      metrics_payload: {},
                      debug_payload: {},
                      started_at: '2026-04-25T10:00:01Z',
                      finished_at: '2026-04-25T10:00:10Z'
                    }
            }
          })
        )
    };

    renderWithQueryClient(
      <ConversationLogPanel
        message={{
          id: 'conversation-assistant-run-subagents',
          role: 'assistant',
          content: 'Subagent done',
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
    fireEvent.click(await screen.findByRole('button', { name: /Parent LLM/ }));

    const parentDetail = await screen.findByRole('region', {
      name: 'Parent LLM 节点详情'
    });
    const agentsButton = await within(parentDetail).findByRole('button', {
      name: /Agents/
    });
    expect(agentsButton).toHaveAttribute('aria-expanded', 'false');
    expect(
      within(parentDetail).queryByRole('region', {
        name: 'Agents 节点详情'
      })
    ).not.toBeInTheDocument();
    expect(traceLoader.loadContent).not.toHaveBeenCalledWith(
      'run-application-log',
      agentsNode.trace_node_id
    );

    fireEvent.click(agentsButton);
    await waitFor(() =>
      expect(traceLoader.loadChildren).toHaveBeenCalledWith(
        'run-application-log',
        agentsNode.trace_node_id,
        undefined
      )
    );
    const subagentButton = await within(parentDetail).findByRole('button', {
      name: /Research agent/
    });
    fireEvent.click(subagentButton);

    await waitFor(() =>
      expect(traceLoader.loadContent).toHaveBeenCalledWith(
        'run-application-log',
        subagentNode.trace_node_id
      )
    );
    await waitFor(() =>
      expect(traceLoader.loadDetail).toHaveBeenCalledWith(
        'run-application-log',
        subagentNode.trace_node_id,
        'node_run'
      )
    );
    const subagentDetail = await within(parentDetail).findByRole('region', {
      name: 'Research agent 节点详情'
    });
    expect(
      within(subagentDetail).getByLabelText('输入 JSON')
    ).toHaveTextContent('Investigate agent projection');
    expect(
      within(subagentDetail).getByLabelText('输入 JSON')
    ).not.toHaveTextContent('Research agent short brief');
    expect(
      within(subagentDetail).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('anthropic');
    expect(
      within(subagentDetail).getByLabelText('数据处理 JSON')
    ).toHaveTextContent('Research agent short brief');
    expect(
      within(subagentDetail).getByLabelText('输出 JSON')
    ).toHaveTextContent('Use a dedicated Agents group');

    fireEvent.click(
      await within(subagentDetail).findByRole('button', { name: /Tools/ })
    );
    expect(
      await within(subagentDetail).findByRole('button', { name: /Bash/ })
    ).toBeInTheDocument();
  }, 10_000);

  test('loads lazy trace tool details only when a tool callback expands', async () => {
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
      metrics_payload: {},
      has_children: true,
      has_content: true
    };
    const toolsNode = {
      trace_node_id: 'tool_group:node-run-llm',
      node_kind: 'tool_group',
      node_run_id: null,
      node_id: null,
      node_type: 'tools',
      node_alias: 'Tools',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:02Z',
      finished_at: '2026-04-25T10:00:03Z',
      duration_ms: 1234,
      metrics_payload: {},
      has_children: true,
      has_content: false
    };
    const toolCallbackNode = {
      trace_node_id: 'tool_callback:call-refund-policy',
      node_kind: 'tool_callback',
      node_run_id: null,
      node_id: null,
      node_type: 'tool',
      node_mode: 'fusion',
      node_alias: 'refund_policy_lookup',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:02Z',
      finished_at: '2026-04-25T10:00:03Z',
      duration_ms: 1234,
      metrics_payload: {},
      has_children: true,
      has_content: true
    };
    const fusionNode = {
      trace_node_id: 'fusion:call-refund-policy',
      node_kind: 'fusion',
      node_run_id: null,
      node_id: null,
      node_type: 'fusion',
      node_alias: 'refund_policy_lookup',
      status: 'succeeded',
      started_at: '2026-04-25T10:00:02Z',
      finished_at: '2026-04-25T10:00:03Z',
      duration_ms: 1234,
      metrics_payload: {},
      has_children: false,
      has_content: false
    };
    const traceLoader = {
      loadTree: vi.fn().mockResolvedValue({ nodes: [rootNode] }),
      loadChildren: vi
        .fn()
        .mockImplementation(
          async (_runId: string, parentTraceNodeId: string) => ({
            items:
              parentTraceNodeId === rootNode.trace_node_id
                ? [toolsNode]
                : parentTraceNodeId === toolsNode.trace_node_id
                  ? [toolCallbackNode]
                  : parentTraceNodeId === toolCallbackNode.trace_node_id
                    ? [fusionNode]
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
        .mockImplementation(async (_runId: string, traceNodeId: string) => {
          if (traceNodeId === toolCallbackNode.trace_node_id) {
            return {
              trace_node_id: toolCallbackNode.trace_node_id,
              node_kind: 'tool_callback',
              content_kind: 'tool_callback',
              payload: {
                id: 'call-refund-policy',
                name: 'refund_policy_lookup',
                callback_status: 'returned',
                execution_status: 'succeeded',
                request_payload: {
                  arguments: {
                    topic: 'refund'
                  }
                },
                callback_payload: {
                  content: '30 days refund window'
                },
                parsed_result: {
                  content: '30 days refund window'
                },
                duration_ms: 1234
              }
            };
          }

          return {
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
          };
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
            metrics_payload: {},
            debug_payload: {
              provider: 'deepseek'
            },
            started_at: '2026-04-25T10:00:01Z',
            finished_at: '2026-04-25T10:00:05Z'
          }
        }
      }),
      loadToolCallbackDetail: vi.fn().mockResolvedValue({
        id: 'call-refund-policy',
        name: 'refund_policy_lookup',
        callback_status: 'returned',
        execution_status: 'succeeded',
        request_payload: {
          arguments: {
            topic: 'refund'
          }
        },
        callback_payload: {
          content: '30 days refund window'
        },
        parsed_result: {
          content: '30 days refund window'
        },
        request_round_index: 0,
        result_round_index: 1,
        duration_ms: 1234
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
    const llmTraceNode = await screen.findByRole('button', { name: /LLM/ });
    fireEvent.click(llmTraceNode);
    const nodeDetail = await screen.findByRole('region', {
      name: 'LLM 节点详情'
    });
    const toolsButton = await within(nodeDetail).findByRole('button', {
      name: /Tools/
    });
    expect(toolsButton).toHaveAttribute('aria-expanded', 'false');

    expect(traceLoader.loadToolCallbackDetail).not.toHaveBeenCalled();
    expect(traceLoader.loadContent).not.toHaveBeenCalledWith(
      'run-application-log',
      'tool_callback:call-refund-policy'
    );

    fireEvent.click(toolsButton);
    await waitFor(() =>
      expect(traceLoader.loadChildren).toHaveBeenCalledWith(
        'run-application-log',
        'tool_group:node-run-llm',
        undefined
      )
    );
    expect(
      within(nodeDetail).queryByRole('region', {
        name: 'Tools 节点详情'
      })
    ).not.toBeInTheDocument();
    const toolCallback = await within(nodeDetail).findByRole('button', {
      name: /refund_policy_lookup/
    });
    expect(toolCallback).toHaveTextContent('1.23 s');
    expect(toolCallback).toHaveTextContent('fusion');
    const toolMode = within(toolCallback).getByTestId(
      'debug-workflow-node-mode'
    );
    expect(toolMode).toHaveTextContent('fusion');
    expect(toolMode).not.toHaveClass('ant-tag');
    expect(
      within(nodeDetail).queryByRole('region', {
        name: /refund_policy_lookup 节点详情/
      })
    ).not.toBeInTheDocument();

    fireEvent.click(toolCallback);

    await waitFor(() =>
      expect(traceLoader.loadContent).toHaveBeenCalledWith(
        'run-application-log',
        'tool_callback:call-refund-policy'
      )
    );
    const toolDetail = await within(nodeDetail).findByRole('region', {
      name: /refund_policy_lookup 节点详情/
    });
    await waitFor(() => expect(toolCallback).toHaveTextContent('fusion'));
    expect(
      within(toolDetail).queryByRole('button', {
        name: /fusion/
      })
    ).not.toBeInTheDocument();
    expect(within(toolDetail).getByLabelText('输入 JSON')).toHaveTextContent(
      'refund'
    );
    expect(within(toolDetail).getByLabelText('输出 JSON')).toHaveTextContent(
      '30 days refund window'
    );
    expect(traceLoader.loadToolCallbackDetail).not.toHaveBeenCalled();
  }, 10_000);

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

    fireEvent.click(
      await screen.findByRole('button', { name: /续聊上下文/ })
    );
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
    expect(await within(nodeDetail).findByLabelText('输入 JSON')).toHaveTextContent(
      '历史问题'
    );
    expect(await within(nodeDetail).findByLabelText('输出 JSON')).toHaveTextContent(
      '历史回答'
    );
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
