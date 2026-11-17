import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';
import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';
import { ApiClientError, type ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import type { ComponentProps } from 'react';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { resetAuthStore } from '../../../state/auth-store';
import { loadApplicationI18nResources } from '../../../shared/i18n/app-i18n';
import { i18nText } from '../../../shared/i18n/text';
import { createTestMemoryRouter } from '../../../test/renderers/create-test-memory-router';
import { authenticateEditor } from '../../agent-flow/_tests/fixtures/editor-auth';
import * as runtime from '../../agent-flow/api/runtime';
import * as orchestration from '../../agent-flow/api/orchestration';
import * as publicApi from '../../applications/api/public-api';
import type { NodeDetailPanel } from '../../agent-flow/components/detail/NodeDetailPanel';
import type { FlowEditorState } from '../../flow-editor/store';
import type { FlowEditorStore } from '../../flow-editor/store/provider';
import { WorkflowEditorAssembly } from '../components/WorkflowEditorAssembly';

interface CanvasCommit {
  active: boolean;
  selectedNodeId: string | null;
  detailOpen: boolean;
  environmentOpen: boolean;
}
interface DetailCommit {
  runId: string | null;
  nodeId: string | null;
  configSelected: boolean;
  runSelected: boolean;
}
const observed = vi.hoisted(() => ({
  canvas: [] as CanvasCommit[],
  detail: [] as DetailCommit[],
  store: null as FlowEditorStore | null
}));

vi.mock('../../agent-flow/components/editor/AgentFlowCanvas', async () => {
  const { useContext, useLayoutEffect } = await import('react');
  const { screen } = await import('@testing-library/react');
  const { i18nText } = await import('../../../shared/i18n/text');
  const { AgentFlowEditorStoreContext, useAgentFlowEditorStore } = await import('../../agent-flow/store/editor/provider');
  return {
    AgentFlowCanvas: ({ interactionLocked = false }: { interactionLocked?: boolean }) => {
      const store = useContext(AgentFlowEditorStoreContext);
      const selectedNodeId = useAgentFlowEditorStore((state) => state.selectedNodeId);
      const setSelection = useAgentFlowEditorStore((state) => state.setSelection);
      useLayoutEffect(() => {
        observed.store = store;
        observed.canvas.push({
          active: interactionLocked,
          selectedNodeId,
          detailOpen: screen.queryByTestId('agent-flow-editor-detail-dock') !== null,
          environmentOpen: screen.queryByRole('region', { name: i18nText('agentFlow', 'auto.environment_variables') }) !== null
        });
      });
      function select(nodeId: string) {
        setSelection({ selectedNodeId: nodeId, selectedNodeIds: [nodeId], selectedEdgeId: null });
      }
      // The production canvas allows selecting nodes while a run is active.
      return <div>
        <button onClick={() => select('node-workflow-start')}>Select workflow start</button>
        <button onClick={() => select('node-workflow-end')}>Select workflow end</button>
      </div>;
    }
  };
});

// Observe real rendered Tabs; do not substitute activeTab props for visible UI.
vi.mock('../../agent-flow/components/detail/NodeDetailPanel', async (original) => {
  const { NodeDetailPanel: ActualDetail } = await original<typeof import('../../agent-flow/components/detail/NodeDetailPanel')>();
  const { Profiler } = await import('react');
  const { screen, within } = await import('@testing-library/react');
  const { i18nText } = await import('../../../shared/i18n/text');
  const { useAgentFlowEditorStore } = await import('../../agent-flow/store/editor/provider');
  return {
    NodeDetailPanel: (props: ComponentProps<typeof NodeDetailPanel>) => {
      const selectedNodeId = useAgentFlowEditorStore((state) => state.selectedNodeId);
      function recordCommittedDetail() {
        const body = screen.queryByTestId('node-detail-body');
        if (!body) return;
        const tabs = within(body);
        const runLabel = props.activeRunId || props.interactionLocked
          ? i18nText('agentFlow', 'auto.current_run')
          : i18nText('agentFlow', 'auto.last_run');
        observed.detail.push({
          runId: props.activeRunId ?? null,
          nodeId: selectedNodeId,
          configSelected: tabs.getByRole('tab', { name: i18nText('agentFlow', 'auto.settings') }).getAttribute('aria-selected') === 'true',
          runSelected: tabs.getByRole('tab', { name: runLabel }).getAttribute('aria-selected') === 'true'
        });
      }
      return (
        <Profiler id="workflow-detail-commits" onRender={recordCommittedDetail}>
          <ActualDetail {...props} />
        </Profiler>
      );
    }
  };
});

const clients: QueryClient[] = [];
const releasePending: Array<() => void> = [];
function runEntrySnapshot(state: FlowEditorState) {
  return {
    selectedNodeId: state.selectedNodeId,
    selectedNodeIds: state.selectedNodeIds,
    selectedEdgeId: state.selectedEdgeId,
    historyOpen: state.historyOpen,
    issuesOpen: state.issuesOpen,
    nodePickerState: state.nodePickerState,
    pendingLocateNodeId: state.pendingLocateNodeId,
    connectingPayload: state.connectingPayload
  };
}
function initialState(): ConsoleApplicationOrchestrationState {
  return {
    flow_id: 'flow-1', messages: [],
    draft: {
      id: 'draft-1', flow_id: 'flow-1', updated_at: '2026-11-17T00:00:00Z',
      document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
    },
    analysis: { diagnostics: [], node_outputs: [], variable_catalogs: [] },
    autosave_interval_seconds: 30, user_protection_limit: 10, versions: []
  };
}
async function mountFrame() {
  const state = initialState();
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, refetchOnWindowFocus: false },
      mutations: { retry: false }
    }
  });
  clients.push(client);
  vi.spyOn(orchestration, 'analyzeDocument').mockResolvedValue(state.analysis);
  vi.spyOn(publicApi, 'fetchApplicationApiPublication').mockRejectedValue(
    new ApiClientError({ status: 404, message: 'Publication not found' })
  );
  vi.spyOn(runtime, 'fetchNodeLastRun').mockResolvedValue(null);
  vi.spyOn(runtime, 'fetchApplicationRunNodeLastRun').mockResolvedValue(null);
  vi.spyOn(runtime, 'fetchApplicationRunDebugSnapshot').mockRejectedValue(new Error('Unexpected fallback polling in stream fixture'));
  const streams: Array<{ handlers: runtime.FlowDebugRunStreamHandlers; release: () => void }> = [];
  const start = vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
    async (_applicationId, _input, _csrfToken, handlers) => {
      let release!: () => void;
      const pending = new Promise<void>((resolve) => { release = resolve; });
      releasePending.push(release);
      handlers.getAbortController?.(new AbortController());
      streams.push({ handlers, release });
      await pending;
    }
  );
  const scene = await createTestMemoryRouter(
    <WorkflowEditorAssembly
      applicationId="app-1" applicationName="Frame transition"
      initialState={state}
      workflowTriggerContext={{ applicationId: 'app-1', triggerType: 'schedule', mapping: null, schedule: null, workflowStartFieldContract: undefined }}
    />
  );
  render(<AppThemeProvider><AppI18nProvider><QueryClientProvider client={client}>{scene}</QueryClientProvider></AppI18nProvider></AppThemeProvider>);
  async function beginRun() {
    const next = streams.length;
    await click(screen.getByRole('button', { name: i18nText('workflow', 'auto.workflow_test_run') }));
    expect(streams).toHaveLength(next + 1);
    return streams[next];
  }
  async function finishRun(runId: string, stream: (typeof streams)[number]) {
    await act(async () => {
      stream.handlers.onEvent({ type: 'flow_finished', run_id: runId, status: 'succeeded', output: {} });
      stream.release();
    });
    await waitFor(() => expect(screen.getByRole('button', { name: i18nText('workflow', 'auto.workflow_test_run') })).toBeEnabled());
  }
  return { beginRun, finishRun, start };
}
async function click(button: HTMLElement) {
  await act(async () => { button.click(); });
}
async function selectNode(which: 'start' | 'end') {
  await click(screen.getByRole('button', { name: `Select workflow ${which}` }));
  await screen.findByTestId('node-detail-body');
  await waitFor(() => expect(
    within(screen.getByTestId('node-detail-header')).getByDisplayValue(
      which === 'start' ? 'Workflow Start' : 'Workflow End'
    )
  ).toBeInTheDocument());
}
async function chooseConfig() {
  const tab = screen.getByRole('tab', { name: i18nText('agentFlow', 'auto.settings') });
  await click(tab);
  expect(tab).toHaveAttribute('aria-selected', 'true');
}
async function completeFirstRun() {
  const owner = await mountFrame();
  const stream = await owner.beginRun();
  await owner.finishRun('run-A', stream);
  await selectNode('start');
  await waitFor(() => expect(screen.getByRole('tab', { name: i18nText('agentFlow', 'auto.current_run') })).toHaveAttribute('aria-selected', 'true'));
  return owner;
}
beforeAll(async () => {
  await loadApplicationI18nResources();
});
beforeEach(() => {
  observed.canvas.length = 0;
  observed.detail.length = 0;
  observed.store = null;
  resetAuthStore();
  authenticateEditor(['save_draft', 'start_flow_debug_run_stream', 'replace_application_environment_variables'], 'member');
});
afterEach(async () => {
  cleanup();
  await act(async () => { releasePending.splice(0).forEach((release) => release()); });
  clients.splice(0).forEach((client) => client.clear());
  vi.restoreAllMocks();
  resetAuthStore();
});

describe('WorkflowCanvasFrame committed run-transition UI', () => {
  it('notifies the real store once with the complete run-entry cleanup', async () => {
    const owner = await mountFrame();
    await selectNode('start');
    const store = observed.store;
    if (!store) throw new Error('The real editor provider did not expose its store');
    await act(async () => {
      store.getState().setPanelState({
        historyOpen: true,
        issuesOpen: true,
        nodePickerState: {
          open: true,
          anchorNodeId: 'node-workflow-start',
          anchorEdgeId: null,
          anchorCanvasPosition: { x: 12, y: 24 }
        }
      });
      store.getState().setInteractionState({
        pendingLocateNodeId: 'node-workflow-start',
        connectingPayload: {
          sourceNodeId: 'node-workflow-start',
          sourceHandleId: 'output',
          sourceNodeType: 'workflow_start'
        }
      });
    });
    const notifications: ReturnType<typeof runEntrySnapshot>[] = [];
    const unsubscribe = store.subscribe((next, previous) => {
      const snapshot = runEntrySnapshot(next);
      if (JSON.stringify(snapshot) !== JSON.stringify(runEntrySnapshot(previous)))
        notifications.push(snapshot);
    });
    try {
      await owner.beginRun();

      const cleared = {
        selectedNodeId: null,
        selectedNodeIds: [],
        selectedEdgeId: null,
        historyOpen: false,
        issuesOpen: false,
        nodePickerState: {
          open: false,
          anchorNodeId: null,
          anchorEdgeId: null,
          anchorCanvasPosition: null
        },
        pendingLocateNodeId: null,
        connectingPayload: {
          sourceNodeId: null,
          sourceHandleId: null,
          sourceNodeType: null
        }
      };
      expect(runEntrySnapshot(store.getState())).toEqual(cleared);
      expect(notifications).toHaveLength(1);
      expect(notifications).toEqual([cleared]);
    } finally {
      unsubscribe();
    }
  });

  it('shows the new run tab on the first ready detail commit after a new run and same-node selection', async () => {
    const owner = await completeFirstRun();
    await chooseConfig();
    const streamB = await owner.beginRun();
    await owner.finishRun('run-B', streamB);
    const beforeSelection = observed.detail.length;

    await selectNode('start');

    const firstReady = observed.detail.slice(beforeSelection).find((commit) => commit.runId === 'run-B' && commit.nodeId === 'node-workflow-start');
    expect(firstReady).toMatchObject({ configSelected: false, runSelected: true });
    expect(owner.start).toHaveBeenCalledTimes(2);
  });

  it('shows the run tab in the first ready commit for a different node in the same completed run', async () => {
    await completeFirstRun();
    await chooseConfig();
    const beforeSelection = observed.detail.length;

    await selectNode('end');

    const firstReady = observed.detail.slice(beforeSelection).find((commit) => commit.runId === 'run-A' && commit.nodeId === 'node-workflow-end');
    expect(firstReady).toMatchObject({ configSelected: false, runSelected: true });
  });

  it('hides the previous detail dock and environment panel in the first active commit', async () => {
    const owner = await completeFirstRun();
    await chooseConfig();
    await click(screen.getByRole('button', { name: i18nText('agentFlow', 'auto.environment_variables') }));
    await screen.findByRole('region', { name: i18nText('agentFlow', 'auto.environment_variables') });
    expect(screen.getByTestId('agent-flow-editor-detail-dock')).toBeInTheDocument();
    const beforeRun = observed.canvas.length;

    await owner.beginRun();

    const firstActive = observed.canvas.slice(beforeRun).find((commit) => commit.active);
    expect(firstActive).toMatchObject({ detailOpen: false, environmentOpen: false });
    // Shared canvas highlight/store timing is deliberately outside this visible Frame AC.
  });

  it('retains manual config for the same run and node through an unrelated real panel resize', async () => {
    await completeFirstRun();
    await chooseConfig();
    const beforeResize = observed.detail.length;

    fireEvent.keyDown(screen.getByRole('separator', { name: i18nText('agentFlow', 'auto.adjust_node_detail_width') }), { key: 'ArrowLeft' });

    expect(observed.detail.length).toBeGreaterThan(beforeResize);
    expect(observed.detail.slice(beforeResize)).toEqual(expect.arrayContaining([
      expect.objectContaining({ runId: 'run-A', nodeId: 'node-workflow-start', configSelected: true, runSelected: false })
    ]));
    expect(observed.detail.slice(beforeResize).every((commit) => commit.configSelected)).toBe(true);
    expect(screen.getByRole('tab', { name: i18nText('agentFlow', 'auto.settings') })).toHaveAttribute('aria-selected', 'true');
  });

  it('allows selecting and inspecting a node during the active run and keeps environment closed after terminal', async () => {
    const owner = await mountFrame();
    await click(screen.getByRole('button', { name: i18nText('agentFlow', 'auto.environment_variables') }));
    await screen.findByRole('region', { name: i18nText('agentFlow', 'auto.environment_variables') });
    const stream = await owner.beginRun();
    await act(async () => { stream.handlers.onEvent({ type: 'flow_started', run_id: 'run-active', status: 'running' }); });
    expect(screen.queryByTestId('agent-flow-editor-detail-dock')).not.toBeInTheDocument();

    await selectNode('start');

    expect(screen.getByTestId('agent-flow-editor-detail-dock')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: i18nText('agentFlow', 'auto.current_run') })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: i18nText('agentFlow', 'auto.settings') })).toHaveAttribute('aria-disabled', 'true');
    await owner.finishRun('run-active', stream);
    expect(screen.queryByRole('region', { name: i18nText('agentFlow', 'auto.environment_variables') })).not.toBeInTheDocument();
    expect(owner.start).toHaveBeenCalledTimes(1);
  });
});
