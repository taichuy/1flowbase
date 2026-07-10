import { describe, expect, test } from 'vitest';

import {
  createJsBlockRuntimeSession,
  reduceJsBlockRuntimeSession,
  type JsBlockRunRequest,
  type JsBlockRuntimeSessionState
} from '../index';

const validSource = `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render() {
    return Text({ children: 'Ready' });
  }
});
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

function renderedMessage(requestId: string) {
  return {
    direction: 'worker_to_host',
    type: 'rendered',
    requestId,
    schema: {
      primitive: 'Text',
      props: { children: 'Ready' }
    }
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
      type: 'rendered',
      requestId: 'request-1',
      schema: {
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
        schema: {
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

  test('maps source transform failures into a stable run result before sending source to a worker', () => {
    const state = run(
      createJsBlockRuntimeSession(),
      createRunRequest({
        source: `
import { defineBlock } from '@1flowbase/block-sdk';

const block = defineBlock({
  render() {
    return { primitive: 'Text' };
  }
});
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
      renderedMessage('request-1')
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
      type: 'rendered',
      requestId: 'request-1',
      schema: {
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
    const pendingTwo = run(run(createJsBlockRuntimeSession(), request1), request2);

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
      renderedMessage('request-1')
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
      renderedMessage('request-1')
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
      renderedMessage('request-1')
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
    const afterLateData = reduceJsBlockRuntimeSession(afterLateEvent, {
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'request-1',
      queryId: 'late.query',
      params: { ok: true }
    });
    const afterLateAction = reduceJsBlockRuntimeSession(afterLateData, {
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      actionId: 'late-action',
      payload: { ok: true }
    });

    expect(afterLateAction.requests['request-1']).toMatchObject({
      status: 'disposed',
      logs: [],
      effects: []
    });
    expect(
      afterLateAction.rejections.filter(
        (rejection) =>
          rejection.code === 'request_not_pending' &&
          rejection.requestId === 'request-1'
      )
    ).toHaveLength(5);
  });

  test('rejects late runtime errors after a request is ready without overwriting the ready result', () => {
    const ready = reduceJsBlockRuntimeSession(
      run(createJsBlockRuntimeSession(), createRunRequest()),
      renderedMessage('request-1')
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
      type: 'rendered',
      requestId: 'missing-schema'
    });

    expect(malformed.rejections).toContainEqual(
      expect.objectContaining({
        code: 'invalid_message',
        path: 'message.schema'
      })
    );
  });
});
