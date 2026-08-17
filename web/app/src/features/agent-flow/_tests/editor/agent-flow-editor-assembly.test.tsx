import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { App } from 'antd';
import { act } from '@testing-library/react';
import { memo, useState } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';

let canvasBoundaryCommitCount = 0;
let pushDebugMessage: (() => void) | null = null;

vi.mock('../../components/editor/AgentFlowCanvas', () => ({
  AgentFlowCanvas: memo(function MockAgentFlowCanvas() {
    canvasBoundaryCommitCount += 1;
    return <div data-testid="mock-agent-flow-canvas" />;
  })
}));

vi.mock('../../hooks/runtime/useAgentFlowDebugSession', () => ({
  useAgentFlowDebugSession: () => {
    const [messages, setMessages] = useState<unknown[]>([]);
    pushDebugMessage = () =>
      setMessages((current) => [...current, { id: `message-${current.length}` }]);

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
      variableGroups: []
    };
  }
}));

describe('AgentFlowEditorAssembly', () => {
  beforeEach(() => {
    canvasBoundaryCommitCount = 0;
    pushDebugMessage = null;
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
