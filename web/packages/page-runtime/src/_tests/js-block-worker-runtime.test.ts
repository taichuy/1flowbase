import { describe, expect, test } from 'vitest';

import {
  createJsBlockRuntimeSession,
  reduceJsBlockRuntimeSession,
  type JsBlockRunRequest,
  type JsBlockRuntimeSessionState
} from '../index';

const validSource = `
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main() {
  return {
    view: Text({ children: 'Ready' }),
    outputs: {}
  };
}

export default {
  main
};
`;

function createRunRequest(
  overrides: Partial<JsBlockRunRequest> = {}
): JsBlockRunRequest {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    source: validSource,
    props: { label: 'Ready' },
    state: { count: 1 },
    contextSnapshot: {
      applicationId: 'app-1',
      pageId: 'page-1',
      locale: 'en-US'
    },
    limits: {
      timeoutMs: 1000,
      maxRenderDepth: 8,
      maxRenderNodes: 250
    },
    ...overrides
  };
}

function run(
  state: JsBlockRuntimeSessionState,
  request: JsBlockRunRequest
): JsBlockRuntimeSessionState {
  return reduceJsBlockRuntimeSession(state, {
    direction: 'host_to_worker',
    type: 'run',
    request
  });
}

function completedMessage(requestId: string) {
  return {
    direction: 'worker_to_host',
    type: 'completed',
    requestId,
    view: {
      primitive: 'Text',
      props: { children: 'Ready' }
    },
    outputs: {}
  };
}

describe('JS block worker runtime protocol state machine', () => {
  test('moves a valid run request from pending to ready after rendered schema validation', () => {
    const pending = run(createJsBlockRuntimeSession(), createRunRequest());

    expect(pending.currentRequestId).toBe('request-1');
    expect(pending.requests['request-1']).toMatchObject({
      requestId: 'request-1',
      blockId: 'block-1',
      status: 'pending'
    });

    const ready = reduceJsBlockRuntimeSession(pending, {
      direction: 'worker_to_host',
      type: 'completed',
      outputs: {},
      requestId: 'request-1',
      view: {
        primitive: 'Text',
        props: { children: 'Ready' }
      }
    });

    expect(ready.requests['request-1']).toMatchObject({
      requestId: 'request-1',
      status: 'ready',
      result: {
        ok: true,
        requestId: 'request-1',
        view: {
          primitive: 'Text',
          props: { children: 'Ready' }
        }
      }
    });
  });

  test('maps source policy failures into a stable run result without executing source', () => {
    const state = run(
      createJsBlockRuntimeSession(),
      createRunRequest({
        source: 'window.location.href;'
      })
    );

    expect(state.requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        requestId: 'request-1',
        error: {
          kind: 'source_policy_failed',
          errors: [
            {
              code: 'transform_failed',
              path: 'source.identifiers.window'
            }
          ]
        }
      }
    });
  });

  test('accepts the typed TSX BlockModule contract during host preflight', () => {
    const state = run(
      createJsBlockRuntimeSession(),
      createRunRequest({
        source: `
import type { BlockModule, BlockResult } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main(): Promise<BlockResult> {
  return { view: <Text>Ready</Text>, outputs: {} };
}

export default { main } satisfies BlockModule;
`,
        allowedImports: [
          '@1flowbase/block-sdk',
          '@1flowbase/block-renderer/antd-facade'
        ]
      })
    );

    expect(state.requests['request-1']).toMatchObject({
      status: 'pending',
      phase: 'queued',
      compiledSource: { ok: true }
    });
  });

  test('maps source transform failures into a stable run result before sending source to a worker', () => {
    const state = run(
      createJsBlockRuntimeSession(),
      createRunRequest({
        source: `
const block = {
  async main() {
    return { view: { primitive: 'Text' }, outputs: {} };
  }
};
`
      })
    );

    expect(state.currentRequestId).toBeUndefined();
    expect(state.requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        requestId: 'request-1',
        error: {
          kind: 'source_policy_failed',
          errors: [
            {
              code: 'transform_failed',
              path: 'source.defaultExport'
            }
          ]
        }
      }
    });
  });

  test('rejects late rendered messages after source policy failure without overwriting the failure result', () => {
    const failed = run(
      createJsBlockRuntimeSession(),
      createRunRequest({
        source: 'window.location.href;'
      })
    );
    const failureResult = failed.requests['request-1']?.result;

    expect(failed.currentRequestId).toBeUndefined();

    const afterLateRendered = reduceJsBlockRuntimeSession(
      failed,
      completedMessage('request-1')
    );

    expect(afterLateRendered.requests['request-1']).toMatchObject({
      status: 'failed',
      result: failureResult
    });
    expect(afterLateRendered.rejections.at(-1)).toMatchObject({
      code: 'request_not_pending',
      requestId: 'request-1'
    });
  });

  test('maps invalid rendered schemas into a stable schema_invalid run result', () => {
    const pending = run(createJsBlockRuntimeSession(), createRunRequest());

    const failed = reduceJsBlockRuntimeSession(pending, {
      direction: 'worker_to_host',
      type: 'completed',
      outputs: {},
      requestId: 'request-1',
      view: {
        primitive: 'Unknown'
      }
    });

    expect(failed.requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        requestId: 'request-1',
        error: {
          kind: 'schema_invalid',
          errors: [
            {
              code: 'schema_invalid',
              path: 'root.primitive'
            }
          ]
        }
      }
    });
  });

  test('preserves stable worker error kinds when worker reports a controlled failure', () => {
    const failed = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'request-1',
        kind: 'source_policy_failed',
        message: 'JS block source transform failed.',
        errors: [
          {
            code: 'transform_failed',
            path: 'source.identifiers.window',
            message: "Identifier 'window' is not allowed in JS block source."
          }
        ]
      }
    );

    expect(failed.requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        requestId: 'request-1',
        error: {
          kind: 'source_policy_failed',
          errors: [{ path: 'source.identifiers.window' }]
        }
      }
    });
  });

  test('applies timeout and runtime error messages only to the current requestId', () => {
    const request1 = createRunRequest({ requestId: 'request-1' });
    const request2 = createRunRequest({ requestId: 'request-2' });
    const pendingTwo = run(
      run(createJsBlockRuntimeSession(), request1),
      request2
    );

    const staleTimeout = reduceJsBlockRuntimeSession(pendingTwo, {
      direction: 'host_to_worker',
      type: 'timeout',
      requestId: 'request-1'
    });

    expect(staleTimeout.requests['request-1']?.status).toBe('pending');
    expect(staleTimeout.requests['request-2']?.status).toBe('pending');
    expect(staleTimeout.rejections.at(-1)).toMatchObject({
      code: 'stale_request_id',
      requestId: 'request-1'
    });

    const timedOut = reduceJsBlockRuntimeSession(staleTimeout, {
      direction: 'host_to_worker',
      type: 'timeout',
      requestId: 'request-2'
    });

    expect(timedOut.requests['request-2']).toMatchObject({
      status: 'timed_out',
      result: {
        ok: false,
        requestId: 'request-2',
        error: {
          kind: 'runtime_timeout',
          errors: [
            {
              code: 'runtime_timeout',
              path: 'runtime'
            }
          ]
        }
      }
    });

    const runtimeFailed = reduceJsBlockRuntimeSession(
      run(timedOut, createRunRequest({ requestId: 'request-3' })),
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'request-3',
        message: 'Render failed'
      }
    );

    expect(runtimeFailed.requests['request-3']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        requestId: 'request-3',
        error: {
          kind: 'runtime_error',
          errors: [
            {
              code: 'runtime_error',
              path: 'runtime'
            }
          ]
        }
      }
    });
  });

  test('rejects late rendered messages after timeout without overwriting the timeout result', () => {
    const timedOut = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      {
        direction: 'host_to_worker',
        type: 'timeout',
        requestId: 'request-1'
      }
    );
    const timeoutResult = timedOut.requests['request-1']?.result;

    expect(timedOut.currentRequestId).toBeUndefined();

    const afterLateRendered = reduceJsBlockRuntimeSession(
      timedOut,
      completedMessage('request-1')
    );

    expect(afterLateRendered.requests['request-1']).toMatchObject({
      status: 'timed_out',
      result: timeoutResult
    });
    expect(afterLateRendered.rejections.at(-1)).toMatchObject({
      code: 'request_not_pending',
      requestId: 'request-1'
    });
  });

  test('rejects late rendered messages after runtime error without overwriting the error result', () => {
    const runtimeFailed = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'request-1',
        message: 'Render failed'
      }
    );
    const runtimeErrorResult = runtimeFailed.requests['request-1']?.result;

    expect(runtimeFailed.currentRequestId).toBeUndefined();

    const afterLateRendered = reduceJsBlockRuntimeSession(
      runtimeFailed,
      completedMessage('request-1')
    );

    expect(afterLateRendered.requests['request-1']).toMatchObject({
      status: 'failed',
      result: runtimeErrorResult
    });
    expect(afterLateRendered.rejections.at(-1)).toMatchObject({
      code: 'request_not_pending',
      requestId: 'request-1'
    });
  });

  test('rejects late worker messages after dispose without changing logs or effects', () => {
    const disposed = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      {
        direction: 'host_to_worker',
        type: 'dispose',
        requestId: 'request-1'
      }
    );

    expect(disposed.requests['request-1']).toMatchObject({
      status: 'disposed',
      logs: [],
      effects: []
    });
    expect(disposed.currentRequestId).toBeUndefined();

    const afterLateRendered = reduceJsBlockRuntimeSession(
      disposed,
      completedMessage('request-1')
    );
    const afterLateLog = reduceJsBlockRuntimeSession(afterLateRendered, {
      direction: 'worker_to_host',
      type: 'log',
      requestId: 'request-1',
      level: 'info',
      message: 'late log'
    });
    const afterLateEvent = reduceJsBlockRuntimeSession(afterLateLog, {
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'request-1',
      name: 'late-event',
      payload: { ok: true }
    });
    const afterLateInterface = reduceJsBlockRuntimeSession(afterLateEvent, {
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'request-1',
      effectId: 'late-effect',
      interfaceId: 'late_interface',
      schemaDigest: 'digest-late',
      request: { ok: true }
    });

    expect(afterLateInterface.requests['request-1']).toMatchObject({
      status: 'disposed',
      logs: [],
      effects: []
    });
    expect(
      afterLateInterface.rejections.filter(
        (rejection) =>
          rejection.code === 'request_not_pending' &&
          rejection.requestId === 'request-1'
      )
    ).toHaveLength(4);
  });

  test('AC-025 preserves stream operation identity in the runtime effect log', () => {
    const next = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      {
        direction: 'worker_to_host',
        type: 'interface',
        requestId: 'request-1',
        effectId: 'effect-stream-next',
        interfaceId: 'watch_run',
        schemaDigest: 'digest-stream',
        operation: 'stream_next',
        streamId: 'run-1:stream-1'
      }
    );

    expect(next.requests['request-1']?.effects).toContainEqual({
      type: 'interface',
      requestId: 'request-1',
      effectId: 'effect-stream-next',
      interfaceId: 'watch_run',
      schemaDigest: 'digest-stream',
      operation: 'stream_next',
      streamId: 'run-1:stream-1'
    });
  });

  test('rejects late runtime errors after a request is ready without overwriting the ready result', () => {
    const ready = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      completedMessage('request-1')
    );
    const readyResult = ready.requests['request-1']?.result;

    expect(ready.currentRequestId).toBeUndefined();

    const afterLateError = reduceJsBlockRuntimeSession(ready, {
      direction: 'worker_to_host',
      type: 'error',
      requestId: 'request-1',
      message: 'late failure'
    });

    expect(afterLateError.requests['request-1']).toMatchObject({
      status: 'ready',
      result: readyResult
    });
    expect(afterLateError.rejections.at(-1)).toMatchObject({
      code: 'request_not_pending',
      requestId: 'request-1'
    });
  });

  test('rejects unknown request ids and malformed messages as structured rejections', () => {
    const missingRequest = reduceJsBlockRuntimeSession(
      createJsBlockRuntimeSession(),
      {
        direction: 'worker_to_host',
        type: 'error',
        requestId: 'missing-request',
        message: 'Failed'
      }
    );

    expect(missingRequest.rejections).toContainEqual(
      expect.objectContaining({
        code: 'unknown_request_id',
        requestId: 'missing-request'
      })
    );

    const malformed = reduceJsBlockRuntimeSession(missingRequest, {
      direction: 'worker_to_host',
      type: 'completed',
      outputs: {},
      requestId: 'missing-schema'
    });

    expect(malformed.rejections).toContainEqual(
      expect.objectContaining({
        code: 'invalid_message',
        path: 'message.view'
      })
    );
  });
});
