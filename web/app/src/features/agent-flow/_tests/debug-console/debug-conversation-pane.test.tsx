import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type {
  AgentFlowDebugMessage,
  AgentFlowRunContext
} from '../../api/runtime';
import { DebugConversationPane } from '../../components/debug-console/conversation/DebugConversationPane';

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

  test('renders LLM tool callbacks under the automatically expanded Tools child node', () => {
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
    expect(
      screen.getByRole('button', { name: /lookup_weather/ })
    ).toBeInTheDocument();
    expect(screen.queryByText('call_weather')).not.toBeInTheDocument();
  });

  test('collapses repeated LLM node runs into one workflow row', () => {
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

    expect(screen.getAllByTestId('debug-workflow-node-row')).toHaveLength(2);

    const llmTraceNode = screen.getAllByTestId('debug-workflow-node-row')[1];
    expect(llmTraceNode).toHaveTextContent('工具 2');

    const toolsNode = screen.getByRole('button', {
      name: /^工具 2 次工具回调$/
    });
    expect(toolsNode).toHaveAttribute('aria-expanded', 'true');

    expect(
      screen.queryByLabelText('工具回调索引 JSON')
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /lookup_weather/ })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /read_policy/ })
    ).toBeInTheDocument();
    expect(screen.queryByText('call_weather')).not.toBeInTheDocument();
    expect(screen.queryByText('call_policy')).not.toBeInTheDocument();
  });
});
