import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode
} from 'react';

export interface BlockSettingsItem {
  id: string;
  label: string;
  disabled?: boolean;
  onSelect(): void;
}

const emptyItems: readonly BlockSettingsItem[] = [];
function createRegistry() {
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

const Context = createContext<ReturnType<typeof createRegistry> | null>(null);
const subscribeEmpty = () => () => {};
const getEmptySnapshot = () => emptyItems;

/** Each canvas slot owns its configuration menu; registrations never cross blocks. */
export function BlockSettingsProvider({ children }: { children: ReactNode }) {
  const [registry] = useState(createRegistry);
  return <Context.Provider value={registry}>{children}</Context.Provider>;
}

export function useBlockSettings() {
  const registry = useContext(Context);
  return useSyncExternalStore(
    registry?.subscribe ?? subscribeEmpty,
    registry?.getSnapshot ?? getEmptySnapshot,
    getEmptySnapshot
  );
}

/** Register menu metadata, keeping handlers current without repeated parent updates. */
export function useRegisterBlockSettings(items: readonly BlockSettingsItem[]) {
  const registry = useContext(Context);
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
