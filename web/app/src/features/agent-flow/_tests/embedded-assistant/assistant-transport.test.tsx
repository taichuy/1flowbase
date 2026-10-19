import {
  setupEmbeddedAssistantFixture,
  renderSettled,
  domEvents,
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  getConsoleAssistantSettings,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream
} from './assistant-fixture';
import { act, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import * as runtimeApi from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';

describe('EmbeddedAgentAssistant', () => {
  setupEmbeddedAssistantFixture();

  test('issue 1601 drives context from AI Gateway snapshots instead of Provider usage', async () => {
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockResolvedValue({
      flow_run: { status: 'succeeded' },
      node_runs: [],
      checkpoints: [],
      callback_tasks: [],
      events: [],
      context_snapshot: {
        type: 'context_snapshot',
        event_id: 'event-context-terminal',
        run_id: 'run-context',
        node_run_id: 'node-run-llm',
        node_id: 'node-llm',
        sequence: 9,
        input_tokens: 321,
        effective_context_window: 100000,
        remaining_tokens: 99679,
        measurement: {
          method: 'generic_estimate',
          accuracy: 'estimated',
          coverage: 'complete',
          unknown_block_count: 0
        },
        created_at: '2026-08-06T00:00:00Z'
      }
    } as never);
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-context',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'context_snapshot',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          input_tokens: 100,
          effective_context_window: 100000,
          remaining_tokens: 99900,
          measurement: {
            method: 'generic_estimate',
            accuracy: 'estimated',
            coverage: 'complete',
            unknown_block_count: 0
          }
        });
        handlers.onEvent({
          type: 'usage_snapshot',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          usage: {
            input_tokens: 80000,
            output_tokens: 50,
            total_tokens: 80050
          }
        });
        handlers.onEvent({
          type: 'node_finished',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          status: 'succeeded',
          metrics_payload: {
            usage: {
              input_tokens: 90000,
              output_tokens: 50,
              total_tokens: 90050
            }
          }
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-context',
          status: 'succeeded',
          output: { answer: 'Context updated' }
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
    const initialContextProgress = document.querySelector(
      '.assistant-panel-plugin__context-progress'
    );
    expect(initialContextProgress).toBeInTheDocument();
    await domEvents.mouseEnter(initialContextProgress as HTMLElement);
    expect(await screen.findByText('剩余：100%')).toBeInTheDocument();
    expect(
      await screen.findByText('最大上下文 100K，已用 0')
    ).toBeInTheDocument();
    await domEvents.mouseLeave(initialContextProgress as HTMLElement);
    await domEvents.change(composer, { target: { value: 'Measure context' } });
    await domEvents.click(sendButton);

    const contextProgress = await waitFor(() => {
      const element = document.querySelector(
        '.assistant-panel-plugin__context-progress'
      );
      expect(element).toBeInTheDocument();
      return element as HTMLElement;
    });
    await domEvents.mouseEnter(contextProgress);

    expect(
      await screen.findByText('最大上下文 100K，已用 321')
    ).toBeInTheDocument();
    expect(await screen.findByText('剩余：99.7%')).toBeInTheDocument();
    expect(contextProgress.querySelector('.ant-progress')).toHaveAttribute(
      'aria-valuenow',
      '1'
    );
  });

  test('issue 1601 does not poll snapshots while WebSocket is healthy and calibrates once at terminal', async () => {
    const snapshot = vi
      .spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot')
      .mockRejectedValue(new Error('optional calibration unavailable'));
    let finishWebSocket: (() => void) | null = null;
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-live',
          status: 'queued'
        });
        await new Promise<void>((resolve) => {
          finishWebSocket = () => {
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-live',
              status: 'succeeded',
              output: { answer: 'done' }
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
    await domEvents.change(composer, { target: { value: 'Keep streaming' } });
    await domEvents.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    expect(snapshot).not.toHaveBeenCalled();

    await act(async () => {
      finishWebSocket?.();
    });
    await waitFor(() =>
      expect(snapshot).toHaveBeenCalledWith('flow-1', 'run-live')
    );
    expect(snapshot).toHaveBeenCalledTimes(1);
  });

  test('issue 1601 falls back to Assistant SSE only before any WebSocket event', async () => {
    startConsoleAssistantRunWebSocket.mockRejectedValue(
      new Error('WebSocket unavailable')
    );
    startConsoleAssistantRunStream.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-fallback',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-fallback',
          status: 'succeeded',
          output: { answer: 'SSE fallback reply' }
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
    await waitFor(() =>
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1)
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    await domEvents.change(composer, { target: { value: 'Fallback please' } });
    await domEvents.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunStream).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          conversation_id: 'conversation-new',
          query: 'Fallback please'
        },
        'csrf-token',
        expect.any(Object)
      )
    );
  });

  test('issue 1601 stops a stalled handshake before a run id exists', async () => {
    const abort = vi.fn();
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) =>
        new Promise<void>((_resolve, reject) => {
          const abortController = new AbortController();
          abortController.signal.addEventListener('abort', () => {
            abort();
            reject(new DOMException('Aborted', 'AbortError'));
          });
          handlers.getAbortController?.(abortController);
        })
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
    await domEvents.change(composer, { target: { value: 'Stall' } });
    await domEvents.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.terminate_debugging_run')
      })
    );

    await waitFor(() => expect(abort).toHaveBeenCalledTimes(1));
    expect(startConsoleAssistantRunStream).not.toHaveBeenCalled();
    await waitFor(() => expect(composer).toBeEnabled());
  });

  test('AC-001 closes only the transport and reattaches the accepted run when reopened', async () => {
    const abort = vi.fn();
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) =>
        new Promise<void>((_resolve, reject) => {
          const abortController = new AbortController();
          abortController.signal.addEventListener('abort', () => {
            abort();
            reject(new DOMException('Aborted', 'AbortError'));
          });
          handlers.getAbortController?.(abortController);
          handlers.onEvent({
            type: 'flow_accepted',
            run_id: 'run-close',
            status: 'queued'
          });
        })
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
    await domEvents.change(composer, { target: { value: 'Close me' } });
    await domEvents.click(sendButton);
    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );

    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.close', {
          value1: i18nText('appShell', 'auto.assistant')
        })
      })
    );

    await waitFor(() => expect(abort).toHaveBeenCalledTimes(1));
    expect(cancelConsoleFlowRun).not.toHaveBeenCalled();
    expect(
      screen.queryByTestId('embedded-agent-assistant-preview')
    ).not.toBeInTheDocument();

    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await waitFor(() =>
      expect(attachConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        'flow-1',
        'run-close',
        'csrf-token',
        expect.any(Object),
        expect.objectContaining({ onControl: expect.any(Function) })
      )
    );
  });

});
