import type { BlockProtocolError } from '@1flowbase/page-protocol';

import type { NativeTrustedBlockPreparePlan } from './native-trusted-block-manifest';

export type NativeTrustedBlockRootHandle = unknown;

export interface NativeTrustedBlockMountInput {
  plan: NativeTrustedBlockPreparePlan;
  root: NativeTrustedBlockRootHandle;
}

export interface NativeTrustedBlockMountedInstance {
  update?: (plan: NativeTrustedBlockPreparePlan) => void | Promise<void>;
  dispose?: () => void | Promise<void>;
}

export interface NativeTrustedBlockHostAdapter {
  mount(
    input: NativeTrustedBlockMountInput
  ):
    | NativeTrustedBlockMountedInstance
    | void
    | Promise<NativeTrustedBlockMountedInstance | void>;
}

export type NativeTrustedBlockHostStatus =
  | 'idle'
  | 'mounted'
  | 'failed'
  | 'disposed';

export interface NativeTrustedBlockHostState {
  status: NativeTrustedBlockHostStatus;
  blockId?: string;
  runtime?: NativeTrustedBlockPreparePlan['runtime'];
  error?: BlockProtocolError;
}

export interface NativeTrustedBlockHostOptions {
  adapter: NativeTrustedBlockHostAdapter;
}

export interface NativeTrustedBlockHost {
  getState(): NativeTrustedBlockHostState;
  mount(
    plan: NativeTrustedBlockPreparePlan,
    root: NativeTrustedBlockRootHandle
  ): Promise<NativeTrustedBlockHostState>;
  update(
    plan: NativeTrustedBlockPreparePlan
  ): Promise<NativeTrustedBlockHostState>;
  retry(): Promise<NativeTrustedBlockHostState>;
  dispose(): Promise<NativeTrustedBlockHostState>;
}

export function createNativeTrustedBlockHost(
  options: NativeTrustedBlockHostOptions
): NativeTrustedBlockHost {
  let state: NativeTrustedBlockHostState = { status: 'idle' };
  let mountedInstance: NativeTrustedBlockMountedInstance | undefined;
  let didDisposeInstance = false;
  let didDisposeHost = false;
  let currentPlan: NativeTrustedBlockPreparePlan | undefined;
  let currentRoot: NativeTrustedBlockRootHandle;

  const disposeMountedInstanceOnce = async (): Promise<void> => {
    if (didDisposeInstance) {
      return;
    }

    didDisposeInstance = true;
    await mountedInstance?.dispose?.();
    mountedInstance = undefined;
  };

  const mountAdapter = async (
    plan: NativeTrustedBlockPreparePlan,
    root: NativeTrustedBlockRootHandle
  ): Promise<NativeTrustedBlockHostState> => {
    try {
      const instance = await options.adapter.mount({ plan, root });
      if (didDisposeHost) {
        if (isMountedInstance(instance)) {
          await instance.dispose?.();
        }
        return state;
      }

      mountedInstance = isMountedInstance(instance) ? instance : undefined;
      didDisposeInstance = false;
      state = {
        status: 'mounted',
        blockId: plan.blockId,
        runtime: plan.runtime
      };
    } catch (error) {
      if (didDisposeHost) {
        return state;
      }

      state = {
        status: 'failed',
        blockId: plan.blockId,
        runtime: plan.runtime,
        error: createRuntimeError(
          'runtime.mount',
          `Native trusted block adapter mount failed: ${getErrorMessage(error)}`
        )
      };
    }

    return state;
  };

  return {
    getState() {
      return state;
    },
    async mount(plan, root) {
      if (didDisposeHost || state.status === 'disposed') {
        return state;
      }

      if (state.status === 'mounted') {
        return state;
      }

      currentPlan = plan;
      currentRoot = root;
      return mountAdapter(plan, root);
    },
    async retry() {
      if (didDisposeHost || !currentPlan) {
        return state;
      }

      try {
        await disposeMountedInstanceOnce();
      } catch (error) {
        state = {
          status: 'failed',
          blockId: currentPlan.blockId,
          runtime: currentPlan.runtime,
          error: createRuntimeError(
            'runtime.retry',
            `Native trusted block retry cleanup failed: ${getErrorMessage(error)}`
          )
        };
        return state;
      }

      return mountAdapter(currentPlan, currentRoot);
    },
    async update(plan) {
      if (
        didDisposeHost ||
        state.status !== 'mounted' ||
        !mountedInstance?.update
      ) {
        return state;
      }
      currentPlan = plan;
      try {
        await mountedInstance.update(plan);
        state = {
          status: 'mounted',
          blockId: plan.blockId,
          runtime: plan.runtime
        };
      } catch (error) {
        state = {
          status: 'failed',
          blockId: plan.blockId,
          runtime: plan.runtime,
          error: createRuntimeError(
            'runtime.update',
            `Native trusted block adapter update failed: ${getErrorMessage(error)}`
          )
        };
      }
      return state;
    },
    async dispose() {
      if (didDisposeHost) {
        return state;
      }

      didDisposeHost = true;
      try {
        await disposeMountedInstanceOnce();
        state = { status: 'disposed' };
      } catch (error) {
        state = {
          status: 'failed',
          error: createRuntimeError(
            'runtime.dispose',
            `Native trusted block adapter dispose failed: ${getErrorMessage(error)}`
          )
        };
      }

      return state;
    }
  };
}

function isMountedInstance(
  value: NativeTrustedBlockMountedInstance | void
): value is NativeTrustedBlockMountedInstance {
  return typeof value === 'object' && value !== null;
}

function createRuntimeError(path: string, message: string): BlockProtocolError {
  return {
    code: 'runtime_error',
    path,
    message
  };
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === 'string' && error.trim() !== '') {
    return error;
  }

  return 'unknown error';
}
