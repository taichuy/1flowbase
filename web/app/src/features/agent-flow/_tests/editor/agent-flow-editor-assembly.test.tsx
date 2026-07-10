import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { App } from 'antd';
import { describe, expect, test, vi } from 'vitest';

import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';

vi.mock('../../hooks/runtime/useAgentFlowDebugSession', () => ({
  useAgentFlowDebugSession: () => ({
    activeRunId: null,
    clearSession: vi.fn(),
    messages: [],
    runContext: {},
    selectRunScope: vi.fn(),
    setRunContextValue: vi.fn(),
    status: 'idle',
    stopRun: vi.fn(),
    stopping: false,
    submitPrompt: vi.fn(),
    variableGroups: []
  })
}));

describe('AgentFlowEditorAssembly', () => {
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
            nodeContributions={[]}
          />
        </App>
      </QueryClientProvider>
    );

    expect(screen.getByTestId('agent-flow-editor-assembly')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '预览' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '会话变量' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '系统变量' })).toBeInTheDocument();
  });
});
