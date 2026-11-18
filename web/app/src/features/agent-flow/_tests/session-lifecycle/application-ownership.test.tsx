import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';
import { createDefaultAgentFlowDocument, type FlowAuthoringDocument } from '@1flowbase/flow-schema';
import type { ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import { useLayoutEffect, useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AppI18nProvider } from '../../../../app/AppI18nProvider';
import { resetAuthStore } from '../../../../state/auth-store';
import { authenticateEditor } from '../fixtures/editor-auth';
import * as orchestration from '../../api/orchestration';
import * as runtime from '../../api/runtime';
import { AgentFlowEditorAssembly } from '../../components/editor/AgentFlowEditorAssembly';
import { useAgentFlowEditorStore } from '../../store/editor/provider';

// The actual Frame, draft-sync hook, Kernel and store remain in the tree.
vi.mock('../../components/editor/AgentFlowCanvas', () => ({ AgentFlowCanvas: () => null }));

interface Commit {
  applicationId: string;
  document: FlowAuthoringDocument;
  draftMeta: { draftId: string; flowId: string; updatedAt: string };
  dirty: boolean;
}
const commits: Commit[] = [];
const clients: QueryClient[] = [];
const releasePending: Array<() => void> = [];
function initialState(suffix: string): ConsoleApplicationOrchestrationState {
  const flowId = `flow-${suffix}`;
  const document = createDefaultAgentFlowDocument({ flowId });
  document.meta.name = `Agent flow ${suffix}`;
  return {
    flow_id: flowId,
    messages: [],
    draft: {
      id: `draft-${suffix}`, flow_id: flowId,
      updated_at: '2026-11-17T00:00:00Z', document
    },
    analysis: { diagnostics: [], node_outputs: [], variable_catalogs: [] },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}
function edited(document: FlowAuthoringDocument, name: string) {
  return { ...document, meta: { ...document.meta, name } };
}
function StoreConsumer({ applicationId }: { applicationId: string }) {
  const document = useAgentFlowEditorStore((state) => state.workingDocument);
  const draftMeta = useAgentFlowEditorStore((state) => state.draftMeta);
  const dirty = useAgentFlowEditorStore((state) => state.isDirty);
  const setWorkingDocument = useAgentFlowEditorStore((state) => state.setWorkingDocument);
  useLayoutEffect(() => {
    commits.push({ applicationId, document: structuredClone(document), draftMeta: { ...draftMeta }, dirty });
  });
  return (
    <label>
      Agent flow title
      <input aria-label="Agent flow title" value={document.meta.name}
        onChange={(event) => setWorkingDocument((current) => edited(current, event.target.value))}
      />
    </label>
  );
}
function mountEditor() {
  const stateA = initialState('A');
  const stateB = initialState('B');
  const states = new Map([['app-A', stateA], ['app-B', stateB]]);
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity, refetchOnWindowFocus: false },
      mutations: { retry: false }
    }
  });
  clients.push(client);
  vi.spyOn(orchestration, 'analyzeDocument').mockResolvedValue(stateA.analysis);
  vi.spyOn(runtime, 'fetchDebugVariableSnapshot').mockResolvedValue({ variable_cache: {} });
  const save = vi.spyOn(orchestration, 'saveDraft').mockImplementation(async (applicationId) => states.get(applicationId)!);
  const ownerCommits: Array<{ applicationId: string; instance: object }> = [];
  function Owner({ applicationId, state }: { applicationId: string; state: ConsoleApplicationOrchestrationState }) {
    const instance = useRef({});
    useLayoutEffect(() => { ownerCommits.push({ applicationId, instance: instance.current }); });
    return (
      <AgentFlowEditorAssembly
        applicationId={applicationId}
        applicationName={applicationId}
        initialState={state}
        topSlot={<StoreConsumer applicationId={applicationId} />}
      />
    );
  }
  function tree(applicationId: string, state: ConsoleApplicationOrchestrationState) {
    return (
      <AppThemeProvider><AppI18nProvider><QueryClientProvider client={client}>
        <Owner applicationId={applicationId} state={state} />
      </QueryClientProvider></AppI18nProvider></AppThemeProvider>
    );
  }
  const view = render(tree('app-A', stateA));
  return {
    stateA, stateB, save, ownerCommits,
    rerender(applicationId: string, state = states.get(applicationId)!) { view.rerender(tree(applicationId, state)); }
  };
}
async function editTitle(name: string) {
  fireEvent.change(screen.getByRole('textbox', { name: 'Agent flow title' }), { target: { value: name } });
  await act(async () => vi.advanceTimersByTimeAsync(0));
}
beforeEach(() => {
  vi.useFakeTimers();
  commits.length = 0;
  resetAuthStore();
  authenticateEditor(['save_draft'], 'member');
});
afterEach(async () => {
  cleanup();
  await act(async () => releasePending.splice(0).forEach((release) => release()));
  clients.splice(0).forEach((client) => client.clear());
  vi.useRealTimers();
  vi.restoreAllMocks();
  resetAuthStore();
});

describe('AgentFlowEditorAssembly application ownership', () => {
  it('commits B document and draft identity when the same assembly owner switches from A', async () => {
    const owner = mountEditor();
    await act(async () => vi.advanceTimersByTimeAsync(0));
    const instance = owner.ownerCommits.at(-1)!.instance;
    expect(screen.getByRole('textbox', { name: 'Agent flow title' })).toHaveValue('Agent flow A');

    owner.rerender('app-B');
    await act(async () => vi.advanceTimersByTimeAsync(0));

    expect(owner.ownerCommits.at(-1)!.instance).toBe(instance);
    expect(screen.getByRole('region', { name: 'app-B editor' })).toBeInTheDocument();
    const firstB = commits.find((commit) => commit.applicationId === 'app-B')!;
    expect(firstB).toMatchObject({
      applicationId: 'app-B', document: owner.stateB.draft.document,
      draftMeta: { draftId: owner.stateB.draft.id, flowId: owner.stateB.flow_id, updatedAt: owner.stateB.draft.updated_at },
      dirty: false
    });
    expect(screen.getByRole('textbox', { name: 'Agent flow title' })).toHaveValue('Agent flow B');
    expect(owner.save).not.toHaveBeenCalled();
  });

  it('does not send dirty A to B and saves a later B edit through the real Frame autosave', async () => {
    const owner = mountEditor();
    await editTitle('A unsaved edit');
    const dirtyA = edited(owner.stateA.draft.document, 'A unsaved edit');
    expect(commits.at(-1)).toMatchObject({ applicationId: 'app-A', document: dirtyA, dirty: true });
    owner.rerender('app-B');
    await act(async () => vi.advanceTimersByTimeAsync(30_000));

    expect(owner.save).not.toHaveBeenCalledWith('app-B', expect.objectContaining({ document: dirtyA }), 'csrf-token');
    const beforeBEdit = owner.save.mock.calls.length;
    await editTitle('B unsaved edit');
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(owner.save.mock.calls.slice(beforeBEdit)).toEqual([
      ['app-B', { document: edited(owner.stateB.draft.document, 'B unsaved edit'), change_kind: 'logical', summary: expect.any(String) }, 'csrf-token']
    ]);
  });

  it('retains same-application dirty work when initialState refreshes', async () => {
    const owner = mountEditor();
    await editTitle('A unsaved edit');
    const refreshed = {
      ...owner.stateA,
      draft: { ...owner.stateA.draft, updated_at: '2026-11-17T00:01:00Z', document: edited(owner.stateA.draft.document, 'A server refresh') }
    };
    const previousCommits = commits.length;
    owner.rerender('app-A', refreshed);
    await act(async () => vi.advanceTimersByTimeAsync(0));

    expect(commits.length).toBeGreaterThan(previousCommits);
    expect(commits.at(-1)).toMatchObject({ applicationId: 'app-A', document: edited(owner.stateA.draft.document, 'A unsaved edit'), dirty: true });
    expect(screen.getByRole('textbox', { name: 'Agent flow title' })).toHaveValue('A unsaved edit');
    expect(owner.save).not.toHaveBeenCalled();
  });

  it('allows an already dispatched A save to finish for A without replacing the B session', async () => {
    const owner = mountEditor();
    let resolveA!: (state: ConsoleApplicationOrchestrationState) => void;
    const pendingA = new Promise<ConsoleApplicationOrchestrationState>((resolve) => { resolveA = resolve; });
    releasePending.push(() => resolveA(owner.stateA));
    owner.save.mockReturnValueOnce(pendingA);
    await editTitle('A submitted edit');
    const submittedA = edited(owner.stateA.draft.document, 'A submitted edit');
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(owner.save).toHaveBeenCalledExactlyOnceWith(
      'app-A', { document: submittedA, change_kind: 'logical', summary: expect.any(String) }, 'csrf-token'
    );

    owner.rerender('app-B');
    await act(async () => {
      resolveA({ ...owner.stateA, draft: { ...owner.stateA.draft, document: submittedA } });
    });

    expect(commits.at(-1)).toMatchObject({
      applicationId: 'app-B', document: owner.stateB.draft.document,
      draftMeta: { draftId: owner.stateB.draft.id, flowId: owner.stateB.flow_id }, dirty: false
    });
    expect(screen.getByRole('textbox', { name: 'Agent flow title' })).toHaveValue('Agent flow B');
    expect(owner.save).toHaveBeenCalledTimes(1);
  });
});
