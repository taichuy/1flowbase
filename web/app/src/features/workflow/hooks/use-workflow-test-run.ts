import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ApiClientError,
  type ConsoleFlowDebugStreamEvent
} from '@1flowbase/api-client';
import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { useAuthStore } from '../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../state/console-operation-access';
import {
  fetchApplicationRunDebugSnapshot,
  startFlowDebugRunStream
} from '../../agent-flow/api/runtime';
import { buildWorkflowTestRunInput } from '../lib/test-run';
import {
  applyWorkflowRunEvent,
  emptyWorkflowRunState,
  isWorkflowRunActive,
  workflowNodeRunStatuses,
  workflowRunSnapshot
} from '../lib/run-state';

function createRunObservation() {
  const observation = {
    disposed: false,
    controller: null as AbortController | null,
    timer: null as ReturnType<typeof setTimeout> | null,
    resume: null as (() => void) | null,
    runId: null as string | null,
    terminal: false,
    async pause(ms: number) {
      if (ms <= 0 || observation.disposed) return;
      await new Promise<void>((resolve) => {
        observation.resume = resolve;
        observation.timer = setTimeout(resolve, ms);
      });
      observation.timer = null;
      observation.resume = null;
    },
    dispose() {
      observation.disposed = true;
      observation.controller?.abort();
      if (observation.timer) clearTimeout(observation.timer);
      observation.resume?.();
    }
  };
  return observation;
}

export function useWorkflowTestRun(
  applicationId: string,
  document: FlowAuthoringDocument
) {
  const [state, setState] = useState(emptyWorkflowRunState);
  const observationRef = useRef<ReturnType<typeof createRunObservation> | null>(
    null
  );
  const can = useConsoleOperationAccess();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const active = isWorkflowRunActive(state);
  const canRun = can('start_flow_debug_run_stream') && Boolean(csrfToken);
  const nodeStatuses = useMemo(() => workflowNodeRunStatuses(state), [state]);

  useEffect(() => {
    setState(emptyWorkflowRunState());
    return () => {
      observationRef.current?.dispose();
      observationRef.current = null;
    };
  }, [applicationId]);

  const start = useCallback(
    async (inputs: Record<string, unknown>) => {
      if (!canRun || !csrfToken || observationRef.current) return;
      let runInput: ReturnType<typeof buildWorkflowTestRunInput>;
      try {
        runInput = buildWorkflowTestRunInput({ document, inputs });
      } catch (error) {
        setState({
          ...emptyWorkflowRunState(),
          status: 'failed',
          failure: error instanceof Error ? error.message : String(error)
        });
        return;
      }
      const observation = createRunObservation();
      observationRef.current = observation;
      setState({ ...emptyWorkflowRunState(), status: 'starting' });
      const isCurrent = () =>
        !observation.disposed && observationRef.current === observation;
      let presentation = Promise.resolve();
      const visibleSince = new Map<string, number>();
      const shownNodes = new Set<string>();
      const seenEvents = new Set<string>();
      const enqueue = (event: ConsoleFlowDebugStreamEvent) => {
        if (!isCurrent()) return;
        if ('run_id' in event && event.run_id) observation.runId = event.run_id;
        if (
          [
            'flow_finished',
            'flow_incomplete',
            'flow_failed',
            'flow_cancelled'
          ].includes(event.type)
        )
          observation.terminal = true;
        if (
          ![
            'flow_accepted',
            'flow_started',
            'node_started',
            'node_finished',
            'flow_finished',
            'flow_incomplete',
            'flow_failed',
            'flow_cancelled',
            'waiting_human',
            'waiting_callback'
          ].includes(event.type)
        )
          return;
        const key =
          event.event_id ??
          (event.type === 'node_started' || event.type === 'node_finished'
            ? `${event.node_run_id}:${event.type}`
            : null);
        if (key && seenEvents.has(key)) return;
        if (key) seenEvents.add(key);
        presentation = presentation.then(async () => {
          if (!isCurrent()) return;
          if (event.type === 'node_finished') {
            const since = visibleSince.get(event.node_run_id);
            if (since !== undefined)
              await observation.pause(280 - (Date.now() - since));
          }
          if (!isCurrent()) return;
          if (event.type === 'node_started' && !shownNodes.has(event.node_id)) {
            visibleSince.set(event.node_run_id, Date.now());
            shownNodes.add(event.node_id);
          }
          setState((current) => applyWorkflowRunEvent(current, event));
          if (
            event.type === 'flow_finished' ||
            event.type === 'flow_incomplete' ||
            event.type === 'flow_failed' ||
            event.type === 'flow_cancelled'
          )
            observation.controller?.abort();
        });
      };

      try {
        await startFlowDebugRunStream(
          applicationId,
          { ...runInput, document },
          csrfToken,
          {
            getAbortController(controller) {
              observation.controller = controller;
              if (!isCurrent()) controller.abort();
            },
            onEvent: enqueue
          }
        );
      } catch (error) {
        if (!isCurrent()) return;
        if (!observation.runId && error instanceof ApiClientError) {
          setState((current) => ({
            ...current,
            status: 'failed',
            failure: error.message
          }));
          observationRef.current = null;
          return;
        }
        // Transport failure is not a business failure and never resubmits the run.
        if (!observation.terminal)
          setState((current) => ({ ...current, connectionLost: true }));
      }
      await presentation;
      if (!isCurrent()) return;
      if (observation.terminal) {
        observationRef.current = null;
        return;
      }
      // Stream closure without a terminal event: observe the accepted run by ID.
      if (!observation.runId) {
        setState((current) => ({
          ...current,
          status: 'unconfirmed',
          connectionLost: true
        }));
        return;
      }
      while (isCurrent()) {
        try {
          const detail = await fetchApplicationRunDebugSnapshot(
            applicationId,
            observation.runId
          );
          if (!isCurrent()) return;
          const snapshot = workflowRunSnapshot(detail);
          setState(snapshot);
          if (!isWorkflowRunActive(snapshot)) {
            observationRef.current = null;
            return;
          }
        } catch {
          if (isCurrent())
            setState((current) => ({ ...current, connectionLost: true }));
        }
        await observation.pause(1500);
      }
    },
    [applicationId, document, canRun, csrfToken]
  );

  return {
    state,
    active,
    canRun,
    nodeStatuses,
    start,
    clear: () => {
      if (!active) setState(emptyWorkflowRunState());
    }
  };
}
