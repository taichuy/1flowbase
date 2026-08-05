import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const {
  getConsoleAssistantSettings,
  startConsoleAssistantRunStream,
  updateConsoleAssistantSettings
} = vi.hoisted(() => ({
  getConsoleAssistantSettings: vi.fn(),
  startConsoleAssistantRunStream: vi.fn(),
  updateConsoleAssistantSettings: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    getConsoleAssistantSettings,
    startConsoleAssistantRunStream,
    updateConsoleAssistantSettings
  };
});

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { i18nText } from '../../../../shared/i18n/text';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

describe('EmbeddedAgentAssistant', () => {
  beforeEach(() => {
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        name: 'Root',
        nickname: 'Root',
        email: 'root@example.com',
        phone: null,
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: []
      }
    });
    getConsoleAssistantSettings.mockReset();
    getConsoleAssistantSettings.mockResolvedValue({
      preference: {
        application_id: 'flow-1',
        mcp_instance_ids: []
      },
      published_agent_flows: [
        { application_id: 'flow-1', name: 'Support Flow' }
      ],
      enabled_mcp_instances: [],
      run_capabilities: {
        model_selection_enabled: true,
        reasoning_effort_enabled: true,
        models: [
          {
            id: 'gpt-5.4',
            name: 'GPT-5.4',
            reasoning_efforts: ['low', 'high'],
            default_reasoning_effort: 'high'
          }
        ]
      }
    });
    updateConsoleAssistantSettings.mockReset();
    startConsoleAssistantRunStream.mockReset();
  });

  test('AC-001 opens the Agent Flow Preview console instead of a hand-written drawer', async () => {
    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );

    const trigger = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant')
    });
    expect(trigger).toHaveTextContent('AI');

    await act(async () => {
      fireEvent.click(trigger);
    });

    await waitFor(() => {
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1);
    });
    expect(
      document.querySelector('.agent-flow-editor__debug-console')
    ).toBeInTheDocument();
    expect(document.querySelector('.ant-drawer')).not.toBeInTheDocument();
    expect(
      screen.getByTestId('embedded-agent-assistant-preview')
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole('separator').map((element) => element.getAttribute('aria-label'))
    ).toEqual(expect.arrayContaining([expect.any(String)]));

    const settings = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant_settings')
    });
    expect(settings).toHaveTextContent('AI');
    expect(settings.querySelector('.anticon-setting')).toBeNull();

    await waitFor(() => expect(settings).not.toBeDisabled());
    await act(async () => {
      fireEvent.click(settings);
    });
    expect(
      await screen.findByRole('dialog', {
        name: i18nText('appShell', 'auto.assistant_settings')
      })
    ).toBeInTheDocument();
    expect(
      screen.getByLabelText(i18nText('appShell', 'auto.assistant_model'))
    ).toBeInTheDocument();
  });

  test('AC-004 projects assistant SSE events through the Preview conversation', async () => {
    startConsoleAssistantRunStream.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-1',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-1',
          node_id: 'node-answer',
          text: 'Assistant reply',
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'flow_incomplete',
          run_id: 'run-1',
          status: 'incomplete',
          reason: 'output_limit',
          output: { answer: 'Assistant reply' }
        });
      }
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );

    await waitFor(() => {
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1);
    });

    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).not.toBeDisabled());
    fireEvent.change(composer, { target: { value: 'Summarize this' } });
    fireEvent.click(sendButton);

    await waitFor(() => {
      expect(startConsoleAssistantRunStream).toHaveBeenCalledWith(
        { query: 'Summarize this', history: [] },
        'csrf-token',
        expect.any(Object)
      );
    });
    expect(await screen.findByText('Assistant reply')).toBeInTheDocument();
  });
});
