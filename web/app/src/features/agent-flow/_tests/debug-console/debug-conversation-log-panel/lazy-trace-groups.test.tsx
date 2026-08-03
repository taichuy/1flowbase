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
        .mockImplementation(async (_runId: string, traceNodeId: string) => ({
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
        }))
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
});
