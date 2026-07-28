import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import * as runtimeApi from '../../api/runtime';
import { AgentFlowEditorShell } from '../../components/editor/AgentFlowEditorShell';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { renderReactFlowScene } from '../../../../test/renderers/render-react-flow-scene';

function createInitialState() {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-25T09:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
  };
}

function renderShell(ui: ReactElement) {
  return renderReactFlowScene(ui);
}

function createCompletedRunDetail() {
  return {
    flow_run: {
      id: 'flow-run-1',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run' as const,
      status: 'succeeded' as const,
      target_node_id: null,
      input_payload: {
        'node-start': { query: '请总结退款政策' }
      },
      output_payload: { answer: '退款政策摘要' },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-25T09:00:00Z',
      finished_at: '2026-04-25T09:00:03Z',
      created_at: '2026-04-25T09:00:00Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-1',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: '用户输入',
        status: 'succeeded' as const,
        input_payload: {},
        output_payload: { query: '请总结退款政策' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T09:00:00Z',
        finished_at: '2026-04-25T09:00:00Z'
      },
      {
        id: 'node-run-answer',
        flow_run_id: 'flow-run-1',
        node_id: 'node-answer',
        node_type: 'answer',
        node_alias: '直接回复',
        status: 'succeeded' as const,
        input_payload: { answer_template: '退款政策摘要' },
        output_payload: { answer: '退款政策摘要' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T09:00:01Z',
        finished_at: '2026-04-25T09:00:03Z'
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

describe('debug console shell', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: ['application.view.all', 'application.edit.own']
      }
    });

    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue({
      flow_run: {
        id: 'flow-run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_flow_run',
        status: 'running',
        target_node_id: null,
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-25T09:00:00Z',
        finished_at: null,
        created_at: '2026-04-25T09:00:00Z'
      },
      node_runs: [],
      checkpoints: [],
      callback_tasks: [],
      events: []
    });
    vi.spyOn(runtimeApi, 'buildFlowDebugRunInput').mockReturnValue({
      input_payload: {
        'node-start': { query: '请总结退款政策' }
      }
    });
    vi.spyOn(runtimeApi, 'fetchNodeLastRun').mockResolvedValue(null);
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockResolvedValue({
      flow_run: {
        id: 'flow-run-1',
        application_id: 'app-1',
        flow_id: 'flow-1',
        draft_id: 'draft-1',
        compiled_plan_id: 'plan-1',
        run_mode: 'debug_flow_run',
        status: 'running',
        target_node_id: null,
        input_payload: {},
        output_payload: {},
        error_payload: null,
        created_by: 'user-1',
        started_at: '2026-04-25T09:00:00Z',
        finished_at: null,
        created_at: '2026-04-25T09:00:00Z'
      },
      node_runs: [],
      checkpoints: [],
      callback_tasks: [],
      events: []
    });
  });

  test('opens a docked chat preview from overlay and keeps inspector separate', async () => {
    renderShell(
      <AgentFlowEditorShell
        applicationId="app-1"
        applicationName="Support Agent"
        initialState={createInitialState()}
      />
    );

    fireEvent.click(await screen.findByRole('button', { name: '预览' }));

    expect(runtimeApi.startFlowDebugRun).not.toHaveBeenCalled();
    expect(
      screen.getByRole('complementary', { name: '预览' })
    ).toBeInTheDocument();
    expect(screen.getByPlaceholderText('和 Bot 聊天')).toBeInTheDocument();
    expect(screen.getByText('功能已开启')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '管理功能' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Input' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: 'Trace' })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: '设置' })).not.toBeInTheDocument();
  }, 20_000);

  test('opens conversation log in the shared resizable side dock', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
      () =>
        ({
          x: 0,
          y: 0,
          width: 1440,
          height: 720,
          top: 0,
          right: 1440,
          bottom: 720,
          left: 0,
          toJSON: () => ({})
        }) as DOMRect
    );
    vi.mocked(runtimeApi.startFlowDebugRun).mockResolvedValue(
      createCompletedRunDetail()
    );

    renderShell(
      <AgentFlowEditorShell
        applicationId="app-1"
        applicationName="Support Agent"
        initialState={createInitialState()}
      />
    );

    fireEvent.click(await screen.findByRole('button', { name: '预览' }));
    fireEvent.change(screen.getByPlaceholderText('和 Bot 聊天'), {
      target: { value: '请总结退款政策' }
    });
    fireEvent.click(screen.getByRole('button', { name: '发送调试消息' }));
    fireEvent.click(
      await screen.findByRole('button', { name: '查看对话日志' })
    );

    const logDock = await screen.findByTestId(
      'agent-flow-editor-conversation-log-dock'
    );
    expect(within(logDock).getByLabelText('对话日志')).toBeInTheDocument();
    expect(
      within(logDock).getByRole('separator', { name: '调整对话日志宽度' })
    ).toBeInTheDocument();
    expect(logDock).toHaveStyle('width: 560px');

    fireEvent.mouseDown(
      within(logDock).getByRole('separator', { name: '调整对话日志宽度' }),
      { clientX: 700 }
    );
    fireEvent.mouseMove(window, { clientX: 640 });
    fireEvent.mouseUp(window);

    await waitFor(() => expect(logDock).toHaveStyle('width: 620px'));
  }, 20_000);
});
