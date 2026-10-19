import {
  setupEmbeddedAssistantFixture,
  renderSettled,
  domEvents,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantRunActivity,
  startConsoleAssistantRunWebSocket,
  findReadyHistoryButton,
  rerenderSettled
} from './assistant-fixture';
import { act, screen, within, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import type { ConsoleFlowDebugStreamEvent } from '@1flowbase/api-client';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { AssistantRunTimeline } from '../../components/embedded-assistant/AssistantRunActivityPanel';
import * as runtimeApi from '../../api/runtime';
import type { AgentFlowDebugMessage } from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';

describe('EmbeddedAgentAssistant', () => {
  setupEmbeddedAssistantFixture();

  test('AC-008 waits for the first activity instead of flashing an empty assistant card', async () => {
    const { container } = await renderSettled(
      <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={{
            id: 'run-waiting:assistant',
            role: 'assistant',
            content: '',
            status: 'running',
            runId: 'run-waiting',
            rawOutput: null,
            traceSummary: [],
            presentation: 'answer'
          }}
        />
      </AppProviders>
    );

    await waitFor(() =>
      expect(getConsoleAssistantRunActivity).toHaveBeenCalledWith(
        'flow-1',
        'run-waiting',
        { pageSize: 500 }
      )
    );
    expect(
      screen.queryByText(i18nText('appShell', 'auto.assistant_activity_empty'))
    ).not.toBeInTheDocument();
    expect(
      container.querySelector('.embedded-agent-assistant-activity')
    ).toBeNull();
  });

  test('AC-009 expands only the active reasoning segment and preserves manual collapse across deltas', async () => {
    const firstReasoning = {
      type: 'reasoning_delta' as const,
      run_id: 'run-reasoning-lifecycle',
      node_run_id: 'node-run-llm',
      node_id: 'node-llm',
      text: '第一轮思考',
      event_id: 'run-reasoning-lifecycle:1',
      sequence: 1
    };
    const toolStarted = {
      type: 'assistant_tool_call_started' as const,
      run_id: 'run-reasoning-lifecycle',
      node_run_id: 'node-run-llm',
      node_id: 'node-llm',
      tool_call: {
        id: 'call-lifecycle',
        name: 'lookup',
        arguments: {}
      },
      event_id: 'run-reasoning-lifecycle:2',
      sequence: 2
    };
    const secondReasoning = {
      ...firstReasoning,
      text: '第二轮思考',
      event_id: 'run-reasoning-lifecycle:3',
      sequence: 3
    };
    const secondReasoningDelta = {
      ...secondReasoning,
      text: '继续',
      event_id: 'run-reasoning-lifecycle:4',
      sequence: 4
    };
    const message = (
      activityEvents: ConsoleFlowDebugStreamEvent[]
    ): AgentFlowDebugMessage => ({
      id: 'run-reasoning-lifecycle:assistant',
      role: 'assistant' as const,
      content: '',
      status: 'running',
      runId: 'run-reasoning-lifecycle',
      rawOutput: null,
      traceSummary: [],
      activityEvents,
      presentation: 'answer' as const
    });
    const { rerender } = await renderSettled(
      <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={message([firstReasoning])}
        />
      </AppProviders>
    );

    expect(await screen.findByText('第一轮思考')).toBeInTheDocument();

    await rerenderSettled(rerender, <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={message([firstReasoning, toolStarted])}
        />
      </AppProviders>);
    await waitFor(() =>
      expect(screen.queryByText('第一轮思考')).not.toBeInTheDocument()
    );

    await rerenderSettled(rerender, <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={message([firstReasoning, toolStarted, secondReasoning])}
        />
      </AppProviders>);
    expect(await screen.findByText('第二轮思考')).toBeInTheDocument();
    const reasoningTitles = screen.getAllByText(
      i18nText('agentFlow', 'auto.think')
    );
    await domEvents.click(reasoningTitles[1] as HTMLElement);
    await waitFor(() =>
      expect(screen.queryByText('第二轮思考')).not.toBeInTheDocument()
    );

    await rerenderSettled(rerender, <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={message([
            firstReasoning,
            toolStarted,
            secondReasoning,
            secondReasoningDelta
          ])}
        />
      </AppProviders>);
    expect(screen.queryByText('第二轮思考继续')).not.toBeInTheDocument();
  });

  test('AC-010 preserves sequence_start order when terminal durable activity replaces live events', async () => {
    getConsoleAssistantRunActivity
      .mockResolvedValueOnce({
        status: 'running',
        started_at: '2026-08-16T00:00:00Z',
        finished_at: null,
        duration_ms: null,
        items: [],
        trace_events: [],
        has_more: false,
        next_sequence: null
      })
      .mockResolvedValue({
        status: 'succeeded',
        started_at: '2026-08-16T00:00:00Z',
        finished_at: '2026-08-16T00:00:02Z',
        duration_ms: 2000,
        items: [
          {
            kind: 'tool',
            event_id: 'run-terminal-order:4',
            sequence_start: 4,
            sequence_end: 5,
            created_at: '2026-08-16T00:00:01Z',
            tool_call_id: 'call-order',
            tool_name: 'lookup',
            input: {},
            output: { ok: true },
            duration_ms: 5,
            is_error: false,
            status: 'succeeded'
          },
          {
            kind: 'output',
            event_id: 'run-terminal-order:6',
            sequence_start: 6,
            sequence_end: 6,
            created_at: '2026-08-16T00:00:02Z',
            text: '最终回答',
            segment_index: 0
          },
          {
            kind: 'reasoning',
            event_id: 'run-terminal-order:3',
            sequence_start: 3,
            sequence_end: 3,
            created_at: '2026-08-16T00:00:00Z',
            text: '先思考'
          }
        ],
        trace_events: [],
        has_more: false,
        next_sequence: null
      });
    const runningMessage: AgentFlowDebugMessage = {
      id: 'run-terminal-order:assistant',
      role: 'assistant' as const,
      content: '',
      status: 'running',
      runId: 'run-terminal-order',
      rawOutput: null,
      traceSummary: [],
      activityEvents: [
        {
          type: 'reasoning_delta' as const,
          run_id: 'run-terminal-order',
          node_id: 'node-llm',
          text: '先思考',
          event_id: 'run-terminal-order:3',
          sequence: 3
        }
      ],
      presentation: 'answer' as const
    };
    const { rerender } = await renderSettled(
      <AppProviders>
        <AssistantRunTimeline applicationId="flow-1" message={runningMessage} />
      </AppProviders>
    );
    expect(await screen.findByText('先思考')).toBeInTheDocument();

    await rerenderSettled(rerender, <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={{ ...runningMessage, status: 'completed' }}
        />
      </AppProviders>);
    const duration = await screen.findByText(
      i18nText('appShell', 'auto.assistant_activity_duration_seconds', {
        value1: 2
      })
    );
    await domEvents.click(duration);
    const thinkTitle = await screen.findByText(
      i18nText('agentFlow', 'auto.think')
    );
    expect(screen.queryByText('先思考')).not.toBeInTheDocument();
    await domEvents.click(thinkTitle);
    const reasoning = await screen.findByText('先思考');
    const tool = screen.getByText('lookup');
    expect(reasoning.compareDocumentPosition(tool)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(tool.compareDocumentPosition(screen.getByText('最终回答'))).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
  });

  test('AC-001 identifies list, get, and call activity by their canonical locator', async () => {
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'succeeded',
      started_at: '2026-08-16T00:00:00Z',
      finished_at: '2026-08-16T00:00:01Z',
      duration_ms: 1000,
      items: [
        {
          kind: 'tool',
          event_id: 'tool-list',
          sequence_start: 1,
          sequence_end: 1,
          created_at: '2026-08-16T00:00:00Z',
          tool_call_id: 'call-list',
          tool_name: '1flowbase_mcp_list',
          input: { path: '/frontstage/user_pages' },
          output: {},
          duration_ms: 1,
          is_error: false,
          status: 'succeeded'
        },
        {
          kind: 'tool',
          event_id: 'tool-get',
          sequence_start: 2,
          sequence_end: 2,
          created_at: '2026-08-16T00:00:00Z',
          tool_call_id: 'call-get',
          tool_name: '1flowbase_mcp_get',
          input: { tool_id: 'frontstage_block_tree_get_code' },
          output: {},
          duration_ms: 1,
          is_error: false,
          status: 'succeeded'
        },
        {
          kind: 'tool',
          event_id: 'tool-call',
          sequence_start: 3,
          sequence_end: 3,
          created_at: '2026-08-16T00:00:00Z',
          tool_call_id: 'call-call',
          tool_name: '1flowbase_mcp_call',
          input: { tool_id: 'frontstage_block_tree_get_code', arguments: {} },
          output: {},
          duration_ms: 1,
          is_error: false,
          status: 'succeeded'
        }
      ],
      trace_events: [],
      has_more: false,
      next_sequence: null
    });

    await renderSettled(
      <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={{
            id: 'tool-labels',
            role: 'assistant',
            content: '',
            status: 'completed',
            runId: 'run-tool-labels',
            rawOutput: null,
            traceSummary: [],
            presentation: 'answer'
          }}
        />
      </AppProviders>
    );

    expect(
      await screen.findByText('1flowbase_mcp_list (/frontstage/user_pages)')
    ).toBeInTheDocument();
    expect(
      screen.getByText('1flowbase_mcp_get (frontstage_block_tree_get_code)')
    ).toBeInTheDocument();
    expect(
      screen.getByText('1flowbase_mcp_call (frontstage_block_tree_get_code)')
    ).toBeInTheDocument();

    await domEvents.click(
      screen.getByText('1flowbase_mcp_list (/frontstage/user_pages)')
    );
    expect(
      await screen.findByText(i18nText('agentFlow', 'auto.input'))
    ).toBeVisible();
    expect(
      screen.getByText(i18nText('appShell', 'auto.assistant_activity_output'))
    ).toBeVisible();
  });

  test('AC-006 keeps ordered assistant activity inline and restores every node card in the sidebar', async () => {
    let finishRun: (() => void) | undefined;
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-activity',
          status: 'queued',
          event_id: 'run-activity:1',
          sequence: 1
        });
        handlers.onEvent({
          type: 'node_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          node_type: 'llm',
          title: 'Research node',
          event_id: 'run-activity:2',
          sequence: 2
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          text: '先检查配置',
          event_id: 'run-activity:3',
          sequence: 3,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'assistant_tool_call_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-1',
            name: '1flowbase_mcp_list',
            arguments: { path: '/后台设置' }
          },
          event_id: 'run-activity:4',
          sequence: 4
        });
        handlers.onEvent({
          type: 'assistant_tool_call_finished',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-1',
            name: '1flowbase_mcp_list',
            arguments: { path: '/后台设置' }
          },
          tool_result: { count: 2 },
          duration_ms: 4,
          event_id: 'run-activity:5',
          sequence: 5
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-activity',
          node_id: 'node-answer',
          text: '阶段结果一',
          event_id: 'run-activity:6',
          sequence: 6,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          text: '继续检查数据',
          event_id: 'run-activity:7',
          sequence: 7,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 1
          }
        });
        handlers.onEvent({
          type: 'assistant_tool_call_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-2',
            name: '1flowbase_mcp_get',
            arguments: { tool_id: 'frontstage_block_tree_get_code' }
          },
          event_id: 'run-activity:8',
          sequence: 8
        });
        handlers.onEvent({
          type: 'assistant_tool_call_finished',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-2',
            name: '1flowbase_mcp_get',
            arguments: { tool_id: 'frontstage_block_tree_get_code' }
          },
          tool_result: { id: 'item-1' },
          duration_ms: 3,
          event_id: 'run-activity:9',
          sequence: 9
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-activity',
          node_id: 'node-answer',
          text: '阶段结果二',
          event_id: 'run-activity:10',
          sequence: 10,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 1
          }
        });
        await new Promise<void>((resolve) => {
          finishRun = () => {
            handlers.onEvent({
              type: 'node_finished',
              run_id: 'run-activity',
              node_run_id: 'node-run-llm',
              node_id: 'node-llm',
              status: 'succeeded',
              event_id: 'run-activity:11',
              sequence: 11
            });
            handlers.onEvent({
              type: 'node_started',
              run_id: 'run-activity',
              node_run_id: 'node-run-answer',
              node_id: 'node-answer',
              node_type: 'answer',
              title: 'Answer node',
              event_id: 'run-activity:12',
              sequence: 12
            });
            handlers.onEvent({
              type: 'node_finished',
              run_id: 'run-activity',
              node_run_id: 'node-run-answer',
              node_id: 'node-answer',
              status: 'succeeded',
              event_id: 'run-activity:13',
              sequence: 13
            });
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-activity',
              status: 'succeeded',
              output: { answer: '阶段结果一阶段结果二' },
              event_id: 'run-activity:14',
              sequence: 14
            });
            resolve();
          };
        });
      }
    );

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );

    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    await domEvents.change(composer, { target: { value: 'Inspect activity order' } });
    await domEvents.click(sendButton);

    const reasoningTitles = await screen.findAllByText(
      i18nText('agentFlow', 'auto.think')
    );
    expect(reasoningTitles).toHaveLength(2);
    reasoningTitles.forEach((title) => {
      expect(title.closest('.ant-think')).not.toBeNull();
    });
    await domEvents.click(reasoningTitles[0] as HTMLElement);
    await domEvents.click(reasoningTitles[1] as HTMLElement);
    const firstReasoning = await screen.findByText('先检查配置');
    const firstTool = screen.getByText('1flowbase_mcp_list (/后台设置)');
    const firstOutput = screen.getByText('阶段结果一');
    const secondReasoning = screen.getByText('继续检查数据');
    const secondTool = screen.getByText(
      '1flowbase_mcp_get (frontstage_block_tree_get_code)'
    );
    const secondOutput = screen.getByText('阶段结果二');
    expect(firstReasoning.compareDocumentPosition(firstTool)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(firstTool.compareDocumentPosition(firstOutput)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(firstOutput.compareDocumentPosition(secondReasoning)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(secondReasoning.compareDocumentPosition(secondTool)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(secondTool.compareDocumentPosition(secondOutput)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    await domEvents.click(firstTool);
    const toolTimelineItem = firstTool.closest(
      '.embedded-agent-assistant-activity__timeline-item'
    );
    expect(toolTimelineItem).toHaveStyle({
      display: 'grid',
      gridTemplateColumns: 'auto minmax(0, 1fr)'
    });
    const inputTitle = i18nText('agentFlow', 'auto.input');
    const outputTitle = i18nText('appShell', 'auto.assistant_activity_output');
    const inputJson = screen.getByLabelText(`${inputTitle} JSON`);
    const toolDetail = inputJson.closest(
      '.embedded-agent-assistant-activity__tool-detail'
    );
    expect(toolDetail).not.toBeNull();
    expect(
      within(toolDetail as HTMLElement).getByText(inputTitle)
    ).toBeInTheDocument();
    expect(
      within(toolDetail as HTMLElement).getByText(outputTitle)
    ).toBeInTheDocument();
    expect(inputJson).toHaveTextContent('"path": "/后台设置"');
    expect(screen.getByLabelText(`${outputTitle} JSON`)).toHaveTextContent(
      '"count": 2'
    );
    expect(screen.queryByTestId('mock-json-editor')).not.toBeInTheDocument();
    await domEvents.click(screen.getByRole('button', { name: inputTitle }));
    expect(await screen.findByTestId('mock-json-editor')).toHaveTextContent(
      '"path": "/后台设置"'
    );
    await domEvents.click(screen.getByRole('button', { name: outputTitle }));
    expect(await screen.findAllByTestId('mock-json-editor')).toHaveLength(2);

    await act(async () => {
      finishRun?.();
    });
    expect(await screen.findByText('阶段结果一')).toBeInTheDocument();
    expect(screen.getByText('阶段结果二')).toBeInTheDocument();
    const terminalReasoningTitles = await screen.findAllByText(
      i18nText('agentFlow', 'auto.think')
    );
    expect(terminalReasoningTitles).toHaveLength(2);
    terminalReasoningTitles.forEach((title) => {
      expect(title.closest('.ant-think')).not.toBeNull();
    });
    expect(screen.queryByText('先检查配置')).not.toBeInTheDocument();
    await domEvents.click(terminalReasoningTitles[0] as HTMLElement);
    const terminalFirstReasoning = await screen.findByText('先检查配置');
    const terminalFirstTool = screen.getByText(
      '1flowbase_mcp_list (/后台设置)'
    );
    const terminalFirstOutput = screen.getByText('阶段结果一');
    expect(
      terminalFirstReasoning.compareDocumentPosition(terminalFirstTool)
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(terminalFirstTool.compareDocumentPosition(terminalFirstOutput)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(
      screen.queryByLabelText(i18nText('agentFlow', 'auto.workflow'))
    ).not.toBeInTheDocument();

    const activityButtons = screen.getAllByRole('button', {
      name: i18nText('appShell', 'auto.assistant_activity')
    });
    await domEvents.click(activityButtons.at(-1) as HTMLElement);

    const sidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    expect(sidebar).toHaveAttribute(
      'aria-label',
      i18nText('appShell', 'auto.assistant_activity')
    );
    expect(
      await within(sidebar).findByText('Research node')
    ).toBeInTheDocument();
    expect(
      within(sidebar).getByText(i18nText('agentFlow', 'auto.reply_directly'))
    ).toBeInTheDocument();
    expect(
      within(sidebar).getByLabelText(i18nText('agentFlow', 'auto.workflow'))
    ).toBeInTheDocument();
    expect(within(sidebar).queryByText('先检查配置')).not.toBeInTheDocument();
    expect(within(sidebar).queryByText('继续检查数据')).not.toBeInTheDocument();
  });

  test('AC-007 replays the same run activity projection for restored history', async () => {
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockResolvedValue({
      flow_run: { status: 'succeeded' },
      node_runs: [
        {
          id: 'node-run-history',
          node_id: 'node-history',
          node_type: 'llm',
          node_alias: 'History node',
          status: 'succeeded',
          input_payload: {},
          output_payload: {},
          error_payload: null,
          metrics_payload: {
            usage: { total_tokens: 17793 }
          },
          debug_payload: {
            llm_rounds: {
              tool_callbacks: [
                {
                  id: 'call-history',
                  name: '1flowbase_mcp_list',
                  callback_status: 'returned',
                  execution_status: 'succeeded',
                  call_usage: { total_tokens: 1460 },
                  duration_ms: 61
                }
              ]
            }
          },
          started_at: '2026-08-15T00:00:00Z',
          finished_at: '2026-08-15T00:00:03Z'
        }
      ],
      checkpoints: [],
      callback_tasks: [],
      events: []
    } as never);
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-history:assistant',
        flow_run_id: 'run-history',
        role: 'assistant',
        content: '历史最终回答',
        status: 'succeeded',
        page_references: [],
        created_at: '2026-08-15T00:00:00Z'
      }
    ]);
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'succeeded',
      started_at: '2026-08-15T00:00:00Z',
      finished_at: '2026-08-15T00:00:03Z',
      duration_ms: 3000,
      items: [
        {
          kind: 'reasoning',
          event_id: 'run-history:3',
          sequence_start: 3,
          sequence_end: 3,
          created_at: '2026-08-15T00:00:01Z',
          text: '历史思考'
        },
        {
          kind: 'output',
          event_id: 'run-history:4',
          sequence_start: 4,
          sequence_end: 4,
          created_at: '2026-08-15T00:00:02Z',
          text: '历史最终回答',
          segment_index: 0
        }
      ],
      trace_events: [
        {
          event_id: 'run-history:1',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'node_started',
          sequence: 1,
          created_at: '2026-08-15T00:00:00Z',
          payload: {
            node_id: 'node-history',
            node_type: 'llm',
            title: 'History node'
          },
          delta_index: null,
          content_type: null,
          text: null
        },
        {
          event_id: 'run-history:3',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'reasoning_delta',
          sequence: 3,
          created_at: '2026-08-15T00:00:01Z',
          payload: {
            node_id: 'node-history',
            text: '历史思考',
            presentation: {
              kind: 'answer',
              answer_node_id: 'node-answer',
              segment_index: 0
            }
          },
          delta_index: 3,
          content_type: 'reasoning',
          text: '历史思考'
        },
        {
          event_id: 'run-history:4',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'text_delta',
          sequence: 4,
          created_at: '2026-08-15T00:00:02Z',
          payload: {
            node_id: 'node-answer',
            text: '历史最终回答',
            presentation: {
              kind: 'answer',
              answer_node_id: 'node-answer',
              segment_index: 0
            }
          },
          delta_index: 4,
          content_type: 'text',
          text: '历史最终回答'
        },
        {
          event_id: 'run-history:5',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'node_finished',
          sequence: 5,
          created_at: '2026-08-15T00:00:03Z',
          payload: {
            node_id: 'node-history',
            status: 'succeeded'
          },
          delta_index: null,
          content_type: null,
          text: null
        }
      ],
      has_more: false,
      next_sequence: null
    });

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('First conversation'));
    expect(await screen.findByText('历史最终回答')).toBeInTheDocument();
    expect(screen.queryByText('历史思考')).not.toBeInTheDocument();
    await domEvents.click(
      await screen.findByText(
        i18nText('appShell', 'auto.assistant_activity_duration_seconds', {
          value1: 3
        })
      )
    );
    await domEvents.click(
      await screen.findByText(i18nText('agentFlow', 'auto.think'))
    );
    const historyReasoning = await screen.findByText('历史思考');
    expect(
      historyReasoning.compareDocumentPosition(screen.getByText('历史最终回答'))
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    const activityButtons = screen.getAllByRole('button', {
      name: i18nText('appShell', 'auto.assistant_activity')
    });
    await domEvents.click(activityButtons.at(-1) as HTMLElement);

    const sidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    expect(
      await within(sidebar).findByText('History node', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    expect(
      await within(sidebar).findByText('17.79 K tokens')
    ).toBeInTheDocument();
    expect(within(sidebar).getByText('1.46 K tokens')).toBeInTheDocument();
    expect(within(sidebar).queryByText('历史思考')).not.toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRunDebugSnapshot).toHaveBeenCalledWith(
      'flow-1',
      'run-history'
    );
    expect(getConsoleAssistantRunActivity).toHaveBeenCalledWith(
      'flow-1',
      'run-history',
      { pageSize: 500 }
    );
  });

});
