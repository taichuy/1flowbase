import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, expect, test, vi } from 'vitest';
import * as api from '@1flowbase/api-client';
import { AppProviders } from '../../../../app/AppProviders';
import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { writeLocalePreferenceToStorage } from '../../../../shared/user-preferences/locale-preference';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import type { AgentFlowDebugMessage } from '../../api/runtime';

vi.mock(
  '../../components/embedded-assistant/AssistantRunActivityPanel',
  () => ({
    AssistantRunNodePanel: () => <div>Selected run activity</div>,
    AssistantRunTimeline: ({ message }: { message: AgentFlowDebugMessage }) => (
      <div>{message.content}</div>
    )
  })
);

beforeAll(() => loadApplicationI18nResources());
beforeEach(() => {
  resetAuthStore();
  useAuthStore.setState({ csrfToken: 'csrf-token' });
  window.localStorage.clear();
  writeLocalePreferenceToStorage('zh_Hans');
  vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1440);
  vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(900);
  vi.spyOn(api, 'getConsoleAssistantSettings').mockResolvedValue({
    preference: {
      application_id: 'app-1',
      mcp_instance_ids: [],
      enabled_client_tools: []
    },
    published_agent_flows: [
      { application_id: 'app-1', name: 'Assistant flow' }
    ],
    enabled_mcp_instances: [],
    page_reference_max_bytes: 0,
    page_reference_max_count: 0,
    page_reference_max_total_bytes: 0,
    run_capabilities: {
      model_selection_enabled: false,
      reasoning_effort_enabled: false,
      models: []
    }
  });
  vi.spyOn(
    api,
    'subscribeConsoleAssistantConversationsWebSocket'
  ).mockImplementation(async (_applicationId, _csrfToken, handlers) => {
    handlers.getAbortController?.(new AbortController());
    handlers.onSnapshot({
      items: [
        {
          conversation_id: 'conversation-1',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-1',
          latest_flow_run_status: 'succeeded',
          title: 'Saved conversation',
          created_at: '',
          updated_at: ''
        }
      ],
      page: 1,
      page_size: 20,
      total: 1
    });
  });
  vi.spyOn(api, 'getConsoleAssistantConversationMessages').mockResolvedValue([
    {
      id: 'user-1',
      role: 'user',
      page_references: [],
      content: 'Keep this question',
      flow_run_id: 'run-1',
      status: 'succeeded',
      created_at: ''
    },
    {
      id: 'answer-1',
      role: 'assistant',
      page_references: [],
      content: 'Keep this answer',
      flow_run_id: 'run-1',
      status: 'succeeded',
      created_at: ''
    }
  ]);
});
afterEach(() => vi.restoreAllMocks());

test.each(['运行过程', '历史会话'])(
  '#2022 AC-001 keeps chat when reopening from %s',
  async (sidePanel) => {
    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    const trigger = screen.getByRole('button', { name: 'AI 助手' });
    fireEvent.click(trigger);
    const history = await screen.findByRole(
      'button',
      { name: '历史会话' },
      { timeout: 5000 }
    );
    await waitFor(() => expect(history).toBeEnabled());
    fireEvent.click(history);
    fireEvent.click(await screen.findByText('Saved conversation'));
    expect(await screen.findByText('Keep this answer')).toBeVisible();
    fireEvent.click(screen.getAllByRole('button', { name: sidePanel }).at(-1)!);
    expect(
      await screen.findByTestId('embedded-agent-assistant-history')
    ).toBeInTheDocument();
    fireEvent.click(trigger);
    expect(
      screen.queryByTestId('embedded-agent-assistant-preview')
    ).not.toBeInTheDocument();
    fireEvent.click(trigger);
    await waitFor(() =>
      expect(screen.getByText('Keep this question')).toBeVisible()
    );
    expect(screen.getByText('Keep this answer')).toBeVisible();
    expect(screen.getByText('conversation-1')).toBeVisible();
    expect(
      screen.queryByTestId('embedded-agent-assistant-history')
    ).not.toBeInTheDocument();
  }
);
