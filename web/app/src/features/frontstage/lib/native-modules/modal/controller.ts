import type { Modal } from 'antd';
import { startTransition } from 'react';

type ModalApi = ReturnType<typeof Modal.useModal>[0];
export type ModalHandle = ReturnType<ModalApi['confirm']>;
export type ModalConfig = Parameters<ModalApi['confirm']>[0];
export const modalMethods = [
  'confirm',
  'info',
  'success',
  'error',
  'warning'
] as const;

/** One controller per mounted/evaluated Block, never per cached artifact. */
export function createModalController() {
  let active = false;
  let api: ModalApi | undefined;
  const handles = new Set<ModalHandle>();
  function destroyAll() {
    for (const handle of [...handles]) handle.destroy();
  }
  return {
    activate() {
      active = true;
    },
    dispose() {
      active = false;
      destroyAll();
    },
    bind(next: ModalApi) {
      api = next;
    },
    destroyAll,
    call(method: keyof ModalApi, config: ModalConfig): ModalHandle {
      return active && api ? api[method](config) : closedHandle();
    },
    wrap(
      raw: ModalApi,
      owned: Set<ModalHandle>,
      getContainer: () => HTMLElement
    ): ModalApi {
      return Object.fromEntries(
        modalMethods.map((method) => [
          method,
          (config: ModalConfig) => {
            if (!active) return closedHandle();
            let finished = false;
            let settleClosed!: (value: boolean) => void;
            const closed = new Promise<boolean>((resolve) => {
              settleClosed = resolve;
            });
            const release = () => {
              finished = true;
              handles.delete(handle);
              owned.delete(handle);
            };
            const resolveConfig = (value: ModalConfig): ModalConfig => ({
              ...value,
              getContainer,
              afterClose: () => {
                release();
                value.afterClose?.();
              }
            });
            let current = config;
            let original!: ModalHandle;
            // Keep the committed holder attached while lazy modal content loads.
            startTransition(() => {
              original = raw[method](resolveConfig(current));
            });
            const handle: ModalHandle = {
              destroy() {
                if (finished) return;
                release();
                settleClosed(false);
                startTransition(() => original.destroy());
              },
              update(next) {
                if (finished || !active) return;
                current = {
                  ...current,
                  ...(typeof next === 'function' ? next(current) : next)
                };
                startTransition(() => original.update(resolveConfig(current)));
              },
              then(resolve, reject) {
                return Promise.race([
                  original.then(
                    (value) => value,
                    () => {}
                  ),
                  closed
                ]).then(resolve, (error: unknown) => {
                  reject?.();
                  throw error;
                });
              }
            };
            handles.add(handle);
            owned.add(handle);
            return handle;
          }
        ])
      ) as ModalApi;
    }
  };
}

function closedHandle(): ModalHandle {
  return {
    destroy() {},
    update() {},
    then: (resolve) => Promise.resolve(false).then(resolve)
  };
}
