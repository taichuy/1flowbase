import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../api/runtime';
import { DebugConversationPane } from '../../components/debug-console/conversation/DebugConversationPane';
import { LlmToolTraceTree } from '../../components/debug-console/conversation/LlmToolTraceTree';

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
      value: '你好'
    }
  ]
};

beforeEach(() => {
  vi.stubGlobal(
    'IntersectionObserver',
    class IntersectionObserver {
      disconnect() {}
      observe() {}
      takeRecords() {
        return [];
      }
      unobserve() {}
    }
  );
});

function assistantMessage(content: string): AgentFlowDebugMessage {
  return {
    id: 'assistant-1',
    role: 'assistant',
    status: 'running',
    runId: 'run-1',
    content,
    rawOutput: null,
    traceSummary: []
  };
}

function renderPane(messages: AgentFlowDebugMessage[]) {
  return render(
    <DebugConversationPane
      messages={messages}
      runContext={runContext}
      status="running"
      stopping={false}
      onChangeQuery={vi.fn()}
      onStopRun={vi.fn()}
      onSubmitPrompt={vi.fn()}
    />
  );
}

function configureScrollMetrics(element: HTMLElement) {
  Object.defineProperty(element, 'clientHeight', {
    configurable: true,
    value: 120
  });
  Object.defineProperty(element, 'scrollHeight', {
    configurable: true,
    value: 360
  });
}

describe('DebugConversationPane auto scroll', () => {
  test('keeps streamed output pinned to the bottom until the user scrolls', () => {
    const { rerender } = renderPane([assistantMessage('你好')]);
    const messagesElement = screen.getByTestId('debug-conversation-messages');
    configureScrollMetrics(messagesElement);

    rerender(
      <DebugConversationPane
        messages={[assistantMessage('你好，正在输出更多内容')]}
        runContext={runContext}
        status="running"
        stopping={false}
        onChangeQuery={vi.fn()}
        onStopRun={vi.fn()}
        onSubmitPrompt={vi.fn()}
      />
    );

    expect(messagesElement.scrollTop).toBe(360);

    messagesElement.scrollTop = 40;
    messagesElement.dispatchEvent(new Event('scroll', { bubbles: true }));

    rerender(
      <DebugConversationPane
        messages={[assistantMessage('你好，正在输出更多内容，继续追加')]}
        runContext={runContext}
        status="running"
        stopping={false}
        onChangeQuery={vi.fn()}
        onStopRun={vi.fn()}
        onSubmitPrompt={vi.fn()}
      />
    );

    expect(messagesElement.scrollTop).toBe(360);

    messagesElement.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    messagesElement.scrollTop = 40;

    rerender(
      <DebugConversationPane
        messages={[
          assistantMessage('你好，正在输出更多内容，继续追加，暂停后追加')
        ]}
        runContext={runContext}
        status="running"
        stopping={false}
        onChangeQuery={vi.fn()}
        onStopRun={vi.fn()}
        onSubmitPrompt={vi.fn()}
      />
    );

    expect(messagesElement.scrollTop).toBe(40);
  });
});

describe('DebugConversationPane workflow trace', () => {
  test('shows a running workflow node before the first answer delta arrives', () => {
    renderPane([
      {
        ...assistantMessage(''),
        traceSummary: [
          {
            nodeId: 'node-llm',
            nodeRunId: 'node-run-llm',
            nodeAlias: 'LLM',
            nodeType: 'llm',
            status: 'running',
            startedAt: '2026-08-06T10:00:00Z',
            finishedAt: null,
            durationMs: null,
            inputPayload: { prompt: '你好' },
            outputPayload: {},
            errorPayload: null,
            metricsPayload: {},
            debugPayload: {}
          }
        ]
      }
    ]);

    expect(screen.getByText('工作流')).toBeInTheDocument();
    expect(screen.getByTestId('debug-workflow-node-row')).toHaveTextContent(
      'LLM'
    );
  });

  test('expands the LLM tool callback list while keeping callback details collapsed', () => {
    renderPane([
      {
        ...assistantMessage('等待工具结果'),
        status: 'waiting_callback',
        traceSummary: [
          {
            nodeId: 'node-llm',
            nodeRunId: 'node-run-llm',
            nodeAlias: 'LLM',
            nodeType: 'llm',
            status: 'waiting_callback',
            startedAt: '2026-04-25T10:00:01Z',
            finishedAt: null,
            durationMs: null,
            inputPayload: {
              prompt: '天气?'
            },
            outputPayload: {
              tool_calls: [
                {
                  id: 'call_weather',
                  name: 'lookup_weather'
                }
              ]
            },
            errorPayload: null,
            metricsPayload: {},
            debugPayload: {
              llm_rounds: [
                {
                  round_index: 0,
                  assistant: {
                    role: 'assistant',
                    content: 'need tool',
                    tool_calls: [
                      {
                        id: 'call_weather',
                        name: 'lookup_weather'
                      }
                    ]
                  },
                  finish_reason: 'tool_call'
                }
              ]
            }
          }
        ]
      }
    ]);

    expect(screen.getByText('工作流')).toBeInTheDocument();
    expect(screen.queryByText('Round #1')).not.toBeInTheDocument();

    const toolsNode = screen.getByRole('button', {
      name: /^工具 1 次工具回调$/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');
    expect(
      screen.queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    const toolCallback = screen.getByRole('button', {
      name: /lookup_weather/
    });
    expect(toolCallback).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('call_weather')).not.toBeInTheDocument();

    fireEvent.click(toolCallback);

    expect(toolCallback).toHaveAttribute('aria-expanded', 'true');
  });

  test('opens the tool callback list when a streaming LLM node receives its first callback', () => {
    const { rerender } = render(
      <LlmToolTraceTree debugPayload={{}} defaultToolsExpanded={false} />
    );

    expect(
      screen.queryByRole('button', { name: /^工具 1 次工具回调$/ })
    ).not.toBeInTheDocument();

    rerender(
      <LlmToolTraceTree
        debugPayload={{
          llm_rounds: [
            {
              round_index: 0,
              assistant: {
                role: 'assistant',
                content: 'need tool',
                tool_calls: [
                  {
                    id: 'call_weather',
                    name: 'lookup_weather'
                  }
                ]
              },
              finish_reason: 'tool_call'
            }
          ]
        }}
        defaultToolsExpanded
      />
    );

    expect(
      screen.getByRole('button', { name: /^工具 1 次工具回调$/ })
    ).toHaveAttribute('aria-expanded', 'true');
    expect(
      screen.getByRole('button', { name: /lookup_weather/ })
    ).toHaveAttribute('aria-expanded', 'false');
  });

  test('keeps distinct LLM node runs and their own tool callbacks', () => {
    renderPane([
      {
        ...assistantMessage('等待工具结果'),
        status: 'waiting_callback',
        traceSummary: [
          {
            nodeId: 'node-start',
            nodeRunId: 'node-run-start',
            nodeAlias: 'Start',
            nodeType: 'start',
            status: 'succeeded',
            startedAt: '2026-04-25T10:00:00Z',
            finishedAt: '2026-04-25T10:00:00Z',
            durationMs: 80,
            inputPayload: { query: '天气?' },
            outputPayload: { query: '天气?' },
            errorPayload: null,
            metricsPayload: {},
            debugPayload: {}
          },
          {
            nodeId: 'node-llm',
            nodeRunId: 'node-run-llm-1',
            nodeAlias: 'LLM',
            nodeType: 'llm',
            status: 'succeeded',
            startedAt: '2026-04-25T10:00:01Z',
            finishedAt: '2026-04-25T10:00:06Z',
            durationMs: 5400,
            inputPayload: { prompt: '天气?' },
            outputPayload: { usage: { total_tokens: 8035 } },
            errorPayload: null,
            metricsPayload: {},
            debugPayload: {
              llm_rounds: [
                {
                  round_index: 0,
                  assistant: {
                    role: 'assistant',
                    content: 'need weather',
                    tool_calls: [
                      {
                        id: 'call_weather',
                        name: 'lookup_weather'
                      }
                    ]
                  }
                }
              ]
            }
          },
          {
            nodeId: 'node-llm',
            nodeRunId: 'node-run-llm-2',
            nodeAlias: 'LLM',
            nodeType: 'llm',
            status: 'waiting_callback',
            startedAt: '2026-04-25T10:00:07Z',
            finishedAt: null,
            durationMs: null,
            inputPayload: { prompt: '天气?' },
            outputPayload: { tool_calls: [{ id: 'call_policy' }] },
            errorPayload: null,
            metricsPayload: {},
            debugPayload: {
              llm_rounds: [
                {
                  round_index: 0,
                  assistant: {
                    role: 'assistant',
                    content: 'need policy',
                    tool_calls: [
                      {
                        id: 'call_policy',
                        name: 'read_policy'
                      }
                    ]
                  }
                }
              ]
            }
          }
        ]
      }
    ]);

    const rows = screen.getAllByTestId('debug-workflow-node-row');
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent('用户输入');
    expect(rows[1]).toHaveTextContent('工具 1');
    expect(rows[2]).toHaveTextContent('工具 1');
    const toolsNodes = screen.getAllByRole('button', {
      name: /^工具 1 次工具回调$/
    });
    expect(toolsNodes).toHaveLength(2);
    toolsNodes.forEach((node) => expect(node).toHaveAttribute('aria-expanded', 'true'));

    expect(
      screen.queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /lookup_weather/ })
    ).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByRole('button', { name: /read_policy/ })).toHaveAttribute(
      'aria-expanded',
      'false'
    );
    expect(screen.queryByText('call_weather')).not.toBeInTheDocument();
    expect(screen.queryByText('call_policy')).not.toBeInTheDocument();
  });
});

describe('DebugConversationPane log access before an assistant answer', () => {
  const user: AgentFlowDebugMessage = {
    id: 'pending-user',
    role: 'user',
    status: 'waiting_callback',
    runId: 'task-run',
    detailRunId: 'actual-detail-run',
    canOpenDetail: true,
    content: '请继续执行工具',
    rawOutput: null,
    traceSummary: []
  };

  function renderLogs(
    messages: AgentFlowDebugMessage[],
    logActionRunId?: string
  ) {
    const onOpenMessageLog = vi.fn();
    render(
      <DebugConversationPane
        messages={messages}
        runContext={runContext}
        status="waiting_callback"
        stopping={false}
        showComposer={false}
        logActionRunId={logActionRunId}
        onChangeQuery={vi.fn()}
        onStopRun={vi.fn()}
        onSubmitPrompt={vi.fn()}
        onOpenMessageLog={onOpenMessageLog}
      />
    );
    return onOpenMessageLog;
  }

  test('opens the backend-enabled detail from a pending user-only turn without inventing an answer', () => {
    const openLog = renderLogs([user]);
    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    expect(openLog).toHaveBeenCalledWith(user);
    expect(screen.getByText(user.content)).toBeInTheDocument();
    expect(
      document.querySelector('.agent-flow-editor__debug-message--assistant')
    ).toBeNull();
  });

  test.each([
    { ...user, canOpenDetail: false },
    { ...user, canOpenDetail: undefined },
    { ...user, detailRunId: null, runId: null }
  ])(
    'does not create an entry without an explicit enabled detail identity',
    (message) => {
      renderLogs([message]);
      expect(
        screen.queryByRole('button', { name: '查看对话日志' })
      ).not.toBeInTheDocument();
    }
  );

  test('keeps the existing single assistant entry when the answer is displayed', () => {
    const answer = {
      ...assistantMessage('已完成'),
      detailRunId: user.detailRunId
    };
    const openLog = renderLogs([user, answer]);
    expect(
      screen.getAllByRole('button', { name: '查看对话日志' })
    ).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: '查看对话日志' }));
    expect(openLog).toHaveBeenCalledWith(answer);
  });

  test('respects the selected log run scope', () => {
    renderLogs([user], 'different-run');
    expect(
      screen.queryByRole('button', { name: '查看对话日志' })
    ).not.toBeInTheDocument();
  });
});
