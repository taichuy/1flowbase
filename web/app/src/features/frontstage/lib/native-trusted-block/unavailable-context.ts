import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime/browser';

export function createFrontstageUnavailableBlockContext(
  plan: NativeTrustedBlockPreparePlan
): BlockContextSeed {
  const state: Record<string, unknown> = {};

  return {
    currentUser: null,
    workspace: { id: 'workspace' },
    application: null,
    page: { id: plan.blockId, route: plan.blockId },
    inputs: {},
    outputs: {
      publish() {
        return {
          ok: false,
          stale: false,
          error: 'Native trusted block outputs are unavailable.'
        };
      }
    },
    params: {},
    props: { ...plan.props },
    state,
    patch(patch) {
      Object.assign(state, patch);
    },
    api: {
      get: rejectUnavailable('ctx.api.get'),
      post: rejectUnavailable('ctx.api.post'),
      put: rejectUnavailable('ctx.api.put'),
      patch: rejectUnavailable('ctx.api.patch'),
      delete: rejectUnavailable('ctx.api.delete'),
      head: rejectUnavailable('ctx.api.head'),
      options: rejectUnavailable('ctx.api.options'),
      stream: () => ({
        [Symbol.asyncIterator]() {
          return {
            next: async () => {
              throw createUnavailableContextError('ctx.api.stream');
            }
          };
        }
      })
    },
    events: {
      emit() {
        throw createUnavailableContextError('ctx.events.emit');
      }
    },
    navigation: {
      openBlock() {
        throw createUnavailableContextError('ctx.navigation.openBlock');
      }
    },
    theme: { mode: 'light', tokens: {} },
    ui: {}
  };
}

function rejectUnavailable<Args extends unknown[]>(
  capability: string
): (...args: Args) => Promise<never> {
  return async () => {
    throw createUnavailableContextError(capability);
  };
}

function createUnavailableContextError(capability: string): Error {
  return new Error(
    `Native trusted block ${capability} is unavailable until the host injects a controlled BlockContext.`
  );
}
