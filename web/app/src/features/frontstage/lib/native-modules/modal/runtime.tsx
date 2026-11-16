import { Modal as AntdModal, type ModalProps } from 'antd';
import {
  Suspense,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  type ReactNode
} from 'react';
import { useNativeBlockSurface } from '../native-block-surface-context';
import {
  createModalController,
  modalMethods,
  type ModalConfig,
  type ModalHandle
} from './controller';

export function createBlockModalRuntime() {
  const controller = createModalController();
  function useModal(): ReturnType<typeof AntdModal.useModal> {
    const surface = useNativeBlockSurface();
    const [raw, holder] = AntdModal.useModal();
    const owned = useRef<Set<ModalHandle> | null>(null);
    const api = useMemo(() => {
      let wrapped: typeof raw | undefined;
      return Object.fromEntries(
        modalMethods.map((method) => [
          method,
          (config: ModalConfig) => {
            wrapped ??= controller.wrap(
              raw,
              (owned.current ??= new Set<ModalHandle>()),
              () => {
                if (!surface)
                  throw new Error('Modal requires a mounted Block surface.');
                return surface.overlayHost.getPopupContainer();
              }
            );
            return wrapped[method](config);
          }
        ])
      ) as typeof raw;
    }, [raw, surface]);
    useEffect(() => {
      return () => {
        for (const handle of [...(owned.current ?? [])]) handle.destroy();
      };
    }, []);
    // Lazy content must not suspend the Block and disconnect its resource owner.
    return [
      api,
      <Suspense key="modal-holder" fallback={null}>
        {holder}
      </Suspense>
    ];
  }
  function Lifecycle() {
    const surface = useNativeBlockSurface();
    useLayoutEffect(() => {
      controller.activate();
    }, []);
    // Suspense temporarily disconnects layout effects while loading content.
    // Only actual unmount/surface disposal should release these dialogs.
    useEffect(() => {
      controller.activate();
      const unregister = surface?.registerEffectResource({
        invalidate: controller.destroyAll,
        dispose: controller.dispose
      });
      return () => {
        unregister?.();
        controller.dispose();
      };
    }, [surface]);
    return null;
  }
  function Provider({ children }: { children: ReactNode }) {
    const [api, holder] = useModal();
    controller.bind(api);
    // Commit the holder and activate before user layout effects can open a modal.
    return (
      <>
        {holder}
        <Lifecycle />
        {children}
      </>
    );
  }
  function ScopedModal(props: ModalProps) {
    const surface = useNativeBlockSurface();
    return (
      <AntdModal
        {...props}
        getContainer={
          props.getContainer === false
            ? false
            : surface?.overlayHost.getPopupContainer
        }
      />
    );
  }
  const Modal = Object.assign(ScopedModal, AntdModal, {
    ...Object.fromEntries(
      modalMethods.map((method) => [
        method,
        (config: Parameters<typeof AntdModal.confirm>[0]) =>
          controller.call(method, config)
      ])
    ),
    useModal,
    warn: (config: Parameters<typeof AntdModal.warning>[0]) =>
      controller.call('warning', config),
    destroyAll: controller.destroyAll,
    config: () => {
      throw new Error('Modal.config is global; use a Block ConfigProvider.');
    }
  }) as typeof AntdModal;
  return { Modal, Provider };
}
