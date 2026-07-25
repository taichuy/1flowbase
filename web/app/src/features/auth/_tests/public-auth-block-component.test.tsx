import { render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { JsBlockWorkerLike } from '@1flowbase/page-runtime';

import { PublicAuthBlock } from '../components/PublicAuthBlock';

class FailingBlockWorker implements JsBlockWorkerLike {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  onmessageerror: ((event: { message?: string }) => void) | null = null;

  postMessage(input: unknown): void {
    const message = input as { type?: string; request?: { requestId?: string } };
    if (message.type === 'init') {
      queueMicrotask(() => this.onmessage?.({
        data: { direction: 'worker_to_host', type: 'ready' }
      }));
    }
    if (message.type === 'run') {
      queueMicrotask(() => this.onmessage?.({
        data: {
          direction: 'worker_to_host',
          type: 'error',
          requestId: message.request?.requestId,
          kind: 'main_failed',
          message: 'Authenticator Block failed',
          errors: [{
            code: 'runtime_error',
            path: 'runtime.main',
            message: 'Authenticator Block failed'
          }]
        }
      }));
    }
  }

  terminate(): void {}
}

describe('PublicAuthBlock error boundary', () => {
  test('isolates an invalid authenticator Block as a controlled alert', async () => {
    const onAuthenticated = vi.fn();
    render(
      <PublicAuthBlock
        instance={{
          id: 'auth-invalid',
          auth_type: 'fixture.invalid',
          title: 'Invalid authenticator',
          sort_order: 0,
          public_ui_block: 'export default { main };',
          public_variables: {}
        }}
        onAuthenticated={onAuthenticated}
        workerFactory={() => new FailingBlockWorker()}
      />
    );

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Authenticator Block failed'
    );
    expect(onAuthenticated).not.toHaveBeenCalled();
  });
});
