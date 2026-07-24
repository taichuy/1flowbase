import { Alert } from 'antd';
import { useEffect, useRef, useState } from 'react';

import {
  BlockUiLoadingShell,
  BlockUiRenderer,
  type BlockRendererActionEvent
} from '@1flowbase/block-renderer';
import {
  createJsBlockWorkerHost,
  type JsBlockWorkerFactory,
  type JsBlockWorkerHost,
  type JsBlockWorkerLike
} from '@1flowbase/page-runtime';

import {
  createDefaultJsBlockWorkerFactory,
  getDefaultJsBlockRuntimeFingerprint
} from '../../../shared/code-block/browser-worker';
import { type PublicLoginInstance } from '../api/session';
import {
  createPublicAuthRunRequest,
  dispatchPublicAuthApi
} from './public-auth-block-host';

interface PublicAuthSession {
  csrf_token: string;
  effective_display_role: string;
  current_workspace_id: string;
}

export interface PublicAuthBlockProps {
  instance: PublicLoginInstance;
  onAuthenticated: (session: PublicAuthSession) => void | Promise<void>;
  workerFactory?: JsBlockWorkerFactory;
}

type ViewState =
  | { status: 'loading' }
  | { status: 'ready'; view: unknown }
  | { status: 'failed'; message: string };

export function PublicAuthBlock({
  instance,
  onAuthenticated,
  workerFactory
}: PublicAuthBlockProps) {
  const [viewState, setViewState] = useState<ViewState>({ status: 'loading' });
  const hostRef = useRef<JsBlockWorkerHost | null>(null);
  const runSequenceRef = useRef(0);

  useEffect(() => {
    let active = true;
    const notify = () => {
      queueMicrotask(() => {
        if (!active) return;
        const requestId = `public-auth:${instance.id}:${runSequenceRef.current}`;
        const request = host.getState().requests[requestId];
        if (request?.result?.ok === true) {
          setViewState({ status: 'ready', view: request.result.view });
        } else if (request?.result?.ok === false) {
          setViewState({ status: 'failed', message: request.result.error.message });
        }
      });
    };
    const baseFactory = workerFactory ?? createDefaultJsBlockWorkerFactory();
    const host = createJsBlockWorkerHost({
      workerFactory: createNotifyingWorkerFactory(baseFactory, notify),
      runtimeFingerprint: getDefaultJsBlockRuntimeFingerprint(),
      effectBridge: {
        policy: { allowedEvents: [] },
        handlers: {
          interface: async (effect) => {
            const response = await dispatchPublicAuthApi(
              effect.method,
              effect.path,
              effect.request
            );
            if (
              (effect.path === '/api/public/auth/sign-in' ||
                effect.path === '/api/public/auth/sign-up') &&
              isPublicAuthSession(response)
            ) {
              await onAuthenticated(response);
            }
            return response;
          }
        }
      }
    });
    hostRef.current = host;
    runPublicAuthBlock(host, instance, runSequenceRef.current);

    return () => {
      active = false;
      host.dispose();
      hostRef.current = null;
      runSequenceRef.current = 0;
    };
  }, [instance, onAuthenticated, workerFactory]);

  const handleAction = (event: BlockRendererActionEvent) => {
    const host = hostRef.current;
    if (!host) return;
    runSequenceRef.current += 1;
    setViewState({ status: 'loading' });
    runPublicAuthBlock(host, instance, runSequenceRef.current, event);
  };

  if (viewState.status === 'loading') return <BlockUiLoadingShell />;
  if (viewState.status === 'failed') {
    return <Alert type="error" showIcon message={viewState.message} />;
  }
  return <BlockUiRenderer schema={viewState.view} onAction={handleAction} />;
}

function runPublicAuthBlock(
  host: JsBlockWorkerHost,
  instance: PublicLoginInstance,
  sequence: number,
  event?: BlockRendererActionEvent
): void {
  host.run(createPublicAuthRunRequest(instance, sequence, event));
}

function createNotifyingWorkerFactory(
  factory: JsBlockWorkerFactory,
  notify: () => void
): JsBlockWorkerFactory {
  return () => new NotifyingWorker(factory(), notify);
}

class NotifyingWorker implements JsBlockWorkerLike {
  constructor(
    private readonly worker: JsBlockWorkerLike,
    private readonly notify: () => void
  ) {}

  get onmessage() { return this.worker.onmessage ?? null; }
  set onmessage(handler) {
    this.worker.onmessage = handler
      ? (event) => { handler(event); this.notify(); }
      : null;
  }
  get onerror() { return this.worker.onerror ?? null; }
  set onerror(handler) {
    this.worker.onerror = handler
      ? (event) => { handler(event); this.notify(); }
      : null;
  }
  get onmessageerror() { return this.worker.onmessageerror ?? null; }
  set onmessageerror(handler) {
    this.worker.onmessageerror = handler
      ? (event) => { handler(event); this.notify(); }
      : null;
  }
  postMessage(message: Parameters<JsBlockWorkerLike['postMessage']>[0]) {
    this.worker.postMessage(message);
  }
  terminate() { this.worker.terminate(); }
}

function isPublicAuthSession(value: unknown): value is PublicAuthSession {
  return isRecord(value) &&
    typeof value.csrf_token === 'string' &&
    typeof value.effective_display_role === 'string' &&
    typeof value.current_workspace_id === 'string';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
