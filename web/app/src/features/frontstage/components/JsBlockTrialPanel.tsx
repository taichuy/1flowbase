import {
  hashJsBlockDraft,
  type JsBlockHostEffectHandlers
} from '@1flowbase/page-runtime';
import {
  BlockUiLoadingShell,
  type BlockRendererActionEvent
} from '@1flowbase/block-renderer';
import { Modal } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import {
  createFrontstageRestrictedBlockRuntimeSession,
  type FrontstageRestrictedBlockRuntimeSession
} from '../lib/frontstage-restricted-block-runtime-host';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import type { FrontstageBlockInstance } from '../lib/page-document';
import {
  createRestrictedBlockRunPlan,
  type RestrictedBlockLoaderLimits
} from '../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';
import { RestrictedBlockRuntimePreview } from './RestrictedBlockRuntimePreview';
import { JsBlockPreviewConsole } from './JsBlockPreviewConsole';

export interface JsBlockTrialPanelProps {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  code: string;
  contextSnapshot: Record<string, unknown>;
  createRunInputs?: (
    event?: BlockRendererActionEvent
  ) => Record<string, unknown>;
  handlers?: JsBlockHostEffectHandlers;
  onPrepareDraftRun?: (input: {
    blockId: string;
    runId: string;
    draftHash: string;
    confirmWrite: () => Promise<boolean>;
  }) => Promise<void>;
  onRevokeDraftRun?: (runId: string) => void;
  limits: RestrictedBlockLoaderLimits;
  revision: string;
  runtimeSessionFactory?: typeof createFrontstageRestrictedBlockRuntimeSession;
}

export function JsBlockTrialPanel({
  block,
  catalogEntry,
  code,
  contextSnapshot,
  createRunInputs,
  handlers,
  onPrepareDraftRun,
  onRevokeDraftRun,
  limits,
  revision,
  runtimeSessionFactory = createFrontstageRestrictedBlockRuntimeSession
}: JsBlockTrialPanelProps) {
  const sessionRef = useRef<FrontstageRestrictedBlockRuntimeSession | null>(
    null
  );
  const unsubscribeRef = useRef<(() => void) | null>(null);
  const [snapshot, setSnapshot] =
    useState<RestrictedBlockRuntimeHostSnapshot | null>(null);
  const stop = useCallback(() => {
    unsubscribeRef.current?.();
    unsubscribeRef.current = null;
    const session = sessionRef.current;
    sessionRef.current = null;
    if (session) {
      onRevokeDraftRun?.(session.getSnapshot().requestId);
      setSnapshot(session.dispose());
    }
  }, [onRevokeDraftRun]);

  useEffect(() => stop, [stop]);

  const run = useCallback(
    async (event?: BlockRendererActionEvent) => {
      stop();
      if (!catalogEntry) return;
      const plan = createRestrictedBlockRunPlan({
        block,
        catalogEntry,
        code,
        contextSnapshot,
        inputs: createRunInputs?.(event) ?? {},
        limits
      });
      if (!plan.ok) {
        setSnapshot(createRejectedSnapshot(block.id, plan.message));
        return;
      }
      const runId = `draft:${block.id}:${Date.now().toString(36)}`;
      try {
        await onPrepareDraftRun?.({
          blockId: block.id,
          runId,
          draftHash: hashJsBlockDraft(code),
          confirmWrite: confirmWriteRun
        });
      } catch (error) {
        setSnapshot(
          createRejectedSnapshot(
            block.id,
            error instanceof Error ? error.message : String(error)
          )
        );
        return;
      }
      const runPlan = {
        ...plan,
        request: { ...plan.request, requestId: runId }
      };
      const session = runtimeSessionFactory({
        runPlan,
        handlers
      });
      sessionRef.current = session;
      unsubscribeRef.current = session.subscribe((next) => {
        setSnapshot(next);
        if (next.status !== 'running' && next.status !== 'idle') {
          onRevokeDraftRun?.(runId);
        }
      });
      setSnapshot(session.run());
    },
    [
      block,
      catalogEntry,
      code,
      contextSnapshot,
      createRunInputs,
      handlers,
      limits,
      onPrepareDraftRun,
      onRevokeDraftRun,
      runtimeSessionFactory,
      stop
    ]
  );
  const runRef = useRef(run);

  useEffect(() => {
    runRef.current = run;
  }, [run]);

  useEffect(() => {
    void runRef.current();
  }, [revision]);

  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
        event.preventDefault();
        void run();
      }
    };
    window.addEventListener('keydown', keydown);
    return () => window.removeEventListener('keydown', keydown);
  }, [run]);

  return (
    <JsBlockPreviewConsole
      snapshot={snapshot}
      preview={
        snapshot ? (
          <RestrictedBlockRuntimePreview
            snapshot={snapshot}
            onAction={createRunInputs ? (event) => void run(event) : undefined}
          />
        ) : (
          <BlockUiLoadingShell />
        )
      }
    />
  );
}

function confirmWriteRun(): Promise<boolean> {
  return new Promise((resolve) => {
    Modal.confirm({
      title: i18nText('frontstage', 'auto.confirm_write_run'),
      content: i18nText('frontstage', 'auto.confirm_write_run_description'),
      onOk: () => resolve(true),
      onCancel: () => resolve(false)
    });
  });
}

function createRejectedSnapshot(
  blockId: string,
  message: string
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'failed',
    requestId: `draft:${blockId}:rejected`,
    blockId,
    schemaValidationOptions: {},
    error: {
      kind: 'runtime_error',
      message,
      errors: [{ code: 'runtime_error', path: 'runPlan', message }]
    },
    logs: [],
    effects: [],
    rejections: [],
    interfaceCalls: []
  };
}
