import { cleanup, act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, expect, vi } from 'vitest';

const {
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  subscribeConsoleAssistantConversationsWebSocket,
  updateConsoleAssistantSettings
} = vi.hoisted(() => ({
  attachConsoleAssistantRunWebSocket: vi.fn(),
  cancelConsoleFlowRun: vi.fn(),
  createConsoleAssistantConversation: vi.fn(),
  getConsoleAssistantConversationMessages: vi.fn(),
  getConsoleAssistantLegacySnapshotMessages: vi.fn(),
  getConsoleAssistantRunActivity: vi.fn(),
  getConsoleAssistantSettings: vi.fn(),
  listConsoleAssistantConversations: vi.fn(),
  startConsoleAssistantRunWebSocket: vi.fn(),
  startConsoleAssistantRunStream: vi.fn(),
  subscribeConsoleAssistantConversationsWebSocket: vi.fn(),
  updateConsoleAssistantSettings: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    attachConsoleAssistantRunWebSocket,
    cancelConsoleFlowRun,
    createConsoleAssistantConversation,
    getConsoleAssistantConversationMessages,
    getConsoleAssistantLegacySnapshotMessages,
    getConsoleAssistantRunActivity,
    getConsoleAssistantSettings,
    listConsoleAssistantConversations,
    startConsoleAssistantRunWebSocket,
    startConsoleAssistantRunStream,
    subscribeConsoleAssistantConversationsWebSocket,
    updateConsoleAssistantSettings
  };
});

vi.mock('@monaco-editor/react', () => ({
  default: ({ value }: { value: string }) => (
    <pre data-testid="mock-json-editor">{value}</pre>
  )
}));

vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

// Collect the real lazy chunks as suite dependencies, after hoisted mock registration.
import '../../components/embedded-assistant/EmbeddedAgentAssistantPreview';
import '../../components/assistant-plugin/AssistantPanelPlugin';
import '../../components/embedded-assistant/AssistantRunActivityPanel';

import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';

import { i18nText } from '../../../../shared/i18n/text';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

interface WindowDimensionSpy {
  mockRestore(): void;
  mockReturnValue(value: number): void;
}

const ASSISTANT_WINDOW_SIZE_STORAGE_KEY =
  '1flowbase.embedded_assistant.window_size';

async function findReadyHistoryButton() {
  const button = await screen.findByRole('button', {
    name: i18nText('appShell', 'auto.assistant_history')
  });
  // The toolbar exists before the asynchronous settings make history available.
  await waitFor(() => expect(button).toBeEnabled());
  return button;
}

let innerHeightSpy: WindowDimensionSpy | undefined;
let innerWidthSpy: WindowDimensionSpy | undefined;

export function setupEmbeddedAssistantFixture() {
  beforeAll(async () => {
    // Global setup normally completes this flight; keep the fixture's ready contract.
    await loadApplicationI18nResources();
  });
  beforeEach(() => {
    window.localStorage.removeItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY);
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(900);
    innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
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
        mcp_instance_ids: [],
        enabled_client_tools: ['get_client_context', 'refresh_client_view']
      },
      published_agent_flows: [
        { application_id: 'flow-1', name: 'Support Flow' }
      ],
      enabled_mcp_instances: [],
      page_reference_max_bytes: 65_536,
      page_reference_max_count: 5,
      page_reference_max_total_bytes: 65_536,
      run_capabilities: {
        model_selection_enabled: true,
        reasoning_effort_enabled: true,
        models: [
          {
            id: 'gpt-5.4',
            name: 'GPT-5.4',
            context_window: 100000,
            reasoning_efforts: ['low', 'high'],
            default_reasoning_effort: 'high'
          }
        ]
      }
    });
    updateConsoleAssistantSettings.mockReset();
    createConsoleAssistantConversation.mockReset();
    createConsoleAssistantConversation.mockResolvedValue({
      conversation_id: 'conversation-new',
      application_id: 'flow-1',
      created_at: '2026-08-07T00:00:00Z',
      updated_at: '2026-08-07T00:00:00Z'
    });
    getConsoleAssistantConversationMessages.mockReset();
    getConsoleAssistantConversationMessages.mockResolvedValue([]);
    getConsoleAssistantLegacySnapshotMessages.mockReset();
    getConsoleAssistantLegacySnapshotMessages.mockResolvedValue([
      {
        id: 'legacy-run:user',
        flow_run_id: 'legacy-run',
        role: 'user',
        content: 'Legacy question',
        created_at: '2026-08-06T00:00:00Z'
      }
    ]);
    getConsoleAssistantRunActivity.mockReset();
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'running',
      started_at: '2026-08-16T00:00:00Z',
      finished_at: null,
      duration_ms: null,
      items: [],
      trace_events: [],
      has_more: false,
      next_sequence: null
    });
    listConsoleAssistantConversations.mockReset();
    listConsoleAssistantConversations.mockResolvedValue({
      items: [
        {
          conversation_id: 'conversation-1',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-1',
          latest_flow_run_status: 'succeeded',
          title: 'First conversation',
          created_at: '2026-08-07T00:00:00Z',
          updated_at: '2026-08-07T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    cancelConsoleFlowRun.mockReset();
    cancelConsoleFlowRun.mockResolvedValue(undefined);
    attachConsoleAssistantRunWebSocket.mockReset();
    attachConsoleAssistantRunWebSocket.mockResolvedValue(undefined);
    startConsoleAssistantRunWebSocket.mockReset();
    startConsoleAssistantRunStream.mockReset();
    subscribeConsoleAssistantConversationsWebSocket.mockReset();
    subscribeConsoleAssistantConversationsWebSocket.mockImplementation(
      async (applicationId, _csrfToken, handlers) => {
        const controller = new AbortController();
        handlers.getAbortController?.(controller);
        handlers.onSnapshot(
          await listConsoleAssistantConversations(applicationId, {
            page: 1,
            pageSize: 20
          })
        );
      }
    );
  });

  afterEach(async () => {
    await act(async () => {
      cleanup();
      resetAuthStore();
    });
    window.localStorage.removeItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY);
    innerHeightSpy?.mockRestore();
    innerHeightSpy = undefined;
    innerWidthSpy?.mockRestore();
    innerWidthSpy = undefined;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

}

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => { result = event(); });
  return result;
}
async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => { view = render(...args); });
  return view;
}
async function rerenderSettled(
  rerender: ReturnType<typeof render>['rerender'],
  ui: Parameters<ReturnType<typeof render>['rerender']>[0]
) {
  await act(async () => { rerender(ui); });
}
const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) => dispatch(() => fireEvent.click(...args)),
  change: (...args: Parameters<typeof fireEvent.change>) => dispatch(() => fireEvent.change(...args)),
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) => dispatch(() => fireEvent.mouseDown(...args)),
  mouseMove: (...args: Parameters<typeof fireEvent.mouseMove>) => dispatch(() => fireEvent.mouseMove(...args)),
  mouseUp: (...args: Parameters<typeof fireEvent.mouseUp>) => dispatch(() => fireEvent.mouseUp(...args)),
  mouseEnter: (...args: Parameters<typeof fireEvent.mouseEnter>) => dispatch(() => fireEvent.mouseEnter(...args)),
  mouseLeave: (...args: Parameters<typeof fireEvent.mouseLeave>) => dispatch(() => fireEvent.mouseLeave(...args)),
  fire: (...args: Parameters<typeof fireEvent>) => dispatch(() => fireEvent(...args))
};

export {
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  subscribeConsoleAssistantConversationsWebSocket,
  updateConsoleAssistantSettings,
  ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
  innerWidthSpy,
  findReadyHistoryButton,
  renderSettled,
  rerenderSettled,
  domEvents
};
