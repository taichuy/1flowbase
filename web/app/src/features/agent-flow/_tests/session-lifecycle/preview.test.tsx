import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { App } from 'antd';
import type { ComponentProps } from 'react';
import { afterEach, beforeAll, beforeEach, expect, test, vi } from 'vitest';
import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import * as runtime from '../../api/runtime';
import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';
import type { DraftAssistantPreview } from '../../components/assistant-plugin/DraftAssistantPreview';

vi.mock('../../components/editor/AgentFlowCanvas', () => ({
  AgentFlowCanvas: () => null
}));
// Keep the real editor/session owners; replace only the conversation surface.
vi.mock('../../components/assistant-plugin/DraftAssistantPreview', () => ({
  DraftAssistantPreview: (
    props: ComponentProps<typeof DraftAssistantPreview>
  ) => {
    const query = props.runContext.fields.find(
      (field) => field.key === 'query'
    )!;
    return (
      <div>
        <output data-testid="preview-messages">
          {JSON.stringify(props.messages)}
        </output>
        <output data-testid="preview-status">{props.status}</output>
        <output data-testid="preview-mcp">
          {props.mcp_instance_ids.join(',')}
        </output>
        <input
          aria-label="Temporary query"
          value={String(query.value ?? '')}
          onChange={(event) =>
            props.onChangeRunContextValue(
              query.nodeId,
              query.key,
              event.target.value
            )
          }
        />
        <button onClick={() => props.onChangeMcpInstanceIds(['temporary-mcp'])}>
          Choose preview MCP
        </button>
        <button onClick={() => props.onSubmitPrompt('Preview question')}>
          Send preview
        </button>
        <button onClick={props.onClose}>Close preview</button>
      </div>
    );
  }
}));

const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
function detail(): runtime.FlowDebugRunDetail {
  return {
    flow_run: {
      id: 'run-old',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run',
      status: 'succeeded',
      target_node_id: null,
      input_payload: {},
      output_payload: { answer: 'Old preview answer' },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-09-10T00:00:00Z',
      finished_at: '2026-09-10T00:00:01Z',
      created_at: '2026-09-10T00:00:00Z'
    },
    node_runs: [],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}
function mountEditor() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  render(
    <QueryClientProvider client={client}>
      <App>
        <AgentFlowEditorAssembly
          applicationId="app-1"
          applicationName="Preview flow"
          initialState={{
            flow_id: 'flow-1',
            messages: [],
            draft: {
              id: 'draft-1',
              flow_id: 'flow-1',
              updated_at: '',
              document
            },
            autosave_interval_seconds: 30,
            user_protection_limit: 10,
            versions: []
          }}
        />
      </App>
    </QueryClientProvider>
  );
  fireEvent.click(screen.getByRole('button', { name: '预览' }));
}
async function reopen() {
  fireEvent.click(screen.getByRole('button', { name: 'Close preview' }));
  fireEvent.click(screen.getByRole('button', { name: '预览' }));
  await screen.findByRole('button', { name: 'Send preview' });
}
function expectEmptyPreview() {
  expect(screen.getByTestId('preview-messages')).toHaveTextContent(/^\[\]$/);
  expect(screen.getByTestId('preview-status')).toHaveTextContent('idle');
}

beforeAll(() => loadApplicationI18nResources());
beforeEach(() => {
  resetAuthStore();
  useAuthStore.setState({ csrfToken: 'csrf-token' });
  vi.spyOn(runtime, 'fetchDebugVariableSnapshot').mockResolvedValue({
    variable_cache: {}
  });
  vi.spyOn(runtime, 'cancelFlowDebugRun');
  vi.spyOn(runtime, 'startFlowDebugRunStream').mockRejectedValue(
    new Error('Stream unavailable')
  );
  vi.spyOn(runtime, 'startFlowDebugRun').mockResolvedValue(detail());
});
afterEach(() => vi.restoreAllMocks());

test('#2022 AC-002 starts a fresh preview after close with default temporary input and MCP', async () => {
  mountEditor();
  fireEvent.click(
    await screen.findByRole('button', { name: 'Choose preview MCP' })
  );
  fireEvent.click(screen.getByRole('button', { name: 'Send preview' }));
  await waitFor(() =>
    expect(screen.getByTestId('preview-messages')).toHaveTextContent(
      'Old preview answer'
    )
  );
  fireEvent.change(screen.getByLabelText('Temporary query'), {
    target: { value: 'Unsent draft' }
  });
  const previousSessionId = vi.mocked(runtime.startFlowDebugRun).mock
    .calls[0][1].debug_session_id;
  await reopen();
  expectEmptyPreview();
  expect(screen.getByLabelText('Temporary query')).toHaveValue('');
  expect(screen.getByTestId('preview-mcp')).toBeEmptyDOMElement();
  fireEvent.click(screen.getByRole('button', { name: 'Send preview' }));
  await waitFor(() =>
    expect(runtime.startFlowDebugRun).toHaveBeenCalledTimes(2)
  );
  expect(
    vi.mocked(runtime.startFlowDebugRun).mock.calls[1][1].debug_session_id
  ).not.toBe(previousSessionId);
  expect(runtime.cancelFlowDebugRun).not.toHaveBeenCalled();
});

test.each(['fallback', 'snapshot'] as const)(
  '#2022 AC-003 ignores a late %s after preview close',
  async (source) => {
    let resolveDetail!: (value: runtime.FlowDebugRunDetail) => void;
    const pending = new Promise<runtime.FlowDebugRunDetail>((resolve) => {
      resolveDetail = resolve;
    });
    if (source === 'fallback') {
      vi.mocked(runtime.startFlowDebugRun).mockReturnValue(pending);
    } else {
      vi.mocked(runtime.startFlowDebugRunStream).mockImplementation(
        async (_app, _input, _csrf, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'run-old',
            status: 'running'
          });
          handlers.onEvent({
            type: 'flow_finished',
            run_id: 'run-old',
            status: 'succeeded',
            output: {}
          });
        }
      );
      vi.spyOn(runtime, 'fetchApplicationRunDebugSnapshot').mockReturnValue(
        pending
      );
    }
    mountEditor();
    fireEvent.click(
      await screen.findByRole('button', { name: 'Send preview' })
    );
    await waitFor(() =>
      expect(
        source === 'fallback'
          ? runtime.startFlowDebugRun
          : runtime.fetchApplicationRunDebugSnapshot
      ).toHaveBeenCalled()
    );
    await reopen();
    await act(async () => {
      resolveDetail(detail());
      await pending;
    });
    expectEmptyPreview();
    expect(runtime.cancelFlowDebugRun).not.toHaveBeenCalled();
  }
);

test('#2022 AC-003 detaches the preview stream and ignores its late events', async () => {
  const controller = new AbortController();
  let handlers!: runtime.FlowDebugRunStreamHandlers;
  let finishStream!: () => void;
  const pending = new Promise<void>((resolve) => {
    finishStream = resolve;
  });
  vi.mocked(runtime.startFlowDebugRunStream).mockImplementation(
    async (_app, _input, _csrf, next) => {
      handlers = next;
      next.getAbortController?.(controller);
      next.onEvent({
        type: 'flow_started',
        run_id: 'run-old',
        status: 'running'
      });
      await pending;
    }
  );
  mountEditor();
  fireEvent.click(await screen.findByRole('button', { name: 'Send preview' }));
  await waitFor(() =>
    expect(screen.getByTestId('preview-status')).toHaveTextContent('running')
  );
  await reopen();
  expect(controller.signal.aborted).toBe(true);
  await act(async () => {
    handlers.onEvent({
      type: 'flow_finished',
      run_id: 'run-old',
      status: 'succeeded',
      output: { answer: 'Late answer' }
    });
    finishStream();
    await pending;
  });
  expectEmptyPreview();
  expect(runtime.startFlowDebugRun).not.toHaveBeenCalled();
  expect(runtime.cancelFlowDebugRun).not.toHaveBeenCalled();
});
