import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import copy from 'copy-to-clipboard';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { DebugAssistantMessage } from '../../components/debug-console/conversation/DebugAssistantMessage';
import type { AgentFlowDebugMessage } from '../../api/runtime';
import { AppProviders } from '../../../../app/AppProviders';

vi.mock('copy-to-clipboard', () => ({
  default: vi.fn()
}));

describe('DebugAssistantMessage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(copy).mockResolvedValue(true);
  });

  test('renders streamed answer content as markdown and shows the current workflow node', () => {
    const message: AgentFlowDebugMessage = {
      id: 'assistant-1',
      role: 'assistant',
      status: 'running',
      runId: 'run-1',
      content: [
        '## 处理结果',
        '',
        '| 项目 | 状态 |',
        '| --- | --- |',
        '| 退款 | 已确认 |'
      ].join('\n'),
      rawOutput: null,
      traceSummary: [
        {
          nodeId: 'node-start',
          nodeAlias: 'Start',
          nodeType: 'start',
          status: 'succeeded',
          startedAt: '2026-04-25T10:00:00Z',
          finishedAt: '2026-04-25T10:00:00Z',
          durationMs: 0,
          inputPayload: {},
          outputPayload: { query: '退款' },
          errorPayload: null,
          metricsPayload: {},
          debugPayload: {}
        },
        {
          nodeId: 'node-llm',
          nodeAlias: 'LLM',
          nodeType: 'llm',
          status: 'running',
          startedAt: '2026-04-25T10:00:01Z',
          finishedAt: null,
          durationMs: null,
          inputPayload: { user_prompt: '退款' },
          outputPayload: { text: '退款处理中' },
          errorPayload: { code: 'still_running' },
          metricsPayload: { total_tokens: 128 },
          debugPayload: {
            response_ref: 'runtime_artifact:inline:response-1'
          }
        }
      ]
    };

    render(<DebugAssistantMessage message={message} />);

    expect(
      screen.getByRole('heading', { name: '处理结果' })
    ).toBeInTheDocument();
    const table = screen.getByRole('table');
    expect(within(table).getByText('退款')).toBeInTheDocument();
    expect(within(table).getByText('已确认')).toBeInTheDocument();
    const actionRow = screen.getByRole('group', { name: '输出动作' });
    expect(
      within(actionRow).getByRole('button', { name: '复制输出' })
    ).toBeInTheDocument();
    expect(within(actionRow).queryByText('复制输出')).not.toBeInTheDocument();
    expect(
      within(actionRow).queryByRole('button', { name: /查看 Trace/ })
    ).not.toBeInTheDocument();
    expect(
      within(actionRow).queryByRole('button', { name: /查看 Raw Output/ })
    ).not.toBeInTheDocument();
    expect(
      table.compareDocumentPosition(actionRow) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(screen.getByRole('region', { name: '工作流' })).toBeInTheDocument();
    expect(screen.queryByText('Assistant')).not.toBeInTheDocument();
    expect(screen.getAllByText('LLM').length).toBeGreaterThan(0);
    expect(screen.getByLabelText('llm 节点类型')).toBeInTheDocument();

    const workflowToggle = screen.getByRole('button', { name: /工作流/ });
    expect(workflowToggle).toHaveAttribute('aria-expanded', 'true');
    expect(
      within(workflowToggle).getByLabelText('running 状态')
    ).toBeInTheDocument();
    expect(
      within(workflowToggle).queryByLabelText('succeeded 状态')
    ).not.toBeInTheDocument();

    fireEvent.click(workflowToggle);

    expect(workflowToggle).toHaveAttribute('aria-expanded', 'false');
    expect(
      screen.queryByRole('button', { name: /LLM/ })
    ).not.toBeInTheDocument();

    fireEvent.click(workflowToggle);

    expect(workflowToggle).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(screen.getByRole('button', { name: /LLM/ }));

    const inputToggle = screen.getByRole('button', { name: '输入' });
    expect(inputToggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('输出')).toBeInTheDocument();
    expect(screen.queryByText('错误')).not.toBeInTheDocument();
    expect(screen.queryByText('指标')).not.toBeInTheDocument();
    expect(screen.queryByText('Debug')).not.toBeInTheDocument();
    expect(screen.getByText(/user_prompt/)).toBeInTheDocument();
    const outputJson = screen.getByLabelText('输出 JSON');
    expect(outputJson).toHaveTextContent('退款处理中');
    expect(outputJson).not.toHaveTextContent('still_running');
    expect(outputJson).not.toHaveTextContent('total_tokens');
    expect(screen.queryByLabelText('错误 JSON')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('指标 JSON')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Debug JSON')).not.toBeInTheDocument();
    expect(screen.getByLabelText('数据处理 JSON')).toHaveTextContent(
      'response_ref'
    );

    fireEvent.click(inputToggle);

    expect(inputToggle).toHaveAttribute('aria-expanded', 'false');
  });

  test('AC-005 renders an arrived answer delta immediately without a client typewriter delay', () => {
    const message: AgentFlowDebugMessage = {
      id: 'assistant-live-delta',
      role: 'assistant',
      status: 'running',
      runId: 'run-1',
      content: '',
      rawOutput: null,
      traceSummary: []
    };
    const { rerender } = render(<DebugAssistantMessage message={message} />);

    rerender(
      <DebugAssistantMessage
        message={{ ...message, content: '已到达的首个输出片段' }}
      />
    );

    expect(screen.getByText('已到达的首个输出片段')).toBeInTheDocument();
  });

  test('AC-003 keeps incomplete streamed Markdown control markers out of the preview', async () => {
    const baseMessage: AgentFlowDebugMessage = {
      id: 'assistant-streaming-markdown',
      role: 'assistant',
      status: 'running',
      runId: 'run-1',
      content: '**处理中',
      rawOutput: null,
      traceSummary: []
    };
    const { container, rerender } = render(
      <DebugAssistantMessage message={baseMessage} />
    );

    await waitFor(() => {
      expect(container).not.toHaveTextContent('**处理中');
    });

    rerender(
      <DebugAssistantMessage
        message={{ ...baseMessage, content: '**处理中**' }}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('处理中').tagName).toBe('STRONG');
    });
  });

  test('loads truncated trace artifact values on explicit action', async () => {
    const onLoadArtifact = vi
      .fn()
      .mockResolvedValue({ text: '完整 Trace 内容' });
    const message: AgentFlowDebugMessage = {
      id: 'assistant-artifact',
      role: 'assistant',
      status: 'completed',
      runId: 'run-1',
      content: '处理完成',
      rawOutput: null,
      traceSummary: [
        {
          nodeId: 'node-llm',
          nodeRunId: 'node-run-llm',
          nodeAlias: 'LLM',
          nodeType: 'llm',
          status: 'succeeded',
          startedAt: '2026-04-25T10:00:00Z',
          finishedAt: '2026-04-25T10:00:01Z',
          durationMs: 1000,
          inputPayload: {},
          outputPayload: {
            text: {
              __runtime_debug_artifact: true,
              is_truncated: true,
              original_size_bytes: 4096,
              preview_size_bytes: 128,
              content_type: 'application/json',
              artifact_ref: 'artifact-1',
              preview: '{"text":"preview'
            }
          },
          errorPayload: null,
          metricsPayload: {},
          debugPayload: {}
        }
      ]
    };

    render(
      <AppProviders>
        <DebugAssistantMessage
          message={message}
          onLoadArtifact={onLoadArtifact}
        />
      </AppProviders>
    );

    fireEvent.click(screen.getByRole('button', { name: /LLM/ }));
    fireEvent.click(screen.getByRole('button', { name: '加载完整值' }));

    expect(onLoadArtifact).toHaveBeenCalledWith('artifact-1');
    await waitFor(() => {
      expect(screen.getByLabelText('输出 JSON')).toHaveTextContent(
        '完整 Trace 内容'
      );
    });
  });

  test('opens the resume timeline for the clicked assistant message', () => {
    const onOpenResumeTimeline = vi.fn();
    const message: AgentFlowDebugMessage = {
      id: 'assistant-resume',
      role: 'assistant',
      status: 'waiting_callback',
      runId: 'conversation-run',
      detailRunId: 'detail-run',
      content: '等待回调',
      rawOutput: null,
      traceSummary: []
    };

    render(
      <DebugAssistantMessage
        message={message}
        onOpenResumeTimeline={onOpenResumeTimeline}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '查看 Resume 时间线' }));

    expect(onOpenResumeTimeline).toHaveBeenCalledWith(message);
  });

  test('renders raw debug payload as data processing for trace items', () => {
    const message: AgentFlowDebugMessage = {
      id: 'assistant-process',
      role: 'assistant',
      status: 'completed',
      runId: 'run-1',
      content: '处理完成',
      rawOutput: null,
      traceSummary: [
        {
          nodeId: 'node-tool',
          nodeRunId: 'node-run-tool',
          nodeAlias: 'Tool',
          nodeType: 'tool',
          status: 'succeeded',
          startedAt: '2026-04-25T10:00:00Z',
          finishedAt: '2026-04-25T10:00:01Z',
          durationMs: 1000,
          inputPayload: { query: '退款' },
          outputPayload: {
            result: 'ok',
            request: {
              url: 'https://example.test/search'
            }
          },
          errorPayload: null,
          metricsPayload: {},
          debugPayload: {
            assistant_message: {
              role: 'assistant',
              content: '内部最终文本'
            },
            provider_route: {
              provider_code: 'openai'
            },
            provider_events: [
              {
                type: 'tool_call_commit',
                name: 'search'
              }
            ]
          }
        }
      ]
    };

    render(<DebugAssistantMessage message={message} />);

    fireEvent.click(screen.getByRole('button', { name: /Tool/ }));

    const processJson = screen.getByLabelText('数据处理 JSON');
    expect(processJson).toHaveTextContent('provider_events');
    expect(processJson).toHaveTextContent('tool_call_commit');
    expect(processJson).toHaveTextContent('assistant_message');
    expect(processJson).toHaveTextContent('provider_route');
    const outputJson = screen.getByLabelText('输出 JSON');
    expect(outputJson).toHaveTextContent('ok');
    expect(outputJson).toHaveTextContent('example.test');
  });

  test('always renders data processing for trace items when debug payload is empty', () => {
    const message: AgentFlowDebugMessage = {
      id: 'assistant-empty-process',
      role: 'assistant',
      status: 'completed',
      runId: 'run-1',
      content: '处理完成',
      rawOutput: null,
      traceSummary: [
        {
          nodeId: 'node-code',
          nodeRunId: 'node-run-code',
          nodeAlias: 'Code',
          nodeType: 'code',
          status: 'succeeded',
          startedAt: '2026-04-25T10:00:00Z',
          finishedAt: '2026-04-25T10:00:01Z',
          durationMs: 1000,
          inputPayload: { value: 1 },
          outputPayload: { value: 2 },
          errorPayload: null,
          metricsPayload: {},
          debugPayload: {}
        }
      ]
    };

    render(<DebugAssistantMessage message={message} />);

    fireEvent.click(screen.getByRole('button', { name: /Code/ }));

    expect(screen.getByLabelText('数据处理 JSON')).toHaveTextContent('{}');
  });

  test('renders running streamed content immediately without typewriter delay', () => {
    vi.useFakeTimers();
    const baseMessage: AgentFlowDebugMessage = {
      id: 'assistant-typing',
      role: 'assistant',
      status: 'running',
      runId: 'run-1',
      content: '',
      rawOutput: null,
      traceSummary: []
    };

    const { container, rerender } = render(
      <DebugAssistantMessage message={baseMessage} />
    );

    rerender(
      <DebugAssistantMessage message={{ ...baseMessage, content: 'abcdef' }} />
    );

    expect(container).toHaveTextContent('abcdef');
    vi.useRealTimers();
  });

  test('reveals completed assistant content progressively', () => {
    vi.useFakeTimers();
    const baseMessage: AgentFlowDebugMessage = {
      id: 'assistant-typing-completed',
      role: 'assistant',
      status: 'completed',
      runId: 'run-1',
      content: '',
      rawOutput: null,
      traceSummary: []
    };

    const { container, rerender } = render(
      <DebugAssistantMessage message={baseMessage} />
    );

    rerender(
      <DebugAssistantMessage message={{ ...baseMessage, content: 'abcdef' }} />
    );

    expect(container).not.toHaveTextContent('abcdef');

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(container).toHaveTextContent('abcdef');
    vi.useRealTimers();
  });

  test('renders Dify-style think tags in a collapsible labeled section', () => {
    const message: AgentFlowDebugMessage = {
      id: 'assistant-reasoning',
      role: 'assistant',
      status: 'running',
      runId: 'run-1',
      content: '<think>先分析用户问题，再整理退款政策。</think>退款政策摘要。',
      rawOutput: null,
      traceSummary: []
    };

    render(<DebugAssistantMessage message={message} />);

    const reasoningToggle = screen.getByRole('button', { name: /思考/ });
    expect(reasoningToggle).toHaveAttribute('aria-expanded', 'true');
    expect(
      screen.getByText('先分析用户问题，再整理退款政策。')
    ).toBeInTheDocument();
    expect(screen.getByText('退款政策摘要。')).toBeInTheDocument();
    expect(screen.queryByText(/<think>/)).not.toBeInTheDocument();

    fireEvent.click(reasoningToggle);

    expect(reasoningToggle).toHaveAttribute('aria-expanded', 'false');
    expect(
      screen.queryByText('先分析用户问题，再整理退款政策。')
    ).not.toBeInTheDocument();
    expect(screen.getByText('退款政策摘要。')).toBeInTheDocument();
  });

  test('copies answer content through App message context without static message warning', async () => {
    const warnSpy = vi
      .spyOn(console, 'warn')
      .mockImplementation(() => undefined);
    const errorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const message: AgentFlowDebugMessage = {
      id: 'assistant-copy',
      role: 'assistant',
      status: 'completed',
      runId: 'run-1',
      content: '复制这段输出',
      rawOutput: null,
      traceSummary: []
    };

    render(
      <AppProviders>
        <DebugAssistantMessage message={message} />
      </AppProviders>
    );

    fireEvent.click(screen.getByRole('button', { name: '复制输出' }));

    await waitFor(() => {
      expect(copy).toHaveBeenCalledWith('复制这段输出');
    });
    expect(
      [...warnSpy.mock.calls, ...errorSpy.mock.calls].flat().join('\n')
    ).not.toContain('Static function can not consume context');

    warnSpy.mockRestore();
    errorSpy.mockRestore();
  });
});
