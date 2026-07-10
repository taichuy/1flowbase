import { describe, expect, test, vi } from 'vitest';

import {
  createBlockContextMediator,
  createJsBlockHostEffectBridge
} from '../index';

describe('JS block host mediator effect bridge', () => {
  test('resolves allowed data and action effects through handlers as effect_result messages', () => {
    const messages: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedActions: ['record.save'],
        allowedQueries: ['records'],
        allowedDataOperations: ['query']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      },
      handlers: {
        data: (effect) => ({
          queryId: effect.queryId,
          rows: [{ id: 'record-1' }]
        }),
        action: (effect) => ({
          actionId: effect.actionId,
          saved: true
        })
      }
    });

    const dataResult = bridge.handle({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'request-1',
      effectId: 'effect-data',
      queryId: 'records',
      params: { where: { id: 'record-1' } }
    });
    const actionResult = bridge.handle({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      effectId: 'effect-action',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });

    expect(dataResult).toMatchObject({
      handled: true,
      transition: {
        result: {
          ok: true,
          effect: { type: 'data', effectId: 'effect-data' }
        }
      }
    });
    expect(actionResult).toMatchObject({
      handled: true,
      transition: {
        result: {
          ok: true,
          effect: { type: 'action', effectId: 'effect-action' }
        }
      }
    });
    expect(messages).toEqual([
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'effect-data',
        ok: true,
        value: { queryId: 'records', rows: [{ id: 'record-1' }] }
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'effect-action',
        ok: true,
        value: { actionId: 'record.save', saved: true }
      }
    ]);
  });

  test('converts denied data and action effects to failed effect_result messages', () => {
    const messages: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedActions: ['record.save'],
        allowedQueries: ['records'],
        allowedDataOperations: ['query']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      }
    });

    bridge.handle({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'request-1',
      effectId: 'effect-data',
      queryId: 'private_records'
    });
    bridge.handle({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      effectId: 'effect-action',
      actionId: 'record.delete'
    });

    expect(messages).toEqual([
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'effect-data',
        ok: false,
        error: {
          kind: 'runtime_error',
          message: 'Query is not allowed: private_records.',
          errors: [
            {
              code: 'query_denied',
              path: 'data.queryId',
              message: 'Query is not allowed: private_records.'
            }
          ]
        }
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'effect-action',
        ok: false,
        error: {
          kind: 'runtime_error',
          message: 'Action is not allowed: record.delete.',
          errors: [
            {
              code: 'action_denied',
              path: 'action.actionId',
              message: 'Action is not allowed: record.delete.'
            }
          ]
        }
      }
    ]);
  });

  test('keeps allowed events fire-and-forget without sending effect_result', () => {
    const messages: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedEvents: ['record.saved']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      }
    });

    const result = bridge.handle(
      {
        direction: 'worker_to_host',
        type: 'event',
        requestId: 'request-1',
        name: 'record.saved',
        payload: { id: 'record-1' }
      },
      { tickId: 'tick-1' }
    );

    expect(result).toMatchObject({
      handled: true,
      transition: {
        result: {
          ok: true,
          effect: {
            type: 'event',
            name: 'record.saved',
            payload: { id: 'record-1' }
          }
        }
      }
    });
    expect(messages).toEqual([]);
  });

  test('returns stable denial results for denied events without sending effect_result', () => {
    const messages: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedEvents: ['record.saved']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      }
    });

    const result = bridge.handle({
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'request-1',
      name: 'record.deleted'
    });

    expect(result).toMatchObject({
      handled: true,
      transition: {
        result: {
          ok: false,
          requestId: 'request-1',
          code: 'event_denied',
          path: 'event.name',
          message: 'Event is not allowed: record.deleted.'
        }
      }
    });
    expect(messages).toEqual([]);
  });

  test('does not resolve action or data effects that are missing effectId', () => {
    const messages: unknown[] = [];
    const dataHandler = vi.fn(() => ({ rows: [] }));
    const actionHandler = vi.fn(() => ({ ok: true }));
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedActions: ['record.save'],
        allowedQueries: ['records'],
        allowedDataOperations: ['query']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      },
      handlers: {
        data: dataHandler,
        action: actionHandler
      }
    });

    bridge.handle({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'request-1',
      queryId: 'records'
    });
    bridge.handle({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      actionId: 'record.save'
    });

    expect(messages).toEqual([]);
    expect(dataHandler).not.toHaveBeenCalled();
    expect(actionHandler).not.toHaveBeenCalled();
  });

  test('AC-010 rejects allowed effects when the host has no matching handler', () => {
    const messages: unknown[] = [];
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedActions: ['record.save'],
        allowedQueries: ['records'],
        allowedDataOperations: ['query']
      }),
      resolveEffect: (message) => {
        messages.push(message);
      }
    });

    bridge.handle({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'request-1',
      effectId: 'effect-data',
      queryId: 'records'
    });
    bridge.handle({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'request-1',
      effectId: 'effect-action',
      actionId: 'record.save'
    });

    expect(messages).toEqual([
      expect.objectContaining({
        type: 'effect_result',
        effectId: 'effect-data',
        ok: false,
        error: expect.objectContaining({
          kind: 'runtime_error',
          errors: [
            expect.objectContaining({
              code: 'query_denied',
              path: 'data.handler'
            })
          ]
        })
      }),
      expect.objectContaining({
        type: 'effect_result',
        effectId: 'effect-action',
        ok: false,
        error: expect.objectContaining({
          kind: 'runtime_error',
          errors: [
            expect.objectContaining({
              code: 'action_denied',
              path: 'action.handler'
            })
          ]
        })
      })
    ]);
  });

  test('preserves mediator state such as event chain depth', () => {
    const bridge = createJsBlockHostEffectBridge({
      mediator: createBlockContextMediator({
        allowedEvents: ['record.saved'],
        maxEventChainDepth: 3
      }),
      resolveEffect: () => undefined
    });
    const event = {
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'request-1',
      name: 'record.saved'
    } as const;

    bridge.handle(event, { tickId: 'tick-1' });
    bridge.handle(event, { tickId: 'tick-1' });

    expect(bridge.getMediatorState()).toEqual({
      eventChains: {
        'request-1::tick-1': 2
      }
    });
  });
});
