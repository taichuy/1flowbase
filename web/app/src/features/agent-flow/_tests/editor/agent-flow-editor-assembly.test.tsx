import { authenticateEditor } from '../fixtures/editor-auth';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { App } from 'antd';
import { act } from '@testing-library/react';
import { memo, useState } from 'react';
import { beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';

import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';

import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';
import type { AgentFlowVariableGroup } from '../../api/runtime';

let finishPreviewLoading: () => void;
let previewLoaded = false;
const previewLoading = new Promise<void>((resolve) => {
  finishPreviewLoading = () => {
    previewLoaded = true;
    resolve();
  };
});

vi.mock('../../components/assistant-plugin/DraftAssistantPreview', () => ({
  DraftAssistantPreview: () => {
    if (!previewLoaded) throw previewLoading;
    return <div>Loaded preview content</div>;
  }
}));

let canvasBoundaryCommitCount = 0;
let pushDebugMessage: (() => void) | null = null;
let replaceVariableGroups: ((groups: AgentFlowVariableGroup[]) => void) | null = null;

vi.mock('../../components/editor/AgentFlowCanvas', () => ({
  AgentFlowCanvas: memo(function MockAgentFlowCanvas() {
    canvasBoundaryCommitCount += 1;
    return <div data-testid="mock-agent-flow-canvas" />;
  })
}));

vi.mock('../../hooks/runtime/useAgentFlowDebugSession', () => ({
  useAgentFlowDebugSession: () => {
    const [messages, setMessages] = useState<unknown[]>([]);
    const [variableGroups, setVariableGroups] = useState<AgentFlowVariableGroup[]>([]);
    replaceVariableGroups = setVariableGroups;
    pushDebugMessage = () =>
      setMessages((current) => [
        ...current,
        { id: `message-${current.length}` }
      ]);

    return {
      activeRunId: null,
      clearSession: vi.fn(),
      messages,
      runContext: {},
      selectRunScope: vi.fn(),
      setRunContextValue: vi.fn(),
      status: 'idle',
      stopRun: vi.fn(),
      stopping: false,
      submitPrompt: vi.fn(),
      variableGroups
    };
  }
}));

describe('AgentFlowEditorAssembly', () => {
  beforeAll(() => loadApplicationI18nResources());
  beforeEach(() => {
    authenticateEditor();
    canvasBoundaryCommitCount = 0;
    pushDebugMessage = null;
    replaceVariableGroups = null;
  });

  test('AC-003 keeps AgentFlow preview and variable tools in its assembly', () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AgentFlowEditorAssembly
            applicationId="app-1"
            applicationName="Agent flow"
            initialState={{
              flow_id: 'flow-1',
              messages: [],
              draft: {
                id: 'draft-1',
                flow_id: 'flow-1',
                updated_at: '2026-07-10T10:00:00Z',
                document
              },
              autosave_interval_seconds: 30,
              user_protection_limit: 10,
              versions: []
            }}
            nodeCatalog={{ nodes: [] }}
          />
        </App>
      </QueryClientProvider>
    );

    expect(
      screen.getByTestId('agent-flow-editor-assembly')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '预览' })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '会话变量' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '系统变量' })
    ).toBeInTheDocument();
  });

  test('shows local loading while the first preview is pending and replaces it when ready', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AgentFlowEditorAssembly
            applicationId="app-1"
            applicationName="Agent flow"
            initialState={{
              flow_id: 'flow-1',
              messages: [],
              draft: {
                id: 'draft-1',
                flow_id: 'flow-1',
                updated_at: '2026-07-10T10:00:00Z',
                document
              },
              autosave_interval_seconds: 30,
              user_protection_limit: 10,
              versions: []
            }}
            nodeCatalog={{ nodes: [] }}
          />
        </App>
      </QueryClientProvider>
    );

    fireEvent.click(screen.getByRole('button', { name: '预览' }));
    const dock = window.document.querySelector(
      '.agent-flow-editor__debug-console-dock'
    );
    expect(dock).not.toBeNull();
    expect(
      within(dock as HTMLElement).getByRole('status', { name: 'thinking' })
    ).toBeInTheDocument();
    expect(screen.getByTestId('mock-agent-flow-canvas')).toBeVisible();

    await act(async () => finishPreviewLoading());
    expect(
      await within(dock as HTMLElement).findByText('Loaded preview content')
    ).toBeInTheDocument();
    expect(
      within(dock as HTMLElement).queryByRole('status', { name: 'thinking' })
    ).not.toBeInTheDocument();
  });

  test('keeps selected cache values stable without write access and refreshes permissions and source groups', async () => {
    authenticateEditor([], 'member');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const groups: AgentFlowVariableGroup[] = [{
      title: 'Node inputs',
      items: [
        { key: 'node-start.query', label: 'Start/query', value: 'Initial query' },
        { key: 'node-llm.prompt', label: 'LLM/prompt', value: 'Selected prompt' },
        { key: 'sys.user_id', label: 'System/user', value: 'user-1', isReadOnly: true }
      ]
    }];

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AgentFlowEditorAssembly
            applicationId="app-1"
            applicationName="Agent flow"
            initialState={{
              flow_id: 'flow-1',
              messages: [],
              draft: {
                id: 'draft-1',
                flow_id: 'flow-1',
                updated_at: '2026-07-10T10:00:00Z',
                document
              },
              autosave_interval_seconds: 30,
              user_protection_limit: 10,
              versions: []
            }}
            nodeCatalog={{ nodes: [] }}
          />
        </App>
      </QueryClientProvider>
    );
    act(() => replaceVariableGroups?.(groups));
    fireEvent.click(screen.getByRole('button', { name: '查看缓存' }));
    await screen.findByRole('textbox', { name: '变量值编辑框' });
    const value = () => screen.getByRole('textbox', { name: '变量值编辑框' });
    await waitFor(() => expect(value()).toHaveValue('Initial query'));
    expect(value()).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'LLM/prompt' }));
    await waitFor(() => expect(value()).toHaveValue('Selected prompt'));
    expect(screen.getByRole('button', { name: 'LLM/prompt' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: '复制变量值' })).toBeEnabled();

    act(() => pushDebugMessage?.());
    expect(value()).toHaveValue('Selected prompt');
    expect(value()).toBeDisabled();
    // The permission projection must not mutate the source session groups.
    expect(groups[0].items[1].isReadOnly).toBeUndefined();

    act(() => authenticateEditor(['upsert_debug_variable_cache_entry'], 'member'));
    await waitFor(() => expect(value()).toBeEnabled());
    expect(value()).toHaveValue('Selected prompt');
    fireEvent.click(screen.getByRole('button', { name: 'System/user' }));
    expect(value()).toBeDisabled();
    expect(value()).toHaveValue('user-1');
    fireEvent.click(screen.getByRole('button', { name: 'LLM/prompt' }));
    expect(value()).toBeEnabled();

    act(() => authenticateEditor([], 'member'));
    await waitFor(() => expect(value()).toBeDisabled());
    act(() => replaceVariableGroups?.([{
      ...groups[0],
      items: groups[0].items.map((item) => item.key === 'node-llm.prompt'
        ? { ...item, value: 'Updated prompt' }
        : item)
    }]));
    await waitFor(() => expect(value()).toHaveValue('Updated prompt'));
    expect(value()).toBeDisabled();
    expect(screen.getByRole('button', { name: 'LLM/prompt' })).toHaveAttribute('aria-pressed', 'true');
  });

  test('AC-001 does not propagate preview message frames into the memoized canvas', () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    render(
      <QueryClientProvider client={queryClient}>
        <App>
          <AgentFlowEditorAssembly
            applicationId="app-1"
            applicationName="Agent flow"
            initialState={{
              flow_id: 'flow-1',
              messages: [],
              draft: {
                id: 'draft-1',
                flow_id: 'flow-1',
                updated_at: '2026-07-10T10:00:00Z',
                document
              },
              autosave_interval_seconds: 30,
              user_protection_limit: 10,
              versions: []
            }}
            nodeCatalog={{ nodes: [] }}
          />
        </App>
      </QueryClientProvider>
    );
    const initialCanvasCommitCount = canvasBoundaryCommitCount;

    act(() => pushDebugMessage?.());

    expect(canvasBoundaryCommitCount).toBe(initialCanvasCommitCount);
  });
});
