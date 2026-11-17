import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppThemeProvider } from '@1flowbase/ui/app-theme-provider';
import { createDefaultWorkflowDocument, type FlowAuthoringDocument } from '@1flowbase/flow-schema';
import type { ConsoleApplicationApiMapping, ConsoleApplicationOrchestrationState } from '@1flowbase/api-client';
import { useLayoutEffect, useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { resetAuthStore } from '../../../state/auth-store';
import { authenticateEditor } from '../../agent-flow/_tests/fixtures/editor-auth';
import * as applications from '../../applications/api/applications';
import * as publicApi from '../../applications/api/public-api';
import * as catalog from '../../agent-flow/api/application-node-catalog';
import * as orchestration from '../../agent-flow/api/orchestration';
import { WorkflowEditorPage } from '../pages/WorkflowEditorPage';
import type { WorkflowTriggerContext } from '../lib/trigger-context';

interface CanvasCommit {
  applicationId: string;
  document: FlowAuthoringDocument;
  draftMeta: { draftId: string; flowId: string; updatedAt: string };
  dirty: boolean;
}
const observed = vi.hoisted(() => ({ commits: [] as CanvasCommit[] }));

// Keep the real page, Frame, Kernel, Provider and draft-sync owners.
// Replace only canvas rendering with a consumer of the same real store.
vi.mock('../../agent-flow/components/editor/AgentFlowCanvas', async () => {
  const { useLayoutEffect } = await import('react');
  const { useAgentFlowEditorStore } = await import('../../agent-flow/store/editor/provider');
  return {
    AgentFlowCanvas: ({ workflowTriggerContext }: { workflowTriggerContext: WorkflowTriggerContext }) => {
      const document = useAgentFlowEditorStore((state) => state.workingDocument);
      const draftMeta = useAgentFlowEditorStore((state) => state.draftMeta);
      const dirty = useAgentFlowEditorStore((state) => state.isDirty);
      const setWorkingDocument = useAgentFlowEditorStore((state) => state.setWorkingDocument);
      useLayoutEffect(() => {
        observed.commits.push({
          applicationId: workflowTriggerContext.applicationId,
          document: structuredClone(document),
          draftMeta: { ...draftMeta },
          dirty
        });
      });
      return (
        <label>
          Workflow title
          <input
            aria-label="Workflow title"
            value={document.meta.name}
            onChange={(event) => setWorkingDocument((current) => ({
              ...current,
              meta: { ...current.meta, name: event.target.value }
            }))}
          />
        </label>
      );
    }
  };
});

const mapping: ConsoleApplicationApiMapping = {
  input: {
    query_target: 'node-workflow-start',
    model_target: null,
    inputs_target: null,
    history_target: null,
    attachments_target: null
  },
  output: {
    answer_selector: null,
    usage_selector: null,
    files_selector: null,
    error_selector: null
  }
};
function initialState(suffix: string): ConsoleApplicationOrchestrationState {
  const flowId = `flow-${suffix}`;
  const document = createDefaultWorkflowDocument({ flowId });
  document.meta.name = `Workflow ${suffix}`;
  return {
    flow_id: flowId,
    messages: [],
    draft: {
      id: `draft-${suffix}`,
      flow_id: flowId,
      updated_at: '2026-11-17T00:00:00Z',
      document
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
const clients: QueryClient[] = [];
function mountWarmEditor() {
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
  for (const [applicationId, state] of states) {
    client.setQueryData(orchestration.orchestrationQueryKey(applicationId), state);
    client.setQueryData(catalog.applicationNodeCatalogQueryKey(applicationId), { nodes: [] });
    client.setQueryData(applications.applicationEnvironmentVariablesQueryKey(applicationId), []);
    client.setQueryData(publicApi.applicationApiMappingQueryKey(applicationId), mapping);
    client.setQueryData(publicApi.workflowScheduleTriggerQueryKey(applicationId), null);
    client.setQueryData(publicApi.applicationApiPublicationQueryKey(applicationId), null);
  }
  const fetchState = vi.spyOn(orchestration, 'fetchOrchestrationState').mockImplementation(
    async (applicationId) => states.get(applicationId)!
  );
  vi.spyOn(catalog, 'fetchApplicationNodeCatalog').mockResolvedValue({ nodes: [] });
  vi.spyOn(applications, 'fetchApplicationEnvironmentVariables').mockResolvedValue([]);
  vi.spyOn(publicApi, 'fetchApplicationApiMapping').mockResolvedValue(mapping);
  vi.spyOn(publicApi, 'fetchWorkflowScheduleTrigger').mockResolvedValue(null);
  vi.spyOn(publicApi, 'fetchApplicationApiPublication').mockResolvedValue(null);
  vi.spyOn(orchestration, 'analyzeDocument').mockResolvedValue(stateA.analysis);
  const save = vi.spyOn(orchestration, 'saveDraft').mockImplementation(
    async (applicationId) => states.get(applicationId)!
  );
  const pageCommits: Array<{ applicationId: string; instance: object }> = [];
  function PageOwner({ applicationId }: { applicationId: string }) {
    const instance = useRef({});
    useLayoutEffect(() => {
      pageCommits.push({ applicationId, instance: instance.current });
    });
    return (
      <WorkflowEditorPage
        applicationId={applicationId}
        applicationName={applicationId}
        workflowTriggerType="schedule"
      />
    );
  }
  function tree(applicationId: string) {
    return (
      <AppThemeProvider>
        <AppI18nProvider>
          <QueryClientProvider client={client}>
            <PageOwner applicationId={applicationId} />
          </QueryClientProvider>
        </AppI18nProvider>
      </AppThemeProvider>
    );
  }
  const view = render(tree('app-A'));
  return {
    client, stateA, stateB, save, fetchState, pageCommits,
    rerender(applicationId: string) { view.rerender(tree(applicationId)); }
  };
}
async function editTitle(name: string) {
  fireEvent.change(screen.getByRole('textbox', { name: 'Workflow title' }), {
    target: { value: name }
  });
  await act(async () => vi.advanceTimersByTimeAsync(0));
}
beforeEach(() => {
  vi.useFakeTimers();
  observed.commits.length = 0;
  resetAuthStore();
  authenticateEditor(['save_draft'], 'member');
});
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.useRealTimers();
  vi.restoreAllMocks();
  resetAuthStore();
});

describe('Workflow editor application session provenance', () => {
  it('commits the warm B document and draft identity when the same page switches from A to B', async () => {
    const owner = mountWarmEditor();
    await act(async () => vi.advanceTimersByTimeAsync(0));
    expect(screen.getByRole('textbox', { name: 'Workflow title' })).toHaveValue('Workflow A');
    expect(owner.fetchState).not.toHaveBeenCalled();
    const pageInstance = owner.pageCommits.at(-1)!.instance;

    owner.rerender('app-B');
    await act(async () => vi.advanceTimersByTimeAsync(0));

    expect(screen.getByRole('region', { name: 'app-B workflow editor' })).toBeInTheDocument();
    expect(owner.client.getQueryData(orchestration.orchestrationQueryKey('app-B'))).toEqual(owner.stateB);
    expect(owner.pageCommits.at(-1)!.instance).toBe(pageInstance);
    const firstB = observed.commits.find((commit) => commit.applicationId === 'app-B')!;
    expect(firstB).toMatchObject({
      applicationId: 'app-B',
      document: owner.stateB.draft.document,
      draftMeta: {
        draftId: owner.stateB.draft.id,
        flowId: owner.stateB.flow_id,
        updatedAt: owner.stateB.draft.updated_at
      },
      dirty: false
    });
    expect(screen.getByRole('textbox', { name: 'Workflow title' })).toHaveValue('Workflow B');
    expect(owner.fetchState).not.toHaveBeenCalled();
    expect(owner.save).not.toHaveBeenCalled();
  });

  it('does not send dirty A through B autosave and then saves an actual B edit to B', async () => {
    const owner = mountWarmEditor();
    await editTitle('A unsaved edit');
    const dirtyA = edited(owner.stateA.draft.document, 'A unsaved edit');
    expect(observed.commits.at(-1)).toMatchObject({ applicationId: 'app-A', document: dirtyA, dirty: true });
    expect(owner.save).not.toHaveBeenCalled();

    owner.rerender('app-B');
    await act(async () => vi.advanceTimersByTimeAsync(30_000));

    expect(owner.save).not.toHaveBeenCalledWith(
      'app-B', expect.objectContaining({ document: dirtyA }), 'csrf-token'
    );
    const callsBeforeBEdit = owner.save.mock.calls.length;
    await editTitle('B unsaved edit');
    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(owner.save.mock.calls.slice(callsBeforeBEdit)).toEqual([
      ['app-B', {
        document: edited(owner.stateB.draft.document, 'B unsaved edit'),
        change_kind: 'logical',
        summary: expect.any(String)
      }, 'csrf-token']
    ]);
  });

  it('preserves the same application dirty document when its query returns a refreshed initial state', async () => {
    const owner = mountWarmEditor();
    await editTitle('A unsaved edit');
    const dirtyA = edited(owner.stateA.draft.document, 'A unsaved edit');
    const beforeRefresh = observed.commits.length;
    const refreshed = {
      ...owner.stateA,
      draft: {
        ...owner.stateA.draft,
        updated_at: '2026-11-17T00:01:00Z',
        document: edited(owner.stateA.draft.document, 'A refreshed server title')
      }
    };
    await act(async () => {
      owner.client.setQueryData(orchestration.orchestrationQueryKey('app-A'), refreshed);
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(owner.client.getQueryData(orchestration.orchestrationQueryKey('app-A'))).toEqual(refreshed);
    expect(observed.commits.length).toBeGreaterThan(beforeRefresh);
    expect(observed.commits.at(-1)).toMatchObject({ applicationId: 'app-A', document: dirtyA, dirty: true });
    expect(screen.getByRole('textbox', { name: 'Workflow title' })).toHaveValue('A unsaved edit');
    expect(owner.save).not.toHaveBeenCalled();
  });
});
