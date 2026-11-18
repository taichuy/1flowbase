import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  useSyncExternalStore
} from 'react';

export interface BlockSettingsItem {
  id: string;
  label: string;
  disabled?: boolean;
  onSelect(): void;
}

const emptyItems: readonly BlockSettingsItem[] = [];
export function createBlockSettingsRegistry() {
  const registrations = new Map<string, readonly BlockSettingsItem[]>();
  const listeners = new Set<() => void>();
  let snapshot = emptyItems;
  const publish = () => {
    snapshot = [...registrations.values()].flat();
    listeners.forEach((listener) => listener());
  };
  return {
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getSnapshot: () => snapshot,
    register(owner: string, items: readonly BlockSettingsItem[]) {
      registrations.set(owner, items);
      publish();
      return () => {
        if (registrations.get(owner) !== items) return;
        registrations.delete(owner);
        publish();
      };
    }
  };
}

export const BlockSettingsContext = createContext<
  ReturnType<typeof createBlockSettingsRegistry> | null
>(null);
const subscribeEmpty = () => () => {};
const getEmptySnapshot = () => emptyItems;

export function useBlockSettings() {
  const registry = useContext(BlockSettingsContext);
  return useSyncExternalStore(
    registry?.subscribe ?? subscribeEmpty,
    registry?.getSnapshot ?? getEmptySnapshot,
    getEmptySnapshot
  );
}

/** Register menu metadata, keeping handlers current without repeated parent updates. */
export function useRegisterBlockSettings(items: readonly BlockSettingsItem[]) {
  const registry = useContext(BlockSettingsContext);
  const owner = useId();
  const latest = useRef(items);
  const signature = JSON.stringify(
    items.map(({ id, label, disabled }) => [id, label, Boolean(disabled)])
  );
  useLayoutEffect(() => {
    latest.current = items;
  });
  useLayoutEffect(
    () =>
      registry?.register(
        owner,
        latest.current.map((item) => ({
          ...item,
          onSelect: () =>
            latest.current.find((current) => current.id === item.id)?.onSelect()
        }))
      ),
    [registry, owner, signature]
  );
}
