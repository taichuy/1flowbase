import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import { beforeAll, expect, test, vi } from 'vitest';
import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';
import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';

const { setRunContextValue } = vi.hoisted(() => ({
  setRunContextValue: vi.fn()
}));
vi.mock('../../components/editor/AgentFlowCanvas', () => ({
  AgentFlowCanvas: () => <div />
}));
vi.mock('../../hooks/runtime/useAgentFlowDebugSession', () => ({
  useAgentFlowDebugSession: () => ({
    activeRunId: null,
    messages: [],
    status: 'idle',
    stopping: false,
    variableGroups: [],
    clearSession: vi.fn(),
    stopRun: vi.fn(),
    submitPrompt: vi.fn(),
    selectRunScope: vi.fn(),
    setRunContextValue,
    runContext: {
      environmentLabel: 'draft',
      remembered: false,
      fields: [
        { nodeId: 'node-start', key: 'query', value: '', valueType: 'string' },
        { nodeId: 'node-start', key: 'model', value: '', valueType: 'string' },
        {
          nodeId: 'node-start',
          key: 'reasoning_effort',
          value: '',
          valueType: 'string'
        }
      ]
    }
  })
}));
beforeAll(() => loadApplicationI18nResources());

test('#2018 AC-101/103 preview uses the assistant plugin and writes model selection to draft inputs', async () => {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
  const start = document.graph.nodes.find((node) => node.type === 'start')!;
  start.config.model_list = [
    {
      id: 'draft-model',
      name: 'Draft model',
      reasoning: { default_effort: 'low', supported_efforts: ['low', 'high'] }
    },
    'other-draft-model'
  ];
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <App>
        <AgentFlowEditorAssembly
          applicationId="draft-app"
          applicationName="Current draft"
          nodeCatalog={{ nodes: [] }}
          initialState={{
            flow_id: 'flow-1',
            messages: [],
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '2026-09-09T00:00:00Z',
              document
            },
            versions: [],
            autosave_interval_seconds: 30,
            user_protection_limit: 10
          }}
        />
      </App>
    </QueryClientProvider>
  );
  fireEvent.click(screen.getByRole('button', { name: '预览' }));
  const dock = await screen.findByTestId(
    'agent-flow-editor-debug-console-dock'
  );
  expect(
    await within(dock).findByText('Current draft', {}, { timeout: 5000 })
  ).toBeInTheDocument();
  const modelButton = await screen.findByRole('button', {
    name: /Draft model/
  });
  fireEvent.click(modelButton);
  fireEvent.mouseEnter(
    await screen.findByText('模型', {
      selector: '.assistant-panel-plugin__runtime-menu-row span'
    })
  );
  fireEvent.click(await screen.findByText('other-draft-model'));
  await waitFor(() =>
    expect(setRunContextValue).toHaveBeenCalledWith(
      'node-start',
      'model',
      'other-draft-model'
    )
  );
});
